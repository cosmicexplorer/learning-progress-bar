import dataclasses
import os
from dataclasses import dataclass
from pathlib import Path
from typing import List

from pants.build_graph.address import Address
from pants.engine.console import Console
from pants.engine.fs import Digest, DirectoriesToMerge, Snapshot
from pants.engine.goal import Goal, GoalSubsystem
from pants.engine.isolated_process import (ExecuteProcessRequest, ExecuteProcessResult,
                                           FallibleExecuteProcessResult)
from pants.engine.legacy.graph import HydratedTarget, HydratedTargets
from pants.engine.legacy.structs import CargoTargetAdaptor
from pants.engine.objects import Collection
from pants.engine.parser import SymbolTable
from pants.engine.rules import RootRule, UnionRule, console_rule, optionable_rule, rule
from pants.engine.selectors import Get
from pants.rules.core.core_test_model import Status, TestResult, TestTarget
from pants.rules.core.strip_source_root import SourceRootStrippedSources
from pants.subsystem.subsystem import Subsystem
from pants.util.collections import Enum

from upstreamable.targets.cargo_subproject import CargoSubproject


@dataclass(frozen=True)
class RelPath:
  path: Path

  def __post_init__(self):
    assert not self.path.is_absolute()


class CargoCommands(Enum):
  build = 'build'
  test = 'test'

  def create_cargo_command_argv(self, launcher_path: RelPath) -> List[str]:
    intermediate_args = self.match({
      CargoCommands.build: ['build'],
      CargoCommands.test: ['test'],
    })
    return tuple([str(launcher_path.path)] + intermediate_args)


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
      register('--launcher-path', type=str, default=None, fingerprint=True,
               help='EXPERIMENTAL AND HACKY: Path to the cargo launcher script to execute cargo '
                    'locally.')
      register('--release-mode', type=bool, default=False, fingerprint=True,
               help='EXPERIMENTAL: set whether to build cargo artifacts in debug or release mode.')

    def build(self) -> 'Cargo.Factory':
      options = self.get_options()
      return Cargo(
        launcher_path=RelPath(Path(options.launcher_path or 'cargo')),
        release_mode=bool(options.release_mode),
      )

  @property
  def release_mode_subdir(self) -> str:
    return 'release' if self.release_mode else 'debug'

  def create_execute_process_request(
      self,
      cargo_target: CargoTargetAdaptor,
      source_root_stripped_sources: SourceRootStrippedSources,
      command: CargoCommands,
  ) -> ExecuteProcessRequest:
    return ExecuteProcessRequest(
      argv=command.create_cargo_command_argv(self.launcher_path),
      input_files=source_root_stripped_sources.snapshot.directory_digest,
      description=f'Execute cargo to build the request {cargo_target}!',
      env={
        # FIXME: a way to explicitly say "this is a hacky non-remotable process execution", which
        # automatically adds the PATH to the subprocess env!
        'PATH': os.environ['PATH'],
        'MODE': self.release_mode_subdir,
      },
      output_files=(str(cargo_target.binary_name),),
    )


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
class CargoBuildResult:
  snapshot: Snapshot
  binary_path: RelPath

  @property
  def binary_digest(self) -> Digest:
    return self.snapshot.directory_digest

  def __post_init__(self):
    """Assert that the snapshot contains exactly the single output file requested."""
    assert len(self.snapshot.files) == 1
    assert str(self.binary_path.path) == self.snapshot.files[0]


@rule
async def execute_cargo(buildable_target: CargoTargetAdaptor, cargo: Cargo) -> CargoBuildResult:
  stripped_sources = await Get[SourceRootStrippedSources](HydratedTarget(testable_target.address, testable_target))
  exe_res = await Get[ExecuteProcessResult](ExecuteProcessRequest,
                                            cargo.create_execute_process_request(
                                              cargo_target=buildable_target,
                                              source_root_stripped_sources=stripped_sources,
                                              command=CargoCommands.build,
                                            ))
  snapshot = await Get[Snapshot](Digest, exe_res.output_directory_digest)
  return CargoBuildResult(snapshot=snapshot, binary_path=RelPath(buildable_target.binary_name))


@rule
async def execute_cargo_test(testable_target: CargoTargetAdaptor, cargo: Cargo) -> TestResult:
  stripped_sources = await Get[SourceRootStrippedSources](HydratedTarget(testable_target.address, testable_target, ()))
  exe_res = await Get[FallibleExecuteProcessResult](ExecuteProcessRequest,
                                                    cargo.create_execute_process_request(
                                                      cargo_target=testable_target,
                                                      source_root_stripped_sources=stripped_sources,
                                                      command=CargoCommands.test,
                                                    ))
  return TestResult.from_fallible_execute_process_result(exe_res)


class BuildRustOptions(GoalSubsystem):
  """???/build rust binaries!"""
  name = 'build-rust-binary'


class BuildRust(Goal):
  subsystem_cls = BuildRustOptions


# @console_rule
# async def build_rust(console: Console, cargo: Cargo, options: BuildRustOptions) -> BuildRust:
#   return BuildRust(exit_code=0)


def rules():
  return [
    optionable_rule(Cargo.Factory),
    make_cargo,
    RootRule(CargoTargetAdaptor),
    UnionRule(TestTarget, CargoTargetAdaptor),
    filter_cargo_buildable_targets,
    execute_cargo,
    execute_cargo_test,
    # build_rust,
  ]
