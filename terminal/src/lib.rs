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

#[cfg(not(feature = "pants-injected"))]
compile_error!("This crate currently requires the \"pants-injected\" feature to be activated!");

pub mod streaming_interface;

use thrift_ffi::{self, ThriftBufferHandle, ThriftChunk};

use regex::Regex;

use std::{io, slice};

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Response {
  Success,
  Failure,
}

#[no_mangle]
pub extern "C" fn write_then_read(
  handle: *mut ThriftBufferHandle,
  mut chunk: &mut ThriftChunk,
) -> Response
{
  let ThriftChunk { ptr, len, capacity } = chunk;
  let in_bytes: &[u8] = unsafe { slice::from_raw_parts(*ptr, *len as usize) };
  let in_string: &str = std::str::from_utf8(in_bytes).unwrap();

  let out_string = Regex::new("e").unwrap().replace_all(in_string, "a");
  let mut out_bytes: &[u8] = (*out_string).as_bytes();
  let out_len = out_bytes.len() as usize;

  /* Ensure we're not writing outside of the available memory. */
  assert!(out_len <= *capacity as usize);

  let mut write_byte_slice: &mut [u8] = unsafe { slice::from_raw_parts_mut(*ptr, out_len) };
  io::copy(&mut out_bytes, &mut write_byte_slice).unwrap();
  chunk.len = out_len as u64;

  let mut handle = unsafe { &mut *handle };
  match thrift_ffi::write_buffer(&mut handle, &chunk)
    .map(|written| {
      assert_eq!(written, chunk.len as usize);
    })
    .and_then(|()| thrift_ffi::read_buffer(&mut handle, &mut chunk))
    .map(|read| {
      /* assert_eq!(read, out_len); */
      /* assert_eq!(read, chunk.len as usize); */
    }) {
    Ok(()) => Response::Success,
    Err(_) => Response::Failure,
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
