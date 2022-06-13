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

pub mod streaming_interface;
/* use streaming_interface::*; */

/* NB: This ensures that our produced cdylib will contain the symbols
 * exported by the thrift-ffi library, such as the create_user() and
 * destroy_user() functions. Our exported library will both:
 * (1) Have whatever FFI we want to export in this lib.rs *for a rust
 * client*! (2) Re-expose the symbols for the base thrift FFI idea from the
 * thrift-ffi crate so that those     can be used e.g. by python cffi!
 */
pub use thrift_ffi::all::*;

pub use zipkin::registration::{set_default_tracing_subscriber, wait_on_flushing};

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
