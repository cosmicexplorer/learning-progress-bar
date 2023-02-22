/*
 * Description: A progress bar that uses statistics.
 *
 * Copyright (C) 2022-2023 Danny McClanahan <dmcC2@hypnicjerk.ai>
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

use runtime_inference::{invocation::lines::*, *};
use super_process::{exe, stream};

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use clap::{Args, Parser, Subcommand};
use rand::{thread_rng, CryptoRng, Rng};
use tokio;

use std::{
  fs::{File, OpenOptions},
  io::{self, Write},
  path::PathBuf,
};

/// oh?
///
/// oh.
#[derive(Debug, Subcommand)]
enum CliCommand {
  /// b?
  ///
  /// c.
  ExecuteCli {
    /// Where to write event logging information to.
    ///
    /// This file will contain a database of timestamped events.
    #[clap(short, long, parse(from_os_str), default_value = "-")]
    log_file: PathBuf,

    /// The command line to execute.
    argv: Vec<String>,
  },
}

/// A progress bar that uses statistics.
#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Opts {
  #[clap(subcommand)]
  subcommand: CliCommand,
}

#[tokio::main]
async fn main() -> Result<(), exe::CommandErrorWrapper> {
  let Opts { subcommand } = Opts::parse();
  println!("subcommand: {:?}", subcommand);

  match subcommand {
    CliCommand::ExecuteCli { log_file, argv } => {
      /* (1) Invoke a timestamped process. */
      let exe = exe::Exe::from(&argv[0]);
      let argv: exe::Argv = argv[1..].to_vec().into();

      let id: [u8; 10] = thread_rng().gen();
      let id = Base64Display::new(&id, &STANDARD);

      let command = exe::Command {
        exe,
        argv,
        ..Default::default()
      };
      let stamper = EventStamper::now();
      let mut process = StringProcess::initiate(command.clone()).await?;

      /* (2) Append timestamped events to the log file. */
      let mut log_file: Box<dyn io::Write> = match log_file.to_string_lossy().as_ref() {
        "-" => Box::new(io::stdout()),
        log_file => Box::new(
          OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file.clone())
            .expect("open log file"),
        ),
      };

      log_file
        .write_fmt(format_args!("{}@ [COMMAND] {:?}\n", id, command))
        .expect("command line log");

      while match stamper.emit_stamped(&mut process).await {
        Event {
          emission,
          timestamp: TimeFromStart(time),
        } => match emission {
          Emission::Intermediate(stdio_line) => {
            match stdio_line {
              stream::StdioLine::Out(line) => {
                log_file
                  .write_fmt(format_args!("{}@ {:?}: [STDOUT] {}\n", id, time, line))
                  .expect("stdout");
              },
              stream::StdioLine::Err(line) => {
                log_file
                  .write_fmt(format_args!("{}@ {:?}: [STDERR] {}\n", id, time, line))
                  .expect("stderr");
              },
            };
            true
          },
          Emission::Final(r) => {
            match r {
              Ok(()) => {
                log_file
                  .write_fmt(format_args!("{}@ {:?}: [EXIT-SUCCESS]\n", id, time))
                  .expect("exit-success");
              },
              Err(e) => {
                log_file
                  .write_fmt(format_args!("{}@ {:?}: [EXIT-ERR] {:?}\n", id, time, e))
                  .expect("exit-err");
              },
            };
            false
          },
        },
      } { /* spin */ }

      /* (3) Figure out how to incorporate the progress indicator (carriage return?). */
    },
  }

  Ok(())
}
