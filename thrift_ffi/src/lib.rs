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
  static ref THRIFT_BUFFER_MAPPING: Arc<Mutex<Interns<InMemoryThriftClient>>> =
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum ClientRequest {
  Monocast(usize),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct ThriftBufferHandle {
  key: InternKey,
}

impl ThriftBufferHandle {
  fn create_single_thrift_client(
    capacity: usize,
  ) -> Result<InMemoryThriftClient, ThriftStreamCreationError> {
    let channel = TBufferChannel::with_capacity(capacity, capacity);
    let (readable, writeable) = channel.split()?;
    Ok(InMemoryThriftClient {
      readable,
      writeable,
    })
  }

  fn intern_new_ffi_client(capacity: usize) -> Result<Self, ThriftStreamCreationError> {
    let client = Self::create_single_thrift_client(capacity)?;
    let key = (*THRIFT_BUFFER_MAPPING).lock().intern(client)?;
    Ok(ThriftBufferHandle { key })
  }

  fn get_thrift_client(
    self,
  ) -> Result<Arc<Mutex<InMemoryThriftClient>>, ThriftStreamCreationError> {
    let interns = (*THRIFT_BUFFER_MAPPING).lock();
    Ok(interns.get(self.key)?)
  }

  fn gc(self) -> UninternResult {
    let mut interns = (*THRIFT_BUFFER_MAPPING).lock();
    interns.garbage_collect(self.key)
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ThriftStreamCreationError(String);

impl From<thrift::Error> for ThriftStreamCreationError {
  fn from(e: thrift::Error) -> Self { ThriftStreamCreationError(format!("{:?}", e)) }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ClientCreationResult {
  Created(ThriftBufferHandle),
  Failed,
}

#[no_mangle]
pub extern "C" fn create_thrift_ffi_client(
  request: *const ClientRequest,
  result: *mut ClientCreationResult,
)
{
  let ret = match unsafe { *request } {
    ClientRequest::Monocast(capacity) => {
      match ThriftBufferHandle::intern_new_ffi_client(capacity) {
        Ok(client) => ClientCreationResult::Created(client),
        Err(_) => ClientCreationResult::Failed,
      }
    },
  };
  unsafe {
    *result = ret;
  }
}

#[repr(C)]
pub enum ClientDestructionResult {
  SuccessfullyFreed,
  DoubleFreed,
}

#[no_mangle]
pub extern "C" fn destroy_thrift_ffi_client(handle: ThriftBufferHandle) -> ClientDestructionResult {
  match handle.gc() {
    UninternResult::SuccessfullyUninterned => ClientDestructionResult::SuccessfullyFreed,
    UninternResult::DoubleFreed => ClientDestructionResult::DoubleFreed,
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

#[no_mangle]
pub extern "C" fn write_buffer_handle(
  handle: ThriftBufferHandle,
  chunk: ThriftChunk,
  result: *mut ThriftWriteResult,
)
{
  let client = if let Ok(client) = handle.get_thrift_client() {
    client
  } else {
    unsafe {
      *result = ThriftWriteResult::Failed;
    }
    return;
  };

  let mut channel = (*client).lock();
  let byte_slice = unsafe {
    let ThriftChunk { ptr, len, .. } = chunk;
    std::slice::from_raw_parts(ptr, len as usize)
  };

  let ret = match channel.writeable.write(byte_slice) {
    Ok(len) => ThriftWriteResult::Written(len as u64),
    Err(_) => ThriftWriteResult::Failed,
  };
  unsafe {
    *result = ret;
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum ThriftReadResult {
  Read(ThriftChunk),
  Failed,
}

#[no_mangle]
pub extern "C" fn read_buffer_handle(
  handle: ThriftBufferHandle,
  mut chunk: ThriftChunk,
  result: *mut ThriftReadResult,
)
{
  let client = if let Ok(client) = handle.get_thrift_client() {
    client
  } else {
    unsafe {
      *result = ThriftReadResult::Failed;
    }
    return;
  };

  let mut channel = (*client).lock();
  let byte_slice = unsafe {
    let ThriftChunk { ptr, len, .. } = chunk;
    std::slice::from_raw_parts_mut(ptr, len as usize)
  };

  let ret = match channel.readable.read(byte_slice) {
    Ok(len) => {
      chunk.len = len as u64;
      ThriftReadResult::Read(chunk)
    },
    Err(_) => ThriftReadResult::Failed,
  };
  unsafe {
    *result = ret;
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
