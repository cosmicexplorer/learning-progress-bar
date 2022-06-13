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
