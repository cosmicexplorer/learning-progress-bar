import asyncio
import logging
import sys
import threading
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from types import ModuleType
from typing import Any, AsyncGenerator, Optional, Union, cast

from cffi import FFI
from pants.util.contextutil import temporary_file
from pkg_resources import DefaultProvider, ZipProvider, get_provider
from thrift.protocol.TBinaryProtocol import TBinaryProtocolAccelerated, TBinaryProtocolAcceleratedFactory
from thrift.transport.TTransport import TTransportBase, TTransportFactoryBase, TServerTransportBase
from thrift.server.TServer import TSimpleServer

import generated_headers
import target.debug

from terminal.streaming_interface import TerminalWrapper
from terminal.streaming_interface.constants import *
from terminal.streaming_interface.ttypes import *


logger = logging.getLogger(__name__)


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

  zipkin_bindings = get_resource_string(
    generated_headers,
    Path('zipkin-bindings.h'),
  ).decode('utf-8')
  ffi.cdef(zipkin_bindings)


  lib = open_dylib_resource(
    ffi,
    get_resource_string(target.debug, Path('libterminal_wrapper.dylib')))

  return ffi, lib


class HackedSimpleServer(TSimpleServer):
    """Simple single-threaded server that just pumps around one transport."""

    def __init__(self, *args, **kwargs):
      super().__init__(*args, **kwargs)
      self._is_accepting: bool = False

    # FIXME: The `is_accepting` logic has some super spooky action at a distance!
    def is_accepting(self) -> bool:
      return self._is_accepting

    def serve(self):
      self.serverTransport.listen()
      while True:
        self._is_accepting = True
        client = self.serverTransport.accept()
        if not client:
          continue

        itrans = self.inputTransportFactory.getTransport(client)
        iprot = self.inputProtocolFactory.getProtocol(itrans)

        otrans = self.outputTransportFactory.getTransport(client)
        oprot = self.outputProtocolFactory.getProtocol(otrans)

        try:
          while True:
            self.processor.process(iprot, oprot)
        except TTransport.TTransportException:
          pass
        except Exception as x:
          logger.exception(x)

        self._is_accepting = False

        # import pdb; pdb.set_trace()
        itrans.close()
        if otrans:
          otrans.close()


class StreamingInterfaceHandler:
  def beginExecution(self, exe_req: ProcessExecutionRequest) -> RunId:
    return RunId('asdf')

  def getNextEvent(self) -> SubprocessEvent:
    return SubprocessEvent(
      type=EventType.START,
      timing=TimingWithinRun(0),
      run_id=RunId('asdf'),
      exit_status=None,
    )


