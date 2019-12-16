from pants.backend.jvm.targets.jar_library import JarLibrary
from pants.build_graph.build_file_aliases import BuildFileAliases
from pants.build_graph.target import Target

from upstreamable.targets.scala_2_12 import Scala212Deps


def build_file_aliases():
  return BuildFileAliases(
    context_aware_object_factories={
      'scala_2_12_deps': Scala212Deps,
    },
  )
