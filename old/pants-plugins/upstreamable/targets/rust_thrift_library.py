import os
from pathlib import Path

from pants.base.payload import Payload
from pants.base.payload_field import PrimitiveField
from pants.build_graph.target import Target


class RustThriftLibrary(Target):
  """???/target to be converted into rust thrift sources"""

  # TODO: make this work in engine_initializer.py!
  default_sources_globs = '**/*.thrift'

  @classmethod
  def alias(cls) -> str:
    return 'rust_thrift_library'
