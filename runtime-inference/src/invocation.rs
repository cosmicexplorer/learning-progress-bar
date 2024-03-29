/*
 * Description: Extract streams of text or bytes from subprocesses.
 *
 * Copyright (C) 2022 Danny McClanahan <dmcC2@hypnicjerk.ai>
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published
 * by the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

//! Extract streams of text or bytes from subprocesses.

use crate::{Emission, Emitter};

use async_trait::async_trait;

/// Execute a process and convert its output lines into string events.
///
///```
/// # fn main() -> Result<(), super_process::exe::CommandErrorWrapper> {
/// # tokio_test::block_on(async {
/// use super_process::{exe, stream};
/// use runtime_inference::{*, invocation::lines::*};
///
/// let command = exe::Command {
///   exe: exe::Exe::from(&"echo"),
///   argv: ["hey"].as_ref().into(),
///   ..Default::default()
/// };
///
/// let stamper = EventStamper::now();
/// let mut process = StringProcess::initiate(command).await?;
/// let TimeFromStart(time1) = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Intermediate(stream::StdioLine::Out(line)), timestamp } => {
///     // Line endings are stripped from each line.
///     // Currently only '\n' newlines are supported.
///     assert_eq!(line, "hey");
///     timestamp
///   },
///   _ => unreachable!(),
/// };
/// assert!(!time1.is_zero());
/// let TimeFromStart(time2) = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Final(Ok(())), timestamp } => timestamp,
///   _ => unreachable!(),
/// };
/// assert!(!time2.is_zero());
/// assert!(time2 > time1);
/// # Ok(())
/// # }) // async
/// # }
///```
pub mod lines {
  use super::*;
  use super_process::{
    exe,
    stream::{self, Streamable},
  };

  use async_channel;
  use tokio::task;

  struct StdioLineHandler {
    pub sender:
      async_channel::Sender<Emission<stream::StdioLine, Result<(), exe::CommandErrorWrapper>>>,
  }

  impl StdioLineHandler {
    pub async fn handle_line(&self, line: stream::StdioLine) -> Result<(), exe::CommandError> {
      self
        .sender
        .send(Emission::Intermediate(line))
        .await
        .expect("channel should not be closed");
      Ok(())
    }

    pub async fn handle_end(self, result: Result<(), exe::CommandErrorWrapper>) {
      let Self { sender, .. } = self;
      match result {
        Ok(()) => sender
          .send(Emission::Final(Ok(())))
          .await
          .expect("should not be closed"),
        Err(e) => sender
          .send(Emission::Final(Err(e)))
          .await
          .expect("should not be closed"),
      }
    }
  }


  /// Receive events from lines of stdout/stderr after decoding to UTF-8.
  pub struct StringProcess {
    receiver:
      async_channel::Receiver<Emission<stream::StdioLine, Result<(), exe::CommandErrorWrapper>>>,
  }

  impl StringProcess {
    /// Invoke `command`, read its outputs by line, then check its exit status, all in a background
    /// task from [`task::spawn`].
    ///
    /// Events get processed in [`Self::emit`] via an [`async_channel::unbounded`] queue.
    pub async fn initiate(command: exe::Command) -> Result<Self, exe::CommandErrorWrapper> {
      let (sender, receiver) = async_channel::unbounded();
      let handle = command.invoke_streaming()?;

      task::spawn(async move {
        let handler = StdioLineHandler { sender };
        let result = handle
          .exhaust_string_streams_and_wait(|x| handler.handle_line(x))
          .await;
        handler.handle_end(result).await;
      });

      Ok(Self { receiver })
    }
  }

  #[async_trait]
  impl Emitter for StringProcess {
    type E = stream::StdioLine;
    type F = Result<(), exe::CommandErrorWrapper>;

    async fn emit(&mut self) -> Emission<Self::E, Self::F> {
      let Self { receiver } = self;
      receiver.recv().await.expect("channel should not be closed")
    }
  }
}

/// Execute a process and convert its outputs into byte events.
///
///```
/// # fn main() -> Result<(), super_process::exe::CommandErrorWrapper> {
/// # tokio_test::block_on(async {
/// use super_process::{exe, stream};
/// use runtime_inference::{*, invocation::bytes::*};
///
/// let command = exe::Command {
///   exe: exe::Exe::from(&"echo"),
///   argv: ["hey"].as_ref().into(),
///   ..Default::default()
/// };
///
/// let stamper = EventStamper::now();
/// let mut process = BytesProcess::initiate(command).await?;
/// let TimeFromStart(time1) = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Intermediate(stream::StdioChunk::Out(chunk)), timestamp } => {
///     // Byte chunks are not split at any consistent length or boundary.
///     // In this case, we simply assume the entire output is short enough to fit in one chunk.
///     assert_eq!(chunk, b"hey\n");
///     timestamp
///   },
///   _ => unreachable!(),
/// };
/// assert!(!time1.is_zero());
/// let TimeFromStart(time2) = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Final(Ok(())), timestamp } => timestamp,
///   _ => unreachable!(),
/// };
/// assert!(!time2.is_zero());
/// assert!(time2 > time1);
/// # Ok(())
/// # }) // async
/// # }
///```
pub mod bytes {
  use super::*;
  use super_process::{
    exe,
    stream::{self, Streamable},
  };

  use async_channel;
  use tokio::task;

  struct StdioChunkHandler {
    pub sender:
      async_channel::Sender<Emission<stream::StdioChunk, Result<(), exe::CommandErrorWrapper>>>,
  }

  impl StdioChunkHandler {
    pub async fn handle_chunk(&self, chunk: stream::StdioChunk) -> Result<(), exe::CommandError> {
      self
        .sender
        .send(Emission::Intermediate(chunk))
        .await
        .expect("channel should not be closed");
      Ok(())
    }

    pub async fn handle_end(self, result: Result<(), exe::CommandErrorWrapper>) {
      let Self { sender, .. } = self;
      match result {
        Ok(()) => sender
          .send(Emission::Final(Ok(())))
          .await
          .expect("should not be closed"),
        Err(e) => sender
          .send(Emission::Final(Err(e)))
          .await
          .expect("should not be closed"),
      }
    }
  }

  /// Receive events from byte chunks of stdout/stderr, without decoding into text.
  pub struct BytesProcess {
    receiver:
      async_channel::Receiver<Emission<stream::StdioChunk, Result<(), exe::CommandErrorWrapper>>>,
  }

  impl BytesProcess {
    /// Invoke `command`, read its outputs without decoding to UTF-8, then check its exit status,
    /// all in a background task from [`task::spawn`].
    ///
    /// Events get processed in [`Self::emit`] via an [`async_channel::unbounded`] queue.
    pub async fn initiate(command: exe::Command) -> Result<Self, exe::CommandErrorWrapper> {
      let (sender, receiver) = async_channel::unbounded();
      let handle = command.invoke_streaming()?;

      task::spawn(async move {
        let handler = StdioChunkHandler { sender };
        let result = handle
          .exhaust_byte_streams_and_wait(|x| handler.handle_chunk(x))
          .await;
        handler.handle_end(result).await;
      });

      Ok(Self { receiver })
    }
  }

  #[async_trait]
  impl Emitter for BytesProcess {
    type E = stream::StdioChunk;
    type F = Result<(), exe::CommandErrorWrapper>;

    async fn emit(&mut self) -> Emission<Self::E, Self::F> {
      let Self { receiver } = self;
      receiver.recv().await.expect("channel should not be closed")
    }
  }
}
