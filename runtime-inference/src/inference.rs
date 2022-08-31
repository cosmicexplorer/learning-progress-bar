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
  record::{ProgressFraction, RecordLookup},
  TimeFromStart,
};

use std::{hash::Hash, time};

#[derive(Debug, Clone)]
pub struct Weights {
  weights: Vec<f64>,
}

impl Weights {
  pub fn generate(weights: &[f64]) -> Self {
    let sum: f64 = weights.iter().sum();
    if (sum - 1.0).abs() > 1e-10 {
      panic!("invalid weights vector");
    }
    Self {
      weights: weights.iter().cloned().collect(),
    }
  }

  pub fn len(&self) -> usize { self.weights.len() }
}

#[derive(Debug, Copy, Clone)]
pub struct ResultDistribution {
  pub n: usize,
  pub arithmetic_mean: f64,
  pub standard_deviation: f64,
}

impl ResultDistribution {
  /* FIXME: handle floating point nonsense! see https://en.wikipedia.org/wiki/Standard_deviation#Rapid_calculation_methods */
  pub fn calculate(values: &[f64]) -> Option<Self> {
    let n = values.len();
    if n == 0 {
      return None;
    }
    let sum: f64 = values.iter().sum();
    let arithmetic_mean: f64 = sum / (n as f64);
    let squared_deviations_sum: f64 = values
      .iter()
      .map(|x| {
        let dev = x - arithmetic_mean;
        dev.powi(2)
      })
      .sum();
    let standard_deviation: f64 = (squared_deviations_sum / (n as f64)).sqrt();
    Some(Self {
      n,
      arithmetic_mean,
      standard_deviation,
    })
  }

  pub fn weighted_norm(weights: &Weights, dists: &[Self]) -> Self {
    /* FIXME: implement weighted normalization!! */
    if weights.len() != dists.len() {
      panic!("weights must be aligned with number of dists!");
    }
    todo!()
  }
}

pub struct Inferer<E: Hash+Eq> {
  history: RecordLookup<E>,
}

impl<E> Inferer<E>
where E: Hash+Eq
{
  pub fn create(history: RecordLookup<E>) -> Self { Self { history } }

  pub fn calculate_progress_distribution(&self, event: &E) -> Option<ResultDistribution> {
    self
      .history
      .extract_progress_history(event)
      .and_then(|fractions| {
        let values: Vec<f64> = fractions.iter().map(|ProgressFraction(x)| *x).collect();
        ResultDistribution::calculate(&values)
      })
  }
}

#[derive(Debug, Copy, Clone)]
pub struct DecayRate(pub f64);

#[derive(Debug, Copy, Clone)]
pub struct HistoricalReliance(pub f64);

pub struct OutputInterpolator {
  /// Output a weighted average of these parameter estimates across the whole runtime.
  current_run_events: Vec<(TimeFromStart, ResultDistribution)>,
  /// De-weight estimates by this fraction per second.
  decay_rate: DecayRate,
  /// Weight total runtime history by this fraction when composing estimates with current run data.
  historical_reliance: HistoricalReliance,
  /// History of total runtimes for the given process, regardless of individual event timestamps.
  historical_runtimes: Vec<TimeFromStart>,
}

impl OutputInterpolator {
  pub fn create<E>(
    decay_rate: DecayRate,
    historical_reliance: HistoricalReliance,
    history: RecordLookup<E>,
  ) -> Self
  where
    E: Hash+Eq,
  {
    Self {
      decay_rate,
      current_run_events: Vec::new(),
      historical_reliance,
      historical_runtimes: history.extract_runtime_history().to_vec(),
    }
  }

  pub fn accept(&mut self, cur_time: TimeFromStart, dist: ResultDistribution) {
    self.current_run_events.push((cur_time, dist));
  }

  pub fn sample(&self, cur_time: TimeFromStart) -> Option<ResultDistribution> {
    let historical_estimate = self.calculate_historical_distribution(cur_time);
    /* FIXME: implement online estimation with decay rate! */
    let online_estimate = todo!();
    let HistoricalReliance(historical_reliance) = self.historical_reliance;
    historical_estimate.map(|historical_estimate| {
      let historical_weighting =
        Weights::generate(&[historical_reliance, 1.0_f64 - historical_reliance]);
      ResultDistribution::weighted_norm(&historical_weighting, &[
        historical_estimate,
        online_estimate,
      ])
    })
  }

  fn calculate_historical_distribution(
    &self,
    cur_time: TimeFromStart,
  ) -> Option<ResultDistribution> {
    let TimeFromStart(cur_time) = cur_time;
    let progress_fractions: Vec<f64> = self
      .historical_runtimes
      .iter()
      .map(|TimeFromStart(total_time)| ProgressFraction::duration_fraction(*total_time, cur_time))
      .map(|ProgressFraction(x)| x)
      .collect();
    ResultDistribution::calculate(&progress_fractions[..])
  }
}

pub struct InstantaneousProgressEstimate {
  pub estimated_fraction: ProgressFraction,
  pub estimated_total_time: time::Duration,
  pub standard_time_deviation: time::Duration,
}

impl InstantaneousProgressEstimate {
  pub fn calculate(cur_time: TimeFromStart, dist: ResultDistribution) -> Self {
    let TimeFromStart(cur_time) = cur_time;
    let ResultDistribution {
      arithmetic_mean: estimated_fraction,
      standard_deviation,
      ..
    } = dist;

    /* Divide by the progress fraction to get a larger number! */
    let estimated_total_time: time::Duration = cur_time.div_f64(estimated_fraction);
    let standard_time_deviation: time::Duration = estimated_total_time.mul_f64(standard_deviation);

    Self {
      estimated_fraction: ProgressFraction(estimated_fraction),
      estimated_total_time,
      standard_time_deviation,
    }
  }
}
