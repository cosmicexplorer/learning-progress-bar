import asyncio
from dataclasses import dataclass
from typing import AsyncGenerator, Optional

from thrift.transport.TTransport import TTransportBase


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


async def event_loop() -> AsyncGenerator[TerminalEvent]:
  yield 3
