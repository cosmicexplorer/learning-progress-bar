import dataclasses
import logging
import os
import re
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import List, Tuple

from pants.backend.python.rules.pex_from_target_closure import PythonResources, PythonResourceTarget
from pants.build_graph.address import Address, BuildFileAddress
from pants.engine.addressable import BuildFileAddresses
from pants.engine.console import Console
from pants.engine.fs import (
  Digest,
  DirectoriesToMerge,
  DirectoryWithPrefixToAdd,
  MergeDirectoriesStrictness,
  Snapshot,
)
from pants.engine.goal import Goal, GoalSubsystem
from pants.engine.isolated_process import (
  ExecuteProcessRequest,
  ExecuteProcessResult,
  FallibleExecuteProcessResult,
)
from pants.engine.legacy.graph import HydratedTarget, HydratedTargets, TransitiveHydratedTargets
from pants.engine.legacy.structs import CargoTargetAdaptor
from pants.engine.objects import Collection
from pants.engine.parser import SymbolTable
from pants.engine.rules import RootRule, UnionRule, console_rule, subsystem_rule, rule
from pants.engine.selectors import Get, MultiGet
from pants.rules.core.core_test_model import Status, TestResult, TestTarget
from pants.rules.core.strip_source_root import SourceRootStrippedSources
from pants.subsystem.subsystem import Subsystem
from pants.util.enums import match
from upstreamable.rules.rust_thrift import ThriftTargetRequest, ThriftBuildResult, ThriftLanguage
from upstreamable.targets.cargo_subproject import CargoSubproject

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class RelPath:
  path: Path

  def __post_init__(self):
    assert not self.path.is_absolute()


class CargoCommands(Enum):
  build = 'build'
  test = 'test'

  def create_cargo_command_argv(self, launcher_path: RelPath) -> List[str]:
    intermediate_args = match(self, {CargoCommands.build: ['build'], CargoCommands.test: ['test']})
    return [str(launcher_path.path), *intermediate_args]


@dataclass(frozen=True)
class Cargo:
  launcher_path: RelPath
  release_mode: bool

  class Factory(Subsystem):
    options_scope = 'cargo'

    @classmethod
    def register_options(cls, register):
      super().register_options(register)
      # FIXME: make this a BinaryTool using the UrlToFetch capability from
      # https://github.com/pantsbuild/pants/pull/8825!
      register(
        '--launcher-path',
        type=str,
        default=None,
        fingerprint=True,
        help='EXPERIMENTAL AND HACKY: Path to the cargo launcher script to execute cargo '
        'locally.',
      )
      register(
        '--release-mode',
        type=bool,
        default=False,
        fingerprint=True,
        help='EXPERIMENTAL: set whether to build cargo artifacts in debug or release mode.',
      )

    def build(self) -> 'Cargo.Factory':
      options = self.get_options()
      return Cargo(
        launcher_path=RelPath(Path(options.launcher_path or 'cargo')),
        release_mode=bool(options.release_mode),
      )

  @property
  def _release_mode_subdir(self) -> str:
    return 'release' if self.release_mode else 'debug'

  def _get_expected_output_binary_file(self, cargo_target: CargoTargetAdaptor) -> str:
    return os.path.join('target', self._release_mode_subdir, str(cargo_target.cargo_output))

  def _glob_generated_resources(self, cargo_target: CargoTargetAdaptor) -> List[str]:
    return list(cargo_target.generated_resources.include)

  @property
  def _output_dir(self) -> str:
    return f'target/{self._release_mode_subdir}'

  def _rewrite_subproject_output_file(self, output_file: str) -> str:
    ret = re.sub(f'^({self._output_dir})', '', output_file)
    return f'{self._output_dir}/deps/{ret}'

  def create_execute_process_request(
    self,
    cargo_target: CargoTargetAdaptor,
    source_root_stripped_sources: Digest,
    command: CargoCommands,
    extra_cargo_output_file_paths: Tuple[str, ...],
  ) -> ExecuteProcessRequest:
    argv = command.create_cargo_command_argv(self.launcher_path)
    argv.extend(['--features', ' '.join([
      'pants-injected',
      *cargo_target.features,
    ])])
    ret = ExecuteProcessRequest(
      argv=tuple(argv),
      input_files=source_root_stripped_sources,
      description=f'Execute cargo to build the request {cargo_target}!',
      env={
        # FIXME: a way to explicitly say "this is a hacky non-remotable process execution", which
        # automatically adds the PATH to the subprocess env!
        'PATH': os.environ['PATH'],
        'MODE': self._release_mode_subdir,
      },
      output_files=(
        self._get_expected_output_binary_file(cargo_target),
        *self._glob_generated_resources(cargo_target),
        *(self._rewrite_subproject_output_file(str(f)) for f in extra_cargo_output_file_paths),
      ),
    )
    logger.debug(f'creating process execution request for cargo: {ret}')
    return ret


@rule
def make_cargo(factory: Cargo.Factory) -> Cargo:
  return factory.build()


class ManyCargoTargetAdaptors(Collection[CargoTargetAdaptor]):
  pass


