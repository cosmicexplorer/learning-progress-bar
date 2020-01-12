import asyncio
from dataclasses import dataclass
from pathlib import Path
from threading import Thread
from types import ModuleType
from typing import Any, AsyncGenerator, Optional, Union, cast

from cffi import FFI
from pants.util.contextutil import temporary_file
from pkg_resources import DefaultProvider, ZipProvider, get_provider
from thrift.protocol.TJsonProtocol import TJsonProtocol, TSimpleJSONProtocolFactory
from thrift.transport.TTransport import TTransportBase, TTransportFactoryBase
from thrift.server.TServer import TSimpleServer

import generated_headers
import target.debug

from terminal.streaming_interface import TerminalWrapper
from terminal.streaming_interface.constants import *
from terminal.streaming_interface.ttypes import *


def get_resource_string(module: ModuleType, rel_path: Path) -> bytes:
  # This technique was taken from pex/pex_builder.py in the pex repo.
  provider: Any = get_provider(module.__name__)
  if not isinstance(provider, DefaultProvider):
    mod = __import__(module.__name__, fromlist=['ignore'])
    provider = ZipProvider(mod)  # type: ignore[call-arg]
  provider: Union[DefaultProvider, ZipProvider] = provider  # type: ignore[no-redef]

  return cast(bytes, provider.get_resource_string(module.__name__, str(rel_path)))


def open_dylib_resource(ffi: FFI, dylib: bytes):
  with temporary_file() as f:
    f.write(dylib)
    f.flush()
    return ffi.dlopen(f.name)


def bootstrap_thrift_ffi() -> FFI:
  ffi = FFI()

  # Load the header files for the bindings. These will be contained in two separate header files,
  # one for the base "thrift-ffi" package, then one generated by the "terminal-wrapper" package.
  thrift_ffi_base_bindings = get_resource_string(
    generated_headers,
    Path('thrift-ffi-bindings.h')
  ).decode('utf-8')
  ffi.cdef(thrift_ffi_base_bindings)

  terminal_wrapper_bindings = get_resource_string(
    generated_headers,
    Path('terminal-wrapper-bindings.h'),
  ).decode('utf-8')
  ffi.cdef(terminal_wrapper_bindings)

  lib = open_dylib_resource(
    ffi,
    get_resource_string(target.debug, Path('libterminal_wrapper.dylib')))

  return ffi, lib


class StreamingInterfaceHandler:

  def __init__(self):
    self._die: bool = False

  def set_die(self):
    self._die = True

  def beginExecution(self, exe_req: ProcessExecutionRequest) -> RunId:
    if self._die:
      raise Exception(f'dying! exe req was: {exe_req}')
    return RunId('asdf')

  def getNextEvent(self) -> SubprocessEvent:
    if self._die:
      raise Exception(f'dying! no next event!')
    return SubprocessEvent(
      type=EventType.START,
      timing=TimingWithinRun(0),
      run_id=RunId('asdf'),
      exit_status=None,
    )


