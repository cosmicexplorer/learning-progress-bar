import os
import re
from dataclasses import dataclass

from pants.build_graph.address import Address, BuildFileAddress
from pants.engine.addressable import BuildFileAddresses
from pants.engine.fs import Digest, DirectoriesToMerge, Snapshot
from pants.engine.isolated_process import ExecuteProcessRequest, ExecuteProcessResult
from pants.engine.legacy.graph import HydratedTarget, HydratedTargets, TransitiveHydratedTargets
from pants.engine.legacy.structs import CargoTargetAdaptor, TargetAdaptor
from pants.engine.objects import Collection
from pants.engine.rules import RootRule, rule
from pants.engine.selectors import Get, MultiGet
from pants.rules.core.strip_source_root import SourceRootStrippedSources
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
class RustThriftBuildResult:
  snapshot: Snapshot


@rule
async def execute_rust_thrift(
  rust_thrift_target: RustThriftLibraryWrapper,
) -> RustThriftBuildResult:
  buildable_target = rust_thrift_target.underlying
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
  expected_output_files = [re.sub(r'\.thrift$', '.rs', f) for f in cur_target_sources]
  exe_res = await Get[ExecuteProcessResult](
    ExecuteProcessRequest(
      argv=('thrift', '--gen', 'rs', '-o', '.', *cur_target_sources),
      input_files=merged_stripped_sources,
      description=f'invoke thrift rust for target {buildable_target.address}!',
      env={'PATH': os.environ['PATH']},
      output_files=tuple(expected_output_files),
    )
  )
  snapshot = await Get[Snapshot](Digest, exe_res.output_directory_digest)
  return RustThriftBuildResult(snapshot)


@rule
async def get_rust_thrift_for_subproject(cargo_target: CargoTargetAdaptor) -> RustThriftBuildResult:
  cur_target_bfa = BuildFileAddress(
    build_file=None,
    target_name=cargo_target.address.target_name,
    rel_path=os.path.join(cargo_target.address.spec_path, 'BUILD'),
  )
  thts = await Get[TransitiveHydratedTargets](BuildFileAddresses((cur_target_bfa,)))
  rust_thrift_targets = await Get[ManyRustThriftLibraryAdaptors](
    HydratedTargets(tuple(thts.closure))
  )
  results = await MultiGet(
    Get[RustThriftBuildResult](RustThriftLibraryWrapper, t) for t in rust_thrift_targets
  )
  merged_digest = await Get[Digest](
    DirectoriesToMerge(tuple(r.snapshot.directory_digest for r in results))
  )

  merged_files = [f for r in results for f in r.snapshot.files]

  return RustThriftBuildResult(Snapshot(merged_digest, files=tuple(merged_files), dirs=()))


def rules():
  return [
    filter_rust_thrift_targets,
    RootRule(RustThriftLibraryWrapper),
    execute_rust_thrift,
    get_rust_thrift_for_subproject,
  ]
