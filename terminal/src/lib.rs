#![deny(warnings)]
// Enable all clippy lints except for many of the pedantic ones. It's a shame this needs to be
// copied and pasted across crates, but there doesn't appear to be a way to include inner attributes
// from a common source.
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
// We only use unsafe pointer derefrences in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod streaming_interface;
mod subprocess_stream;

use std::{collections::HashMap, path::PathBuf};

#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TerminalError(String);

#[repr(C)]
pub struct ProcessExecutionRequest {
  argv: Vec<String>,
  env: HashMap<String, String>,
  cwd: PathBuf,
}

#[no_mangle]
pub extern "C" fn return_string(s: &[u8]) -> &[u8] {
  &(s[0..5])
  /* &s.iter().take(5).cloned().collect::<Vec<_>>() */
}

/* #[repr(C)] */
/* pub struct */

#[no_mangle]
pub extern "C" fn start_subprocess() {}

/* fn start_subprocess_streaming() -> Result<SubprocessIOWrapper, > */

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
