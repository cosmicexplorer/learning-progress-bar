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

use cbindgen_cffi_compat::*;

use std::{env, fs, path::{Path, PathBuf}};

fn main() -> Result<(), BindingsCreationError> {
  let crate_dir = env::var("CARGO_MANIFEST_DIR")?;

  #[cfg(feature = "cffi-compatible")]
  let env = Environment::CffiCompatible;
  #[cfg(not(feature = "cffi-compatible"))]
  let env = Environment::Normal;

  #[cfg(feature = "pants-injected")]
  fs::copy("streaming_interface.rs", "src/streaming_interface.rs")
    .map_err(|e| format!("thrift copy: {:?}", e))?;

  if !Path::new("generated_headers").exists() {
    fs::create_dir("generated_headers").map_err(|e| format!("generated_headers dir: {:?}", e))?;
  }

  generate(GenerateBindingsRequest {
    crate_dir: PathBuf::from(crate_dir.clone()),
    bindings_file: PathBuf::from("generated_headers/terminal-wrapper-bindings.h"),
    config_file: PathBuf::from("cbindgen.toml"),
    env,
  })?;

  generate(GenerateBindingsRequest {
    crate_dir: PathBuf::from("../thrift-ffi"),
    bindings_file: PathBuf::from("generated_headers/thrift-ffi-bindings.h"),
    config_file: PathBuf::from("../thrift-ffi/cbindgen.toml"),
    env,
  })
}
