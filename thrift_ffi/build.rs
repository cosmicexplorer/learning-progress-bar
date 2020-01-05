#![deny(warnings)]
// Enable all clippy lints except for many of the pedantic ones. It's a shame this needs to be copied and pasted across crates, but there doesn't appear to be a way to include inner attributes from a common source.
#![deny(
  clippy::all,
  clippy::default_trait_access,
  clippy::expl_impl_clone_on_copy,
  clippy::if_not_else,
  clippy::needless_continue,
  clippy::single_match_else,
  clippy::unseparated_literal_suffix,
  clippy::used_underscore_binding
)]
// It is often more clear to show that nothing is being moved.
#![allow(clippy::match_ref_pats)]
// Subjective style.
#![allow(
  clippy::len_without_is_empty,
  clippy::redundant_field_names,
  clippy::too_many_arguments
)]
// Default isn't as big a deal as people seem to think it is.
#![allow(clippy::new_without_default, clippy::new_ret_no_self)]
// Arc<Mutex> can be more clear than needing to grok Orderings:
#![allow(clippy::mutex_atomic)]

use std::env;
use std::io;

use cbindgen;

#[derive(Debug)]
enum BindingsCreationError {
  IoError(io::Error),
  EnvError(env::VarError),
  CbindgenError(cbindgen::Error),
}

impl From<env::VarError> for BindingsCreationError {
  fn from(err: env::VarError) -> Self {
    BindingsCreationError::EnvError(err)
  }
}

impl From<io::Error> for BindingsCreationError {
  fn from(err: io::Error) -> Self {
    BindingsCreationError::IoError(err)
  }
}

impl From<cbindgen::Error> for BindingsCreationError {
  fn from(err: cbindgen::Error) -> Self {
    BindingsCreationError::CbindgenError(err)
  }
}

fn main() -> Result<(), BindingsCreationError> {
  let crate_dir = std::env::var("CARGO_MANIFEST_DIR")?;
  cbindgen::generate(crate_dir)?.write_to_file("thrift_ffi_bindings.h");
  Ok(())
}
