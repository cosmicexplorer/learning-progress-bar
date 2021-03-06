from pants.backend.jvm.targets.jar_library import JarLibrary
from pants.build_graph.build_file_aliases import BuildFileAliases
from pants.build_graph.target import Target
from upstreamable.rules.cargo import rules as cargo_rules
from upstreamable.rules.rust_thrift import rules as rust_thrift_rules
from upstreamable.targets.cargo_subproject import CargoSubproject
from upstreamable.targets.rust_thrift_library import RustThriftLibrary
from upstreamable.targets.scala_2_12 import Scala212Deps


def build_file_aliases():
  return BuildFileAliases(
    targets={
      CargoSubproject.alias(): CargoSubproject,
      RustThriftLibrary.alias(): RustThriftLibrary,
    },
    context_aware_object_factories={'scala_2_12_deps': Scala212Deps},
  )


def rules():
  return [*cargo_rules(), *rust_thrift_rules()]
