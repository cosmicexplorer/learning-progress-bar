import os
import re
from dataclasses import dataclass
from enum import Enum

from pants.backend.codegen.thrift.python.python_thrift_library import PythonThriftLibrary
from pants.backend.python.rules.pex_from_target_closure import PythonResources, PythonResourceTarget
from pants.build_graph.address import Address, BuildFileAddress
from pants.engine.addressable import BuildFileAddresses
from pants.engine.fs import Digest, DirectoriesToMerge, Snapshot
from pants.engine.isolated_process import ExecuteProcessRequest, ExecuteProcessResult
from pants.engine.legacy.graph import HydratedTarget, HydratedTargets, TransitiveHydratedTargets
from pants.engine.legacy.structs import CargoTargetAdaptor, TargetAdaptor, PythonThriftLibraryAdaptor
from pants.engine.objects import Collection
from pants.engine.rules import RootRule, UnionRule, rule
from pants.engine.selectors import Get, MultiGet
from pants.rules.core.strip_source_root import SourceRootStrippedSources
from pants.util.enums import match
from upstreamable.targets.rust_thrift_library import RustThriftLibrary


@dataclass(frozen=True)
class RustThriftLibraryWrapper:
  underlying: TargetAdaptor


class ManyRustThriftLibraryAdaptors(Collection[RustThriftLibraryWrapper]):
  pass


@rule
def filter_rust_thrift_targets(hts: HydratedTargets) -> ManyRustThriftLibraryAdaptors:
  return ManyRustThriftLibraryAdaptors(
    tuple(
      RustThriftLibraryWrapper(ht.adaptor)
      for ht in hts
      if ht.adaptor.type_alias == RustThriftLibrary.alias()
    )
  )


@dataclass(frozen=True)
class PythonThriftLibraryWrapper:
  underlying: TargetAdaptor


class ManyPythonThriftLibraryAdaptors(Collection[PythonThriftLibraryWrapper]):
  pass


@rule
def filter_python_thrift_targets(hts: HydratedTargets) -> ManyPythonThriftLibraryAdaptors:
  return ManyPythonThriftLibraryAdaptors(
    tuple(
      PythonThriftLibraryWrapper(ht.adaptor)
      for ht in hts
      if ht.adaptor.type_alias == PythonThriftLibrary.alias()
    )
  )


@dataclass(frozen=True)
class ThriftBuildResult:
  snapshot: Snapshot


class ThriftLanguage(Enum):
  rust = 'rust'
  python = 'python'


@dataclass(frozen=True)
class ThriftRequest:
  target: TargetAdaptor
  language: ThriftLanguage


def _strip_thrift_file_ext(filename: str) -> str:
  return re.sub(r'\.thrift$', '', filename)


@rule
async def execute_thrift(
  request: ThriftRequest
) -> ThriftBuildResult:
  buildable_target = request.target
  thts = await Get[TransitiveHydratedTargets](
    BuildFileAddresses((buildable_target.address,) + tuple(buildable_target.dependencies))
  )
  all_stripped_sources = await MultiGet(
    Get[SourceRootStrippedSources](HydratedTarget, ht) for ht in thts.closure
  )
  merged_stripped_sources = await Get[Digest](
    DirectoriesToMerge(tuple(s.snapshot.directory_digest for s in all_stripped_sources))
  )

  all_input_file_paths = (await Get[Snapshot](Digest, merged_stripped_sources)).files
  cur_target_sources = [f for f in all_input_file_paths if f.endswith('.thrift')]

  # Get the expected output files or directories, depending on language.
  if request.language == ThriftLanguage.rust:
    outputs = dict(output_files=tuple(f'{_strip_thrift_file_ext(f)}.rs'
                                      for f in cur_target_sources))
  else:
    assert request.language == ThriftLanguage.python
    outputs = dict(output_directories=('gen-py',))

  # Execute thrift.
  exe_res = await Get[ExecuteProcessResult](
    ExecuteProcessRequest(
      argv=('thrift',
            '--gen', match(request.language, {
              ThriftLanguage.rust: 'rs',
              ThriftLanguage.python: 'py',
            }),
            '-o', '.',
            *cur_target_sources),
      input_files=merged_stripped_sources,
      description=f'invoke thrift {request.language} for target {buildable_target.address}!',
      env={'PATH': os.environ['PATH']},
      **outputs,
    )
  )
  snapshot = await Get[Snapshot](Digest, exe_res.output_directory_digest)
  return ThriftBuildResult(snapshot)


@dataclass(frozen=True)
class ThriftTargetRequest:
  target: TargetAdaptor
  language: ThriftLanguage


@rule
async def get_thrift_for_subproject(request: ThriftTargetRequest) -> ThriftBuildResult:
  thriftable_target = request.target

  # Get all dependencies that are also thrift library targets to put them in the same chroot when
  # executing thrift.
  cur_target_bfa = BuildFileAddress(
    build_file=None,
    target_name=thriftable_target.address.target_name,
    rel_path=os.path.join(thriftable_target.address.spec_path, 'BUILD'),
  )
  thts = await Get[TransitiveHydratedTargets](BuildFileAddresses((cur_target_bfa,)))

  if request.language == ThriftLanguage.rust:
    thrift_targets = await Get[ManyRustThriftLibraryAdaptors](
      HydratedTargets(tuple(thts.closure))
    )
  else:
    assert request.language == ThriftLanguage.python
    thrift_targets = await Get[ManyPythonThriftLibraryAdaptors](
      HydratedTargets(tuple(thts.closure))
    )

  results = await MultiGet(
    Get[ThriftBuildResult](ThriftRequest(
      target=t.underlying,
      language=request.language,
    )) for t in thrift_targets
  )
  merged_digest = await Get[Digest](
    DirectoriesToMerge(tuple(r.snapshot.directory_digest for r in results))
  )

  merged_files = [f for r in results for f in r.snapshot.files]

  return ThriftBuildResult(Snapshot(merged_digest, files=tuple(merged_files), dirs=()))


@rule
async def collect_python_thrift(python_target: PythonThriftLibraryAdaptor) -> PythonResources:
  res = await Get[ThriftBuildResult](ThriftTargetRequest(
    target=python_target,
    language=ThriftLanguage.python,
  ))
  return PythonResources(res.snapshot)


def rules():
  return [
    filter_rust_thrift_targets,
    filter_python_thrift_targets,
    RootRule(ThriftRequest),
    RootRule(ThriftTargetRequest),
    execute_thrift,
    get_thrift_for_subproject,
    UnionRule(PythonResourceTarget, PythonThriftLibraryAdaptor),
    RootRule(PythonThriftLibraryAdaptor),
    collect_python_thrift,
  ]
