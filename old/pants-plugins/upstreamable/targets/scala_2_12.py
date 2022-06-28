from pants.backend.jvm.targets.jar_library import JarLibrary
from pants.backend.jvm.targets.scala_jar_dependency import ScalaJarDependency
from pants.build_graph.build_file_aliases import TargetMacro
from pants.build_graph.target import Target
from pants.java.jar.jar_dependency import JarDependency


class Scala212Deps(TargetMacro):
  """???/provide all the necessary jar_library() dependencies to use scala 2.12!"""

  _FIXED_SCALA_2_12_PATCH_VERSION = '2.12.10'
  _jar_library_target_alias = 'jar_library'
  _pass_through_target_alias = 'target'

  def __init__(self, parse_context):
    self._parse_context = parse_context

  def expand(self, *args, **kwargs):
    deps = []

    scalac = self._parse_context.create_object(
      self._jar_library_target_alias,
      name='scalac',
      jars=[
        JarDependency(
          org='org.scala-lang', name='scala-compiler', rev=self._FIXED_SCALA_2_12_PATCH_VERSION
        )
      ],
    )
    repl = self._parse_context.create_object(
      self._jar_library_target_alias,
      name='scala-repl',
      jars=[
        JarDependency(
          org='com.lihaoyi', name=f'ammonite_{self._FIXED_SCALA_2_12_PATCH_VERSION}', rev='1.8.2'
        )
      ],
    )
    library = self._parse_context.create_object(
      self._jar_library_target_alias,
      name='scala-library',
      jars=[
        JarDependency(
          org='org.scala-lang', name='scala-library', rev=self._FIXED_SCALA_2_12_PATCH_VERSION
        )
      ],
    )

    all_wrapped = self._parse_context.create_object(
      self._pass_through_target_alias,
      name='all-scala-2.12-deps',
      dependencies=[scalac, repl, library],
    )

    return all_wrapped
