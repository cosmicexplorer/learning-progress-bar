/*
 * Description: Retain a persistent and performant event log.
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

//! Retain a persistent and performant event log.
//!
//! The below demonstrates how to use [`RemainingTimeInverter`] and [`TimeLookup`] and checks some
//! basic mathematical relations:
//!```
//! # fn main() -> Result<(), super_process::exe::CommandErrorWrapper> {
//! # tokio_test::block_on(async {
//! use learning_progress_bar::{*, record::*};
//! use async_trait::async_trait;
//! use std::{collections::HashMap, thread, time::Duration};
//!
//! // Create a very simple stream that ends after three emissions.
//! struct ABC {
//!   counter: usize,
//! }
//! impl ABC {
//!   pub fn new() -> Self { Self { counter: 0 } }
//! }
//! #[async_trait]
//! impl Emitter for ABC {
//!   type E = usize;
//!   type F = ();
//!   async fn emit(&mut self) -> Emission<Self::E, Self::F> {
//!     if self.counter > 3 {
//!       Emission::Final(())
//!     } else {
//!       // This test should still pass without this sleep, but it seems more realistic.
//!       thread::sleep(Duration::from_millis(250));
//!       self.counter += 1;
//!       Emission::Intermediate(self.counter)
//!     }
//!   }
//! }
//!
//! // Prepare to capture the output of the stream with timestamps.
//! let stamper = EventStamper::now();
//! let mut abc = ABC::new();
//! let mut inverter = RemainingTimeInverter::<usize>::new();
//!
//! // For the purposes of this test, we retain the original times for each event.
//! let mut matched: HashMap<usize, TimeFromStart> = HashMap::new();
//!
//! // Capture each emission and the corresponding timestamp.
//! let final_time: TimeFromStart;
//! loop {
//!   match stamper.emit_stamped(&mut abc).await {
//!     Event { emission: Emission::Intermediate(counter), timestamp } => {
//!       // Enter the intermediate event into the inverter.
//!       inverter.accept(counter, timestamp);
//!       // Populate this test's table so we can check our work later.
//!       matched.insert(counter, timestamp);
//!     },
//!     Event { emission: Emission::Final(()), timestamp } => {
//!       // Record the timestamp from the final emission, then stop asking it to emit.
//!       final_time = timestamp;
//!       break;
//!     }
//!   }
//! }
//!
//! // Create the inverted time lookup table that would be used for inference.
//! let time_lookup = TimeLookup::invert(final_time, inverter);
//!
//! // For the purposes of this test, we want to verify that whatever the duration actually was
//! // between emissions, that it always adds up to the same total time taken.
//! for (counter, all_remaining_times) in time_lookup.get_events().into_iter() {
//!   assert!(1 == all_remaining_times.len());
//!   let TimeFromStart(from_start) = matched.get(&counter).unwrap();
//!   let RemainingTime(until_end) = all_remaining_times.get(0).unwrap();
//!   // If we've done everything correctly, the time from the start and the time until the end
//!   // should always be equal to the total time of the run.
//!   assert!(final_time == TimeFromStart(*from_start + *until_end));
//! }
//! # Ok(())
//! # }) // async
//! # }
//!```

#[cfg(doc)]
use crate::Emission;
use crate::TimeFromStart;

use std::{collections::HashMap, hash::Hash, time};

/// The elapsed time from this event was recorded to the end of the process invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RemainingTime(pub time::Duration);

/// An in-memory store to record [`Intermediate`](Emission::Intermediate) emissions and their
/// timings from the start of the run.
pub struct RemainingTimeInverter<E: Hash+Eq> {
  events_from_start: HashMap<E, Vec<TimeFromStart>>,
}

impl<E> RemainingTimeInverter<E>
where E: Hash+Eq
{
  /// Create a new event log to record a process invocation.
  pub fn new() -> Self {
    Self {
      events_from_start: HashMap::new(),
    }
  }

  /// While the process is still emitting [`Intermediate`](Emission::Intermediate) emissions, record
  /// each of them here.
  pub fn accept(&mut self, intermediate_emission: E, timestamp: TimeFromStart) {
    self
      .events_from_start
      .entry(intermediate_emission)
      .or_insert_with(Vec::new)
      .push(timestamp);
  }
}

/// An in-memory store to record [`Intermediate`](Emission::Intermediate) emissions and the times
/// after them for the run to complete.
pub struct TimeLookup<E: Hash+Eq> {
  events_from_end: HashMap<E, Vec<RemainingTime>>,
}

impl<E> TimeLookup<E>
where E: Hash+Eq
{
  /// Create a lookup table for remaining time from each event in `inverter` given the `final_time`
  /// timestamp in the single [`Final`](Emission::Final) emission.
  pub fn invert(final_time: TimeFromStart, inverter: RemainingTimeInverter<E>) -> Self {
    let TimeFromStart(final_time) = final_time;
    let mut events_from_end: HashMap<E, Vec<RemainingTime>> = HashMap::new();
    let RemainingTimeInverter { events_from_start } = inverter;

    /* Calculate the distance between the final timestamp and each intermediate timestamp, and
     * record that in a similar in-memory map. */
    for (emission, times_from_start) in events_from_start.into_iter() {
      let events_for_emission = events_from_end.entry(emission).or_insert_with(Vec::new);
      for TimeFromStart(time_from_start) in times_from_start.into_iter() {
        let remaining_time = RemainingTime(final_time - time_from_start);
        events_for_emission.push(remaining_time);
      }
    }

    Self { events_from_end }
  }

  /// **FIXME: THIS IS A TEMPORARY HACK FOR TESTING!**
  pub fn get_events(self) -> HashMap<E, Vec<RemainingTime>> { self.events_from_end }
}