class FFIMulticastTransport(TTransportBase):

  class ServerTransportFactory(TServerTransportBase):
    """Pass on constructor args to an underlying transport, created when .accept() is called!"""

    def __init__(self, ffi, lib, user, target_user_kind, topic, *args, **kwargs) -> None:
      self._ffi = ffi
      self._lib = lib

      self._user = user
      self._topic = topic
      self._target_user_kind = target_user_kind

      self._args = args
      self._kwargs = kwargs

    def listen(self) -> None:
      pass

    def accept(self):
      # import pdb; pdb.set_trace()
      assert self._topic is not None
      # raise Exception(f'args: {self._args}, kwargs: {self._kwargs}')
      ret = FFIMulticastTransport(
        ffi=self._ffi,
        lib=self._lib,
        user=self._user,
        topic=self._topic,
        target_user_kind=self._target_user_kind,
        *self._args,
        **self._kwargs)
      ret.open()
      return ret

    def close(self) -> None:
      pass

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

    self._mutable_read_chunk = None
    self._intermediary_write_chunk = None
    self._max_cap = max(self._read_capacity, self._write_capacity)

    self._cur_read_result = None
    self._cur_write_result = None

  def isOpen(self):
    return self._is_open

  def _zero_out_mutable_read_chunk(self):
    assert self._mutable_read_chunk is not None
    self._mutable_read_chunk[0] = dict(
      ptr=self._ffi.NULL,
      len=0,
      capacity=0,
    )

  @contextmanager
  def _with_chunk_from_buf(self, buf):
    with self._ffi.from_buffer(buf) as data:
      self._write_chunk[0] = dict(
        ptr=data,
        len=len(buf),
        capacity=len(buf),
      )
      print(f'buf: {buf} / data: {data}', file=sys.stderr)
      print(f'write_chunk: {self._write_chunk.ptr}', file=sys.stderr)
      yield self._write_chunk

  def open(self):
    assert not self._is_open

    self._handle = self._ffi.new('InternKey*')

    self._write_chunk = self._ffi.new('ThriftChunk*')

    with self._ffi.new('UserClientRequest*') as request,\
         self._ffi.new('InternedObjectCreationResult*') as result:
      request[0] = dict(
        read_capacity=self._read_capacity,
        write_capacity=self._write_capacity,
        user_key=self._user,
        topic_key=self._topic,
        target_kind_key=self._target_user_kind,
      )
      self._lib.create_user_client(request, result)
      assert result.tag == self._lib.Created
      self._handle[0] = result.created.tup_0

    self._mutable_read_chunk = self._ffi.new('ThriftChunk*')
    self._zero_out_mutable_read_chunk()

    self._cur_read_result = self._ffi.new('ThriftReadResult*')
    self._cur_write_result = self._ffi.new('ThriftWriteResult*')

    self._is_open = True

  def close(self):
    assert self._is_open

    # FIXME: Figure out why cffi says this can't be released!!
    if self._mutable_read_chunk.ptr != self._ffi.NULL:
      try:
        self._ffi.release(self._mutable_read_chunk.ptr)
      except ValueError:
        pass

    self._ffi.release(self._mutable_read_chunk)
    self._mutable_read_chunk = None

    self._ffi.release(self._cur_read_result)
    self._cur_read_result = None

    self._ffi.release(self._cur_write_result)
    self._cur_write_result = None

    self._ffi.release(self._write_chunk)
    self._write_chunk = None

    with self._ffi.new('InternedObjectDestructionResult*') as result:
      self._lib.destroy_user_client(self._handle[0], result)
      assert result == self._lib.Succeeded
    self._ffi.release(self._handle)
    self._handle = None

    self._is_open = False

  def _maybe_expand_chunk(self, sz):
    if sz <= self._mutable_read_chunk.capacity:
      return

    self._max_cap = sz

    # FIXME: Figure out why cffi says this can't be released!!
    if self._mutable_read_chunk.ptr != self._ffi.NULL:
      try:
        self._ffi.release(self._mutable_read_chunk.ptr)
      except ValueError:
        pass

    self._mutable_read_chunk[0] = dict(
      ptr=self._ffi.new('char[]', self._max_cap),
      len=0,
      capacity=self._max_cap,
    )

  def read(self, sz):
    assert self._is_open

    self._maybe_expand_chunk(sz)

    self._mutable_read_chunk.len = sz

    result = self._cur_read_result
    print(f'({self._user.tup_0}) begin read of size {sz}', file=sys.stderr)
    self._lib.receive_topic_messages(self._handle[0], self._mutable_read_chunk[0], result)
    print(f'({self._user.tup_0}) end read of size {sz}', file=sys.stderr)
    assert result.tag == self._lib.Read
    read = result.read.tup_0
    assert read <= sz

    buf = self._ffi.buffer(self._mutable_read_chunk.ptr, read)[:]
    print(f'read={read}, buf={buf}')

    return buf

  def write(self, buf):
    assert self._is_open

    with self._with_chunk_from_buf(buf) as chunk:
      result = self._cur_write_result
      print(f'({self._user.tup_0}) begin write of size {len(buf)}', file=sys.stderr)
      self._lib.send_topic_messages(self._handle[0], chunk[0], result)
      print(f'({self._user.tup_0}) end write of size {len(buf)}', file=sys.stderr)
      assert result.tag == self._lib.Written

    return result.written

  def flush(self):
    assert self._is_open
    # An in-memory buffer has no flush step -- see the docs for the rust TBufferChannel (which this
    # uses) flush() at
    # https://github.com/apache/thrift/blob/master/lib/rs/src/transport/mem.rs#L183-L185!
    pass


