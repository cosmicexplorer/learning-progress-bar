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
#![warn(missing_docs)]
/* Make all doctests fail if they produce any warnings. */
#![doc(test(attr(deny(warnings))))]
#![deny(clippy::all)]

use async_trait::async_trait;

use std::time;

pub mod invocation;
pub mod record;

/// States of a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Emission<E, F> {
  /// The stream is still ongoing, and has yielded a value.
  Intermediate(E),
  /// The stream has completed, and has yielded a separate type of value.
  Final(F),
}

/// A stream of values.
#[async_trait]
pub trait Emitter {
  /// The intermediate case.
  type E;
  /// The final case.
  type F;
  /// Yield an intermediate or final value.
  async fn emit(&mut self) -> Emission<Self::E, Self::F>;
}

/// The elapsed time from when a process was invoked to this event being recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeFromStart(pub time::Duration);

/// A timestamped record of an emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Event<E, F> {
  /// Time since the [`EventStamper`] was created.
  pub timestamp: TimeFromStart,
  /// Emission from stream.
  pub emission: Emission<E, F>,
}

/// Timestamp values from a stream.
pub struct EventStamper {
  start_time: time::Instant,
}

impl EventStamper {
  /// Create a timestamper measuring against [`time::Instant::now`].
  pub fn now() -> Self {
    Self {
      start_time: time::Instant::now(),
    }
  }

  /// Record a timestamp for a single value from a stream.
  pub async fn emit_stamped<E>(&self, emitter: &mut E) -> Event<E::E, E::F>
  where E: Emitter {
    let emission = emitter.emit().await;
    Event {
      timestamp: TimeFromStart(self.start_time.elapsed()),
      emission,
    }
  }
}

/// Dummy variable consumed in (empty) `main.rs` as an example of how to import from `lib.rs`.
pub const X: usize = 3;
