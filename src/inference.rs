/*
 * Description: Perform online inference of remaining time to completion of a process.
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

//! Perform online inference of remaining time to completion of a process.

use crate::{
  record::{RemainingTime, TimeLookup},
  TimeFromStart,
};

use std::hash::Hash;

struct PriorInference {
  timestamp: TimeFromStart,
  inference: RemainingTime,
}

struct PreviousEvents {
  events: Vec<PriorInference>,
}

pub struct Inferer<E: Hash+Eq> {
  time_lookup: TimeLookup<E>,
}

impl<E> Inferer<E>
where E: Hash+Eq
{
  pub fn from_history(time_lookup: TimeLookup<E>) -> Self { Self { time_lookup } }

  fn historical_infer(&self, emission: E) -> Option<RemainingTime> {
    self
      .time_lookup
      .get(emission)
      .map(|previous_results| {
        previous_results.iter().map(|RemainingTime(t)| t).sum() / previous_results.len()
      })
      .map(RemainingTime)
  }

  fn incorporate_prior_inferences(&self, emission: E) -> Option<RemainingTime> {
    todo!()
  }

  /// **TODO: figure out a more reliable model for historical inference (which is able to e.g. guess
  /// a bimodal distribution or more for remaining time given some event, e.g. for events that
  /// always occur more than once)! I suspect that once this is done, the path to incorporating
  /// prior results for the same from the same run will be trivial!**
  pub fn infer_remaining_time(&self, emission: E) -> Option<RemainingTime> {
    todo!()
  }
}
