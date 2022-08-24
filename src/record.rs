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

#[cfg(doc)]
use crate::Event;
use crate::TimeFromStart;

use std::{collections::HashMap, hash::Hash, time};

/// The elapsed time from this event was recorded to the end of the process invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RemainingTime(time::Duration);

/// An in-memory store to record [`Intermediate`](Event::Intermediate) events and their timings from
/// the start of the run.
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

  /// While the process is still emitting [`Intermediate`](Event::Intermediate) events, record each
  /// of them here.
  pub fn accept(&mut self, intermediate_emission: E, timestamp: TimeFromStart) {
    self
      .events_from_start
      .entry(intermediate_emission)
      .or_insert_with(Vec::new)
      .push(timestamp);
  }
}

/// An in-memory store to record [`Intermediate`](Event::Intermediate) events and the times after
/// them for the run to complete.
pub struct TimeLookup<E: Hash+Eq> {
  events_from_end: HashMap<E, Vec<RemainingTime>>,
}

impl<E> TimeLookup<E>
where E: Hash+Eq
{
  /// Create a lookup table for remaining time from each event in `inverter` given the `final_time`
  /// timestamp in the single [`Final`](Event::Final) event.
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
}
