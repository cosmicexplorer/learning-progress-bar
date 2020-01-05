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

mod interning;

use interning::*;

use lazy_static::lazy_static;
use parking_lot::Mutex;
use thrift::{
  self,
  transport::{ReadHalf, TBufferChannel, TIoChannel, WriteHalf},
};

use std::{
  fmt,
  io::{Read, Write},
  sync::Arc,
};

lazy_static! {
  static ref THRIFT_BUFFER_MAPPING: Arc<Mutex<Interns<BidiThriftClient>>> =
    Arc::new(Mutex::new(Interns::new()));
}

pub struct InMemoryThriftClient {
  pub readable: ReadHalf<TBufferChannel>,
  pub writeable: WriteHalf<TBufferChannel>,
}

impl fmt::Debug for InMemoryThriftClient {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    write!(f, "InMemoryThriftClient(...)")
  }
}

#[derive(Clone, Debug)]
struct BidiThriftClient {
  pub ffi_language_writable: Arc<Mutex<InMemoryThriftClient>>,
  pub rust_writable: Arc<Mutex<InMemoryThriftClient>>,
}

/* FIXME: slim this down so that:
1. this type is the only FFI object,
2. this type represents a memory buffer usable for thrift communication! */
#[repr(C)]
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct ThriftBufferHandle {
  key: InternKey,
}

impl ThriftBufferHandle {
  fn create_single_thrift_channel(
    capacity: usize,
  ) -> Result<InMemoryThriftClient, ThriftStreamCreationError> {
    let channel = TBufferChannel::with_capacity(capacity, capacity);
    let (readable, writeable) = channel.split()?;
    Ok(InMemoryThriftClient {
      readable,
      writeable,
    })
  }

  fn create_bidi_ffi_client(capacity: usize) -> Result<Self, ThriftStreamCreationError> {
    let ffi_language_writable = Arc::new(Mutex::new(Self::create_single_thrift_channel(capacity)?));
    let rust_writable = Arc::new(Mutex::new(Self::create_single_thrift_channel(capacity)?));
    let client = BidiThriftClient {
      ffi_language_writable,
      rust_writable,
    };
    let key = (*THRIFT_BUFFER_MAPPING).lock().intern(client)?;
    Ok(ThriftBufferHandle { key })
  }

  fn get_thrift_client(&self) -> Result<BidiThriftClient, ThriftStreamCreationError> {
    let interns = (*THRIFT_BUFFER_MAPPING).lock();
    let ret = interns.get(self.key)?;
    Ok(ret.clone())
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ThriftStreamCreationError(String);

impl From<thrift::Error> for ThriftStreamCreationError {
  fn from(e: thrift::Error) -> Self { ThriftStreamCreationError(format!("{:?}", e)) }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ThriftFFIClientCreationResult {
  Created(ThriftBufferHandle),
  Failed,
}

pub extern "C" fn make_buffer_handle(capacity: usize) -> ThriftFFIClientCreationResult {
  match ThriftBufferHandle::create_bidi_ffi_client(capacity) {
    Ok(client) => ThriftFFIClientCreationResult::Created(client),
    Err(_) => ThriftFFIClientCreationResult::Failed,
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ThriftChunk {
  ptr: *mut u8,
  len: u64,
  capacity: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ThriftWriteResult {
  Written(u64),
  Failed,
}

pub extern "C" fn write_buffer_handle(
  handle: ThriftBufferHandle,
  chunk: ThriftChunk,
) -> ThriftWriteResult
{
  let client = match handle.get_thrift_client() {
    Ok(client) => client,
    Err(_) => {
      return ThriftWriteResult::Failed;
    },
  };

  let mut channel = (*client.ffi_language_writable).lock();
  let byte_slice = unsafe {
    let ThriftChunk { ptr, len, .. } = chunk;
    std::slice::from_raw_parts(ptr, len as usize)
  };

  match channel.writeable.write(byte_slice) {
    Ok(len) => ThriftWriteResult::Written(len as u64),
    Err(_) => ThriftWriteResult::Failed,
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ThriftReadResult {
  Read(ThriftChunk),
  Failed,
}

pub extern "C" fn read_buffer_handle(
  handle: ThriftBufferHandle,
  mut chunk: ThriftChunk,
) -> ThriftReadResult
{
  let client = match handle.get_thrift_client() {
    Ok(client) => client,
    Err(_) => {
      return ThriftReadResult::Failed;
    },
  };

  let mut channel = (*client.ffi_language_writable).lock();
  let byte_slice = unsafe {
    let ThriftChunk { ptr, len, .. } = chunk;
    std::slice::from_raw_parts_mut(ptr, len as usize)
  };

  match channel.readable.read(byte_slice) {
    Ok(len) => {
      chunk.len = len as u64;
      ThriftReadResult::Read(chunk)
    },
    Err(_) => ThriftReadResult::Failed,
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
