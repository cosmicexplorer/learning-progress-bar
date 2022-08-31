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
use crate::Emission;
use crate::TimeFromStart;

use indexmap::{IndexMap, IndexSet};

use std::{hash::Hash, time};

#[derive(Debug, Clone, Copy)]
pub struct ProgressFraction(pub f64);

impl ProgressFraction {
  pub fn duration_fraction(total_time: time::Duration, event_time: time::Duration) -> Self {
    Self(event_time.div_duration_f64(total_time))
  }
}

/// An in-memory store to record [`Intermediate`](Emission::Intermediate) emissions and their
/// timings from the start of the run.
pub struct ElapsedTimeTracker<E> {
  events: Vec<(E, TimeFromStart)>,
}

impl<E> ElapsedTimeTracker<E> {
  /// Create a new event log to record a process invocation.
  pub fn new() -> Self { Self { events: Vec::new() } }

  /// While the process is still emitting [`Intermediate`](Emission::Intermediate) emissions, record
  /// each of them here.
  pub fn accept(&mut self, intermediate_emission: E, timestamp: TimeFromStart) {
    self.events.push((intermediate_emission, timestamp));
  }

  pub fn into_events(self) -> Vec<(E, TimeFromStart)> {
    let Self { events } = self;
    events
  }
}

/* TODO: dump this to/read this from file!! */
pub struct RecordLookup<E>
where E: Hash+Eq
{
  event_progress: IndexMap<E, Vec<ProgressFraction>>,
  total_runtimes: Vec<TimeFromStart>,
}

impl<E> RecordLookup<E>
where E: Hash+Eq
{
  /// ???/Create a lookup table for remaining time from each event in `tracker` given the
  /// `final_time` timestamp in the single [`Final`](Emission::Final) emission.
  pub fn invert(final_time: TimeFromStart, tracker: ElapsedTimeTracker<E>) -> Self {
    let TimeFromStart(final_time) = final_time;
    let mut event_progress: IndexMap<E, Vec<ProgressFraction>> = IndexMap::new();

    /* ???/Calculate the distance between the final timestamp and each intermediate timestamp, and
     * record that in a similar in-memory map. */
    for (emission, TimeFromStart(time_from_start)) in tracker.into_events().into_iter() {
      let progress_fraction = ProgressFraction::duration_fraction(final_time, time_from_start);
      let events_for_emission = event_progress.entry(emission).or_insert_with(Vec::new);
      events_for_emission.push(progress_fraction);
    }

    Self {
      event_progress,
      total_runtimes: vec![TimeFromStart(final_time)],
    }
  }

  pub fn extract_progress_history(&self, event: &E) -> Option<&[ProgressFraction]> {
    self.event_progress.get(event).map(|x| &x[..])
  }

  pub fn extract_runtime_history(&self) -> &[TimeFromStart] { &self.total_runtimes[..] }
}

impl<E> RecordLookup<E>
where E: Hash+Eq+Clone
{
  pub fn merge(self, other: Self) -> Self {
    /* NB: we allow having some events in one and not in another. We still need to figure out what
     * that means though (does that imply one has missing data?). */
    let all_events: IndexSet<&E> = self
      .event_progress
      .keys()
      .chain(other.event_progress.keys())
      .collect();
    let mut event_progress: IndexMap<E, Vec<ProgressFraction>> = all_events
      .into_iter()
      .map(|event| {
        let left: &[ProgressFraction] = self
          .event_progress
          .get(event)
          .map(|x| &x[..])
          .unwrap_or(&[]);
        let right: &[ProgressFraction] = other
          .event_progress
          .get(event)
          .map(|x| &x[..])
          .unwrap_or(&[]);
        let merged: Vec<ProgressFraction> = left.iter().chain(right.iter()).cloned().collect();
        (event.clone(), merged)
      })
      .collect();
    let total_runtimes: Vec<_> = self
      .total_runtimes
      .iter()
      .chain(other.total_runtimes.iter())
      .cloned()
      .collect();
    Self {
      event_progress,
      total_runtimes,
    }
  }
}