@rule
def filter_cargo_buildable_targets(hts: HydratedTargets, cargo: Cargo) -> ManyCargoTargetAdaptors:
  buildable_targets = [
    cargo.prepare_buildable_target(ht)
    for ht in hts
    if ht.adaptor.type_alias == CargoSubproject.alias()
  ]
  return ManyCargoTargetAdaptors(tuple(buildable_targets))


@dataclass(frozen=True)
class CargoTargetMergedSources:
  digest: Digest
  cargo_output_file_paths: Tuple[str, ...]


@rule
async def prepare_cargo_target_sources(
  cargo_target: CargoTargetAdaptor,
) -> CargoTargetMergedSources:
  cur_target_bfa = BuildFileAddress(
    build_file=None,
    target_name=cargo_target.address.target_name,
    rel_path=os.path.join(cargo_target.address.spec_path, 'BUILD'),
  )

  # Resources.
  thts = await Get[TransitiveHydratedTargets](BuildFileAddresses((cur_target_bfa,)))
  all_stripped_sources = await MultiGet(
    Get[SourceRootStrippedSources](HydratedTarget, ht) for ht in thts.closure
  )
  all_stripped_sources = [s.snapshot.directory_digest for s in all_stripped_sources]

  # Thrift generation.
  thrift_result = await Get[ThriftBuildResult](ThriftTargetRequest(
    target=cargo_target,
    language=ThriftLanguage.rust,
  ))

  # Inject any subprojects.
  all_subproject_digests = [
    ht.adaptor.sources.snapshot.directory_digest
    for ht in cargo_target.cargo_subprojects
  ]

  all_subproject_output_files = [
    ht.adaptor.cargo_output
    for ht in cargo_target.cargo_subprojects
  ]

  all_merged_sources = await Get[Digest](
    DirectoriesToMerge(
      (*all_stripped_sources, thrift_result.snapshot.directory_digest, *all_subproject_digests),
      strictness=MergeDirectoriesStrictness.allow_duplicates,
    )
  )

  return CargoTargetMergedSources(
    digest=all_merged_sources,
    cargo_output_file_paths=tuple(all_subproject_output_files))


@dataclass(frozen=True)
class CargoBuildResult:
  snapshot: Snapshot


@rule
async def execute_cargo(buildable_target: CargoTargetAdaptor, cargo: Cargo) -> CargoBuildResult:
  all_merged_sources = await Get[CargoTargetMergedSources](CargoTargetAdaptor, buildable_target)

  exe_res = await Get[ExecuteProcessResult](
    ExecuteProcessRequest,
    cargo.create_execute_process_request(
      cargo_target=buildable_target,
      source_root_stripped_sources=all_merged_sources.digest,
      command=CargoCommands.build,
      extra_cargo_output_file_paths=all_merged_sources.cargo_output_file_paths,
    ),
  )
  snapshot = await Get[Snapshot](Digest, exe_res.output_directory_digest)
  return CargoBuildResult(snapshot=snapshot)


@rule
async def collect_built_cargo_resources(buildable_target: CargoTargetAdaptor) -> PythonResources:
  res = await Get[CargoBuildResult](CargoTargetAdaptor, buildable_target)
  return PythonResources(res.snapshot)


@rule
async def execute_cargo_test(testable_target: CargoTargetAdaptor, cargo: Cargo) -> TestResult:
  all_merged_sources = await Get[CargoTargetMergedSources](CargoTargetAdaptor, testable_target)

  exe_res = await Get[FallibleExecuteProcessResult](
    ExecuteProcessRequest,
    cargo.create_execute_process_request(
      cargo_target=testable_target,
      source_root_stripped_sources=all_merged_sources.digest,
      command=CargoCommands.test,
      extra_cargo_output_file_paths=all_merged_sources.cargo_output_file_paths,
    ),
  )
  return TestResult.from_fallible_execute_process_result(exe_res)


# class BuildRustOptions(GoalSubsystem):
#   """???/build rust binaries!"""

#   name = 'build-rust-binary'

#   @classmethod
#   def register_options(cls, register):
#     super().register_options(register)
#     register('--command', type=CargoCommands, default=CargoCommands.build, help='???')


# class BuildRust(Goal):
#   subsystem_cls = BuildRustOptions

# @console_rule
# async def build_rust(console: Console, hts: HydratedTargets, options: BuildRustOptions) -> BuildRust:
#   cargo_targets = await Get[ManyCargoTargetAdaptors](HydratedTargets, hts)

#   command = options.values.command
#   if command == CargoCommands.test:
#     test_results = await MultiGet(Get[TestResult](CargoTargetAdaptor, t) for t in cargo_targets)
#   else:
#     assert command == CargoCommands.build
#     build_results = await MultiGet(Get[CargoBuildResult](CargoTargetAdaptor, t) for t in cargo_targets)
#   return BuildRust(exit_code=0)


def rules():
  return [
    subsystem_rule(Cargo.Factory),
    make_cargo,
    RootRule(CargoTargetAdaptor),
    UnionRule(PythonResourceTarget, CargoTargetAdaptor),
    UnionRule(TestTarget, CargoTargetAdaptor),
    filter_cargo_buildable_targets,
    prepare_cargo_target_sources,
    execute_cargo,
    collect_built_cargo_resources,
    execute_cargo_test,
    # build_rust,
  ]