class FFIMonocastTransport(TTransportBase):

  def __init__(self, ffi, lib, user, topic, target_user_kind, read_capacity, write_capacity):
    self._handle = None
    self._ffi = ffi
    self._lib = lib
    self._user = user
    self._topic = topic
    self._target_user_kind = target_user_kind
    self._read_capacity = read_capacity
    self._write_capacity = write_capacity
    self._is_open = False

    self._mutable_chunk = None
    self._max_cap = max(self._read_capacity, self._write_capacity)

    self._cur_read_result = None
    self._cur_write_result = None

  def isOpen(self):
    return self._is_open

  def open(self):
    assert not self._is_open

    with self._ffi.new('UserClientRequest*') as request,\
         self._ffi.new('InternedObjectCreationResult*') as result:
      request = dict(
        read_capacity=self._read_capacity,
        write_capacity=self._write_capacity,
        user_key=self._user,
        topic_key=self._topic,
        target_kind=self._target_user_kind,
      )
      self._lib.create_user_client(request, result)
      result = result[0]
      assert result.tag == self._lib.Created
      self._handle = result.created.tup_0

    self._mutable_chunk = self._ffi.new('ThriftChunk*')
    self._mutable_chunk = dict(
      ptr=self._ffi.new('char[]', self._max_cap),
      len=0,
      capacity=self._max_cap,
    )

    self._cur_read_result = self._ffi.new('ThriftReadResult*')
    self._cur_write_result = self._ffi.new('ThriftWriteResult*')

    self._is_open = True

  def close(self):
    if not self._is_open:
      return

    self._ffi.release(self._mutable_chunk.ptr)
    self._ffi.release(self._mutable_chunk)
    self._mutable_chunk = None

    self._ffi.release(self._cur_read_result)
    self._cur_read_result = None

    self._ffi.release(self._cur_write_result)
    self._cur_write_result = None

    with self._ffi.new('InternedObjectDestructionResult*') as result:
      self._lib.destroy_user_client(self._handle, result)
      result = result[0]
      assert result == self._lib.Succeeded
    self._handle = None

    self._is_open = False

  def _maybe_expand_chunk(self, sz):
    if sz <= self._mutable_chunk.capacity:
      return

    self._max_cap = sz

    self._ffi.release(self._mutable_chunk.ptr)
    self._mutable_chunk = dict(
      ptr=self._ffi.new('char[]', self._max_cap),
      len=0,
      capacity=self._max_cap,
    )

  def read(self, sz):
    assert self._is_open

    self._maybe_expand_chunk(sz)

    self._mutable_chunk.len = sz

    result = self._cur_read_result
    self._lib.receive_topic_message(self._handle, chunk, result)
    assert result.tag == self._lib.Read
    assert result.read <= sz

    return self._ffi.buffer(self._mutable_chunk.ptr, result.read)[:]

  def write(self, buf):
    assert self._is_open

    self._maybe_expand_chunk(len(buf))

    self._mutable_chunk.ptr[0:len(buf)] = buf
    self._mutable_chunk.len = len(buf)

    result = self._cur_write_result
    self._lib.send_topic_message(self._handle, chunk, result)
    assert result.tag == self._lib.Written

    return result.written

  def flush(self):
    # An in-memory buffer has no flush step -- see the docs for the rust TBufferChannel (which this
    # uses) flush() at
    # https://github.com/apache/thrift/blob/master/lib/rs/src/transport/mem.rs#L183-L185!
    pass


# async def event_loop() -> AsyncGenerator[TerminalEvent]:
#   yield 3


def main() -> None:
  print('hello!')

  ffi, lib = bootstrap_thrift_ffi()

  capacity = 3000

  with ffi.new('UserKindRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_user_kind(request, result)
    result = result[0]
    assert result.tag == lib.Created
    client_user_kind = result.created.tup_0

  with ffi.new('UserKindRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_user_kind(request, result)
    result = result[0]
    assert result.tag == lib.Created
    server_user_kind = result.created.tup_0

  with ffi.new('UserRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    request = dict(
      kind=client_user_kind,
    )
    lib.create_user(request, result)
    result = result[0]
    assert result.tag == lib.Created
    client_user = result.created.tup_0

  with ffi.new('UserRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    request = dict(
      kind=server_user_kind,
    )
    lib.create_user(request, result)
    result = result[0]
    assert result.tag == lib.Created
    server_user = result.created.tup_0

  with ffi.new('TopicRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_topic(request, result)
    result = result[0]
    assert result.tag == lib.Created
    topic = result.created.tup_0

  # Taken from: https://thrift.apache.org/tutorial/py.
  handler = StreamingInterfaceHandler()
  processor = TerminalWrapper.Processor()
  server_transport = FFIMonocastTransport(ffi, lib, server_user, topic, client_user_kind,
                                          capacity, capacity)
  tfactory = TTransportFactoryBase()
  pfactory = TSimpleJSONProtocolFactory()
  server = TSimpleServer(processor, transport, tfactory, pfactory)


  server_thread = Thread(target=lambda: server.serve())
  server_thread.start()

  client_transport = FFIMonocastTransport(ffi, lib, client_user, topic, server_user_kind,
                                          capacity, capacity)
  client_protocol = TJsonProtocol(transport)
  client = TerminalWrapper.Client(protocol)


  client_transport.open()
  server_transport.open()

  ret = client.beginExecution(ProcessExecutionRequest())
  assert ret == RunId('asdf')
  print(f'ret = {ret}')

  ret = client.getNextEvent()
  assert ret.run_id == RunId('asdf')
  print(f'ret = {ret}')

  client_transport.close()
  server_transport.close()

  server_thread.join()

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user(client_user, result)
    result = result[0]
    assert result == lib.Succeeded

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user(server_user, result)
    result = result[0]
    assert result == lib.Succeeded

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user_kind(client_user_kind, result)
    result = result[0]
    assert result == lib.Succeeded

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user_kind(server_user_kind, result)
    result = result[0]
    assert result == lib.Succeeded

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_topic(topic_handle, result)
    result = result[0]
    assert result == lib.Succeeded
