from dataclasses import dataclass
from typing import AsyncGenerator


async def event_loop() -> AsyncGenerator:
  yield 3
