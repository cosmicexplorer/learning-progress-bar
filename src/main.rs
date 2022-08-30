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
/* Make all doctests fail if they produce any warnings. */
#![doc(test(attr(deny(warnings))))]
#![deny(clippy::all)]

use clap::{Parser, Subcommand, Args};

/// oh?
///
/// oh.
#[derive(Debug, Subcommand)]
enum CliCommand {
  /// a?
  ///
  /// b.
  Wow,

  /// b?
  ///
  /// c.
  ExecuteCli {
    /// The command line to execute.
    ///
    /// asdf?
    argv: Vec<String>,
  },
}

/// A progress bar that uses statistics.
///
/// In particular, uses a Hidden Markov Model.
#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Opts {
  /// Port to use.
  ///
  /// Something else.
  #[clap(short, long, default_value_t = 11111)]
  port: usize,

  #[clap(subcommand)]
  subcommand: CliCommand
}

fn main() {
  let opts = Opts::parse();
  println!("opts: {:?}", opts);
}
