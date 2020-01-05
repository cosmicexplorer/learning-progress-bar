import asyncio
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Any, AsyncGenerator, Optional, Union, cast

import src
import target.debug
from cffi import FFI
from pants.util.contextutil import temporary_file
from pkg_resources import DefaultProvider, ZipProvider, get_provider
from thrift.transport.TTransport import TTransportBase


def get_resource_string(module: ModuleType, rel_path: Path) -> bytes:
  # This technique was taken from pex/pex_builder.py in the pex repo.
  provider: Any = get_provider(module.__name__)
  if not isinstance(provider, DefaultProvider):
    mod = __import__(module.__name__, fromlist=['ignore'])
    provider = ZipProvider(mod)  # type: ignore[call-arg]
  provider: Union[DefaultProvider, ZipProvider] = provider  # type: ignore[no-redef]

  return cast(bytes, provider.get_resource_string(module.__name__, str(rel_path)))


def bootstrap_thrift_ffi() -> FFI:
  ffi = FFI()

  # Load the header file for the bindings.
  generated_bindings = get_resource_string(src, Path('thrift_ffi_bindings.h')).decode('utf-8')
  ffi.cdef(generated_bindings)

  # Load the rust cdylib for the bindings.
  dll = get_resource_string(target.debug, Path('libthrift_ffi.dylib'))
  with temporary_file() as f:
    f.write(dll)
    f.flush()
    lib = ffi.dlopen(f.name)

  return lib


@dataclass(frozen=True)
class FFIBidiTransport(TTransportBase):
  def isOpen(self):
    pass

  def open(self):
    pass

  def close(self):
    pass

  def read(self, sz):
    pass

  def readAll(self, sz):
    buff = b''
    have = 0
    while have < sz:
      chunk = self.read(sz - have)
      chunkLen = len(chunk)
      have += chunkLen
      buff += chunk

      if chunkLen == 0:
        raise EOFError()

    return buff

  def write(self, buf):
    pass

  def flush(self):
    pass


# async def event_loop() -> AsyncGenerator[TerminalEvent]:
#   yield 3


def main() -> None:
  print('hello!')
  ffi = bootstrap_thrift_ffi()
  print(f'ffi = {ffi}')
  ret = ffi.test_str_fn("12345".encode())
  print(f'ret = {ret}')
