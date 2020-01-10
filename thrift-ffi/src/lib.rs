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
// We only use unsafe pointer dereference in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[macro_use]
pub mod interning;

pub mod lifecycle;

pub mod model {
  use super::interning::*;

  use lazy_static::lazy_static;
  use parking_lot::RwLock;
  use thrift::transport::TBufferChannel;

  use std::{collections::HashMap, sync::Arc};

  #[derive(Debug, Eq, PartialEq, Hash)]
  pub struct User {
    pub id: u64,
  }

  new_handle![UserHandle => USER_HANDLES: Arc<RwLock<Interns<User>>>];

  pub struct UserClient {
    pub channel: TBufferChannel,
  }

  impl UserClient {
    pub fn create_single_buffer_channel(read_capacity: usize, write_capacity: usize) -> UserClient {
      let channel = TBufferChannel::with_capacity(read_capacity, write_capacity);
      UserClient { channel }
    }
  }

  new_handle![UserClientHandle => USER_CLIENT_HANDLES: Arc<RwLock<Interns<UserClient>>>];

  pub struct Topic {
    pub users: HashMap<User, UserClient>,
  }

  new_handle![TopicHandle => TOPIC_HANDLES: Arc<RwLock<Interns<Topic>>>];
}

pub mod transport {
  use super::{interning::*, lifecycle::*, model::*};

  use std::{
    convert::From,
    io::{self, Read, Write},
  };

  #[repr(C)]
  #[derive(Debug, Clone, Copy)]
  pub struct MonocastClient {
    pub read_capacity: u64,
    pub write_capacity: u64,
  }

  #[repr(C)]
  #[derive(Debug)]
  pub enum ClientRequest {
    Monocast(MonocastClient),
  }

  #[derive(Debug, Clone, Eq, PartialEq)]
  pub struct ThriftTransportCreationError(String);

  impl From<InternError> for ThriftTransportCreationError {
    fn from(e: InternError) -> Self { ThriftTransportCreationError(format!("{:?}", e)) }
  }

  impl ExternallyManagedLifecycle<UserClient, UserClientHandle, ThriftTransportCreationError>
    for ClientRequest
  {
    fn make_instance(&self) -> Result<UserClient, ThriftTransportCreationError> {
      let client = match self {
        ClientRequest::Monocast(MonocastClient {
          read_capacity,
          write_capacity,
        }) => UserClient::create_single_buffer_channel(
          *read_capacity as usize,
          *write_capacity as usize,
        ),
      };
      Ok(client)
    }
  }

  #[no_mangle]
  pub extern "C" fn create_thrift_ffi_client(
    request: *const ClientRequest,
  ) -> InternedObjectCreationResult {
    let request = unsafe { &*request };
    ClientRequest::create_handle_ffi(&request)
  }

  #[no_mangle]
  pub extern "C" fn destroy_thrift_ffi_client(key: InternKey) -> InternedObjectDestructionResult {
    ClientRequest::destroy_handle_ffi(key)
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
    handle: &mut UserClientHandle,
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
    handle: *mut UserClientHandle,
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
    handle: &mut UserClientHandle,
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
    handle: *mut UserClientHandle,
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
    use super::super::{interning::*, lifecycle::*, model::*, transport::*};

    #[test]
    fn write_then_read() -> Result<(), ThriftTransportError> {
      let message = "hello! this is a test!".as_bytes();
      let mut handle =
        match ClientRequest::create_handle_ffi(&ClientRequest::Monocast(MonocastClient {
          read_capacity: message.len() as u64,
          write_capacity: message.len() as u64,
        })) {
          InternedObjectCreationResult::Created(key) => UserClientHandle::from_key(key),
          InternedObjectCreationResult::Failed => unreachable!(),
        };

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

      assert_eq!(
        ClientRequest::destroy_handle_ffi(handle.as_key()),
        InternedObjectDestructionResult::Succeeded
      );
      /* Attempting to garbage collect again should fail! */
      assert_eq!(
        ClientRequest::destroy_handle_ffi(handle.as_key()),
        InternedObjectDestructionResult::Failed,
      );
      Ok(())
    }
  }
}
