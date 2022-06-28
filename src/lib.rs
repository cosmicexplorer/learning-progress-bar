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

pub trait Event {
  type Timestamp;
  fn timestamp(&self) -> &Self::Timestamp;
}

#[derive(Debug)]
pub enum Emission<E, F> {
  Intermediate(E),
  Final(F),
}

#[async_trait]
pub trait EventEmitter {
  type E: Event;
  type F;
  async fn emit(&mut self) -> Emission<Self::E, Self::F>;
}

#[derive(Debug)]
pub struct StdoutEvent {
  pub timestamp: time::Duration,
  pub line: stream::StdioLine,
}

impl Event for StdoutEvent {
  type Timestamp = time::Duration;

  fn timestamp(&self) -> &Self::Timestamp { &self.timestamp }
}

struct EventStamper {
  pub start_time: time::Instant,
  pub sender: async_channel::Sender<Emission<StdoutEvent, Result<(), exe::CommandErrorWrapper>>>,
}

impl EventStamper {
  fn stamp_line(&self, line: stream::StdioLine) -> StdoutEvent {
    StdoutEvent {
      timestamp: self.start_time.elapsed(),
      line,
    }
  }

  pub async fn handle_line(&self, line: stream::StdioLine) -> Result<(), exe::CommandError> {
    let event = self.stamp_line(line);
    self
      .sender
      .send(Emission::Intermediate(event))
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
/// let mut process = BasicProcess::initiate(command).await?;
///
/// match process.emit().await {
///   Emission::Intermediate(StdoutEvent { line, .. }) => {
///     assert_eq!(line, stream::StdioLine::Out("hey".to_string()));
///   },
///   _ => unreachable!(),
/// }
/// match process.emit().await {
///   Emission::Final(result) => {
///     assert!(result.is_ok());
///   },
///   _ => unreachable!(),
/// }
/// # Ok(())
/// # }) // async
/// # }
///```
pub struct BasicProcess {
  receiver: async_channel::Receiver<Emission<StdoutEvent, Result<(), exe::CommandErrorWrapper>>>,
}

impl BasicProcess {
  pub async fn initiate(command: exe::Command) -> Result<Self, Error> {
    let (sender, receiver) = async_channel::unbounded();
    let start_time = time::Instant::now();
    let handle = command.invoke_streaming()?;

    task::spawn(async move {
      let stamper = EventStamper { start_time, sender };
      let result = handle
        .exhaust_output_streams_and_wait(|x| stamper.handle_line(x))
        .await;
      stamper.handle_end(result).await;
    });

    Ok(Self { receiver })
  }
}

#[async_trait]
impl EventEmitter for BasicProcess {
  type E = StdoutEvent;
  type F = Result<(), exe::CommandErrorWrapper>;

  async fn emit(&mut self) -> Emission<Self::E, Self::F> {
    let Self { receiver } = self;
    receiver.recv().await.expect("channel should not be closed")
  }
}

pub const X: usize = 3;
