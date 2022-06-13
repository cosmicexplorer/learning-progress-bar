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


use std::{
  env, fs,
  io::{self, Write},
  path::PathBuf,
};

use cbindgen;
use regex;

#[derive(Debug)]
pub enum BindingsCreationError {
  IoError(io::Error),
  EnvError(env::VarError),
  CbindgenError(cbindgen::Error),
  StrError(String),
}

impl From<env::VarError> for BindingsCreationError {
  fn from(err: env::VarError) -> Self { BindingsCreationError::EnvError(err) }
}

impl From<io::Error> for BindingsCreationError {
  fn from(err: io::Error) -> Self { BindingsCreationError::IoError(err) }
}

impl From<cbindgen::Error> for BindingsCreationError {
  fn from(err: cbindgen::Error) -> Self { BindingsCreationError::CbindgenError(err) }
}

impl From<String> for BindingsCreationError {
  fn from(err: String) -> Self { Self::StrError(err) }
}

fn build_cbindgen(
  crate_dir: PathBuf,
  config_file: PathBuf,
  env: Environment,
) -> Result<cbindgen::Builder, BindingsCreationError>
{
  let config = cbindgen::Config::from_file(config_file)?;
  let builder = cbindgen::Builder::new()
    .with_crate(crate_dir)
    .with_config(config);
  Ok(match env {
    // CFFI cannot process any C preprocessor directives.
    Environment::CffiCompatible => builder.with_no_includes(),
    Environment::Normal => builder.with_include_guard("__THRIFT_FFI_CBINDGEN_H__"),
  })
}

fn postprocess_bindings_file(
  bindings_file: PathBuf,
  env: Environment,
) -> Result<(), BindingsCreationError>
{
  match env {
    Environment::Normal => (),
    // CFFI doesn't like _0 field names, so we prepend "tup" to all such struct fields,
    // e.g. "tup_0".
    Environment::CffiCompatible => {
      let original_bindings = fs::read_to_string(bindings_file.clone())?;
      let corrected_bindings = regex::Regex::new("(_[0-9]+;)")
        .unwrap()
        .replace_all(&original_bindings, "tup$1");
      fs::File::create(bindings_file)?.write_all(corrected_bindings.as_ref().as_bytes())?;
    },
  }
  Ok(())
}

#[derive(Clone, Copy)]
pub enum Environment {
  Normal,
  CffiCompatible,
}

pub struct GenerateBindingsRequest {
  pub crate_dir: PathBuf,
  pub bindings_file: PathBuf,
  pub config_file: PathBuf,
  pub env: Environment,
}

pub fn generate(request: GenerateBindingsRequest) -> Result<(), BindingsCreationError> {
  let GenerateBindingsRequest {
    crate_dir,
    bindings_file,
    config_file,
    env,
  } = request;

  build_cbindgen(crate_dir, config_file, env)?
    .generate()?
    .write_to_file(bindings_file.clone());

  postprocess_bindings_file(bindings_file, env)?;

  Ok(())
}
