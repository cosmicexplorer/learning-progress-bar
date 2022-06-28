/*
 * Description: A progress bar that uses statistics.
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

//! A progress bar that uses statistics.

#![deny(rustdoc::missing_crate_level_docs)]
/* #![warn(missing_docs)] */
/* Make all doctests fail if they produce any warnings. */
#![doc(test(attr(deny(warnings))))]
#![deny(clippy::all)]

use super_process::{
  exe,
  stream::{self, Streamable},
};

use async_channel;
use async_trait::async_trait;
use displaydoc::Display;
use thiserror::Error;
use tokio::task;

use std::time;

#[derive(Debug, Display, Error)]
pub enum Error {
  /// command invocation error: {0}
  Command(#[from] super_process::exe::CommandErrorWrapper),
}

#[derive(Debug)]
pub enum Emission<E, F> {
  Intermediate(E),
  Final(F),
}

#[async_trait]
pub trait Emitter {
  type E;
  type F;
  async fn emit(&mut self) -> Emission<Self::E, Self::F>;
}

#[derive(Debug)]
pub struct Event<E, F> {
  pub timestamp: time::Duration,
  pub emission: Emission<E, F>,
}

pub struct EventStamper {
  start_time: time::Instant,
}

impl EventStamper {
  pub fn now() -> Self {
    Self {
      start_time: time::Instant::now(),
    }
  }

  pub async fn emit_stamped<E>(&self, emitter: &mut E) -> Event<E::E, E::F>
  where E: Emitter {
    let emission = emitter.emit().await;
    Event {
      timestamp: self.start_time.elapsed(),
      emission,
    }
  }
}

struct StdioHandler {
  pub sender:
    async_channel::Sender<Emission<stream::StdioLine, Result<(), exe::CommandErrorWrapper>>>,
}

impl StdioHandler {
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

/// Execute a process and convert its outputs into events.
///
///```
/// # fn main() -> Result<(), learning_progress_bar::Error> {
/// # tokio_test::block_on(async {
/// use std::path::PathBuf;
/// use super_process::{fs, exe, stream};
/// use learning_progress_bar::*;
///
/// let command = exe::Command {
///   exe: exe::Exe(fs::File(PathBuf::from("echo"))),
///   argv: ["hey"].as_ref().into(),
///   ..Default::default()
/// };
///
/// let stamper = EventStamper::now();
/// let mut process = BasicProcess::initiate(command).await?;
/// let time1 = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Intermediate(line), timestamp } => {
///     assert_eq!(line, stream::StdioLine::Out("hey".to_string()));
///     timestamp
///   },
///   _ => unreachable!(),
/// };
/// assert!(!time1.is_zero());
/// let time2 = match stamper.emit_stamped(&mut process).await {
///   Event { emission: Emission::Final(result), timestamp } => {
///     assert!(result.is_ok());
///     timestamp
///   },
///   _ => unreachable!(),
/// };
/// assert!(!time2.is_zero());
/// assert!(time2 > time1);
/// # Ok(())
/// # }) // async
/// # }
///```
pub struct BasicProcess {
  receiver:
    async_channel::Receiver<Emission<stream::StdioLine, Result<(), exe::CommandErrorWrapper>>>,
}

impl BasicProcess {
  pub async fn initiate(command: exe::Command) -> Result<Self, Error> {
    let (sender, receiver) = async_channel::unbounded();
    let handle = command.invoke_streaming()?;

    task::spawn(async move {
      let handler = StdioHandler { sender };
      let result = handle
        .exhaust_output_streams_and_wait(|x| handler.handle_line(x))
        .await;
      handler.handle_end(result).await;
    });

    Ok(Self { receiver })
  }
}

#[async_trait]
impl Emitter for BasicProcess {
  type E = stream::StdioLine;
  type F = Result<(), exe::CommandErrorWrapper>;

  async fn emit(&mut self) -> Emission<Self::E, Self::F> {
    let Self { receiver } = self;
    receiver.recv().await.expect("channel should not be closed")
  }
}

pub const X: usize = 3;