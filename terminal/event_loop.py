import asyncio
from dataclasses import dataclass
from typing import AsyncGenerator, Optional

from pants.util.collections import Enum


class EventTypes(Enum):
  start = 'start'
  end = 'end'
  output = 'output'


@dataclass(frozen=True)
class UnixTimestamp:
  """???/absolute time of the event, at lower (1-second) resolution"""
  epoch_seconds: long


@dataclass(frozen=True)
class HighResolutionRelativeTime:
  """???/higher-resolution time relative to the start of the run"""
  milliseconds: long


class OutputTypes(Enum):
  stdout = 'stdout'
  stderr = 'stderr'


@dataclass(frozen=True)
class OutputEvent:
  event_type: OutputTypes
  output_contents: bytes


@dataclass(frozen=True)



@dataclass(frozen=True)
class TerminalEvent:
  event_type: EventTypes
  unix_timestamp: UnixTimestamp
  relative_time: HighResolutionRelativeTime
  output_event: Optional[OutputEvent]


async def event_loop() -> AsyncGenerator[TerminalEvent]:
  yield 3