# async def event_loop() -> AsyncGenerator[TerminalEvent]:
#   yield 3


def main() -> None:
  print('hello!')

  ffi, lib = bootstrap_thrift_ffi()

  lib.set_default_tracing_subscriber()

  capacity = 3000

  client_user_kind = ffi.new('InternKey*')
  with ffi.new('UserKindRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_user_kind(request, result)
    assert result.tag == lib.Created
    client_user_kind[0] = result.created.tup_0

  server_user_kind = ffi.new('InternKey*')
  with ffi.new('UserKindRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_user_kind(request, result)
    assert result.tag == lib.Created
    server_user_kind[0] = result.created.tup_0

  client_user = ffi.new('InternKey*')
  with ffi.new('UserRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    request[0] = dict(
      kind=client_user_kind[0],
    )
    lib.create_user(request, result)
    assert result.tag == lib.Created
    client_user[0] = result.created.tup_0

  server_user = ffi.new('InternKey*')
  with ffi.new('UserRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    request[0] = dict(
      kind=server_user_kind[0],
    )
    lib.create_user(request, result)
    assert result.tag == lib.Created
    server_user[0] = result.created.tup_0

  topic = ffi.new('InternKey*')
  with ffi.new('TopicRequest*') as request,\
       ffi.new('InternedObjectCreationResult*') as result:
    lib.create_topic(request, result)
    assert result.tag == lib.Created
    topic[0] = result.created.tup_0

  # Taken from: https://thrift.apache.org/tutorial/py.
  handler = StreamingInterfaceHandler()
  processor = TerminalWrapper.Processor(handler)
  server_transport_factory = FFIMulticastTransport.ServerTransportFactory(
    ffi=ffi,
    lib=lib,
    user=server_user[0],
    target_user_kind=client_user_kind[0],
    topic=topic[0],
    read_capacity=capacity,
    write_capacity=capacity)

  # lib.wait_on_flushing()
  # import sys; sys.exit()

  # This "Base" class merely returns any transport it's provided. Since we are using in-memory
  # buffers, it seems like a good idea for now to avoid further buffering to get closer to native
  # C-ABI FFI-like performance (at first!).
  tfactory = TTransportFactoryBase()
  pfactory = TBinaryProtocolAcceleratedFactory()
  server = HackedSimpleServer(processor, server_transport_factory, tfactory, pfactory)

  client_transport = FFIMulticastTransport(
    ffi, lib, client_user[0], topic[0], server_user_kind[0],
    capacity, capacity)
  client_transport.open()

  client_protocol = TBinaryProtocolAccelerated(client_transport)
  client = TerminalWrapper.Client(client_protocol)

  def server_fun():
    server.serve()

  server_thread = threading.Thread(target=server_fun)
  server_thread.start()

  ret = client.beginExecution(ProcessExecutionRequest())
  assert ret == RunId('asdf')
  print(f'ret = {ret}')

  ret = client.getNextEvent()
  assert ret.run_id == RunId('asdf')
  print(f'ret = {ret}')

  client_transport.close()

  server_thread.join()

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_topic(topic[0], result)
    assert result == lib.Succeeded
  ffi.release(topic)

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user(client_user[0], result)
    assert result == lib.Succeeded
  ffi.release(client_user)

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user(server_user[0], result)
    assert result == lib.Succeeded
  ffi.release(server_user)

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user_kind(client_user_kind[0], result)
    assert result == lib.Succeeded
  ffi.release(client_user_kind)

  with ffi.new('InternedObjectDestructionResult*') as result:
    lib.destroy_user_kind(server_user_kind[0], result)
    assert result == lib.Succeeded
  ffi.release(server_user_kind)
