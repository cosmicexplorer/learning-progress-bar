import os
from pathlib import Path

from pants.base.payload import Payload
from pants.base.payload_field import PrimitiveField
from pants.build_graph.target import Target


class CargoSubproject(Target):
  """???/target representing a cargo subproject"""

  # TODO: make this work in engine_initializer.py!
  default_sources_globs = ('**/*.rs', '**/*.toml')

  @classmethod
  def alias(cls) -> str:
    return 'cargo_subproject'

  # def __init__(self, output_binary_name: str, payload=None, **kwargs) -> None:
  #   """???"""
  #   subdir = 'debug' if os.environ.get('MODE', None) == 'debug' else 'release'
  #   full_relpath = Path(subdir).resolve(output_binary_name)

  #   payload = payload or Payload()
  #   payload.add_fields({
  #     'output_binary_path': PrimitiveField(full_relpath),
  #   })
  #   super().__init__(payload=payloaod, **kwargs)

  # # TODO: merge the export v2 PR with the changes to consume .v1_target payload fields so this can
  # # be used with v2! See https://github.com/pantsbuild/pants/pull/8760!
  # @property
  # def output_binary_path(self) -> Path:
  #   ret = self.payload.output_binary_path
  #   # TODO: correctly type the results of PayloadFields instead of having to add assertions or
  #   # casts!
  #   assert isinstance(ret, Path)
  #   return ret
