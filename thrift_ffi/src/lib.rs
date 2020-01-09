#![feature(associated_type_defaults)]
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
// We only use unsafe pointer dereference in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[macro_use]
pub mod interning;

pub mod model {
  use thrift::transport::TBufferChannel;

  pub struct User;

  pub struct UserClient {
    pub channel: TBufferChannel,
  }

  impl UserClient {
    pub fn create_single_buffer_channel(read_capacity: usize, write_capacity: usize) -> UserClient {
      let channel = TBufferChannel::with_capacity(read_capacity, write_capacity);
      UserClient { channel }
    }
  }

  pub struct Topic {
    pub users: Vec<UserClient>,
  }
}

pub mod transport {
  use super::{interning::*, model::*};

  use lazy_static::lazy_static;
  use parking_lot::RwLock;

  use std::{
    io::{self, Read, Write},
    ops::Drop,
    sync::Arc,
  };

  new_handle![ThriftBufferHandle => THRIFT_BUFFER_MAPPING: Arc<RwLock<Interns<UserClient>>>];

  #[repr(C)]
  #[derive(Debug, Clone, Copy)]
  pub struct MonocastClient {
    pub read_capacity: u64,
    pub write_capacity: u64,
  }

  #[repr(C)]
  #[derive(Debug, Clone, Copy)]
  pub enum ClientRequest {
    Monocast(MonocastClient),
  }

  #[derive(Debug, Clone, Eq, PartialEq)]
  pub struct ThriftTransportCreationError(String);

  impl From<InternError> for ThriftTransportCreationError {
    fn from(e: InternError) -> Self { ThriftTransportCreationError(format!("{:?}", e)) }
  }

  #[repr(C)]
  #[derive(Clone, Copy)]
  pub enum ClientCreationResult {
    Created(*mut ThriftBufferHandle),
    Failed,
  }

  pub fn create_client(
    request: ClientRequest,
  ) -> Result<ThriftBufferHandle, ThriftTransportCreationError> {
    let handle = match request {
      ClientRequest::Monocast(MonocastClient {
        read_capacity,
        write_capacity,
      }) => {
        UserClient::create_single_buffer_channel(read_capacity as usize, write_capacity as usize)
      },
    };
    Ok(ThriftBufferHandle::intern(handle)?)
  }

  #[no_mangle]
  pub extern "C" fn create_thrift_ffi_client(
    request: *const ClientRequest,
    result: *mut ClientCreationResult,
  )
  {
    let ret = match unsafe { create_client(*request) } {
      Ok(client) => {
        /* Box::into_raw() will leak the memory allocated by a Box::new().
         * Box::from_raw() will clean up the memory correctly, as well as call
         * any `Drop` impls! */
        let boxed = Box::new(client);
        ClientCreationResult::Created(Box::into_raw(boxed))
      },
      Err(_) => ClientCreationResult::Failed,
    };
    unsafe {
      *result = ret;
    }
  }

  #[no_mangle]
  pub extern "C" fn destroy_thrift_ffi_client(handle: *mut ThriftBufferHandle) {
    unsafe {
      Box::from_raw(handle);
    }
  }

  #[derive(Debug, Clone, Eq, PartialEq)]
  pub struct ThriftTransportError(String);

  impl From<InternError> for ThriftTransportError {
    fn from(e: InternError) -> Self { ThriftTransportError(format!("{:?}", e)) }
  }

  impl From<String> for ThriftTransportError {
    fn from(e: String) -> Self { ThriftTransportError(e) }
  }

  impl From<io::Error> for ThriftTransportError {
    fn from(e: io::Error) -> Self { ThriftTransportError(format!("{:?}", e)) }
  }

  impl From<ThriftTransportCreationError> for ThriftTransportError {
    fn from(e: ThriftTransportCreationError) -> Self { ThriftTransportError(format!("{:?}", e)) }
  }

  #[repr(C)]
  #[derive(Clone, Copy)]
  pub struct ThriftChunk {
    pub ptr: *mut u8,
    pub len: u64,
    pub capacity: u64,
  }

  #[repr(C)]
  #[derive(Clone, Copy)]
  pub enum ThriftWriteResult {
    Written(u64),
    Failed,
  }

  pub fn write_buffer(
    handle: &mut ThriftBufferHandle,
    chunk: &ThriftChunk,
  ) -> Result<usize, ThriftTransportError>
  {
    let byte_slice = unsafe {
      let ThriftChunk { ptr, len, capacity } = chunk;
      assert!(len <= capacity);
      std::slice::from_raw_parts(*ptr, *len as usize)
    };
    let written = {
      let client_ref = handle.dereference()?;
      let mut client = client_ref.lock();
      let UserClient { ref mut channel } = *client;
      channel.write(byte_slice)?
    };
    assert!(written <= chunk.len as usize);
    Ok(written)
  }

  #[no_mangle]
  pub extern "C" fn write_buffer_handle(
    handle: *mut ThriftBufferHandle,
    chunk: ThriftChunk,
    result: *mut ThriftWriteResult,
  )
  {
    let ret = match unsafe { write_buffer(&mut *handle, &chunk) } {
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

  pub fn read_buffer(
    handle: &mut ThriftBufferHandle,
    chunk: &mut ThriftChunk,
  ) -> Result<usize, ThriftTransportError>
  {
    let byte_slice = unsafe {
      let ThriftChunk { ptr, len, capacity } = chunk;
      assert!(len <= capacity);
      std::slice::from_raw_parts_mut(*ptr, *len as usize)
    };
    let read = {
      let client_ref = handle.dereference()?;
      let mut client = client_ref.lock();
      let UserClient { ref mut channel } = *client;
      channel.read(byte_slice)?
    };
    assert!(read <= chunk.capacity as usize);
    chunk.len = read as u64;
    Ok(read)
  }

  #[no_mangle]
  pub extern "C" fn read_buffer_handle(
    handle: *mut ThriftBufferHandle,
    mut chunk: ThriftChunk,
    result: *mut ThriftReadResult,
  )
  {
    let ret = match unsafe { read_buffer(&mut *handle, &mut chunk) } {
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
}

#[cfg(test)]
mod tests {
  mod transport {
    use super::super::{interning::*, model::*, transport::*};

    #[test]
    fn write_then_read() -> Result<(), ThriftTransportError> {
      let message = "hello! this is a test!".as_bytes();
      let mut handle = create_client(ClientRequest::Monocast(MonocastClient {
        read_capacity: message.len() as u64,
        write_capacity: message.len() as u64,
      }))?;

      let mut copied: Vec<u8> = message.iter().cloned().collect();
      let mut chunk = ThriftChunk {
        ptr: copied.as_mut_ptr(),
        len: copied.len() as u64,
        capacity: copied.len() as u64,
      };

      let written = write_buffer(&mut handle, &chunk)?;
      assert_eq!(written, copied.len());

      /* Copy written bytes to the read buffer so we can be sure to read the exact
       * same bytes back out again. */
      {
        let client_ref = handle.dereference()?;
        let mut client = client_ref.lock();
        let UserClient { ref mut channel } = *client;
        channel.copy_write_buffer_to_read_buffer();
      }

      /* Zero out the message so we can ensure that it is read back in full from
       * the thrift transport. */
      for byte in copied.iter_mut() {
        *byte = 0;
      }
      assert!(message != copied.as_slice());

      let read = read_buffer(&mut handle, &mut chunk)?;
      assert_eq!(read, written);

      assert_eq!(message, copied.as_slice());
      Ok(())
    }
  }
}
