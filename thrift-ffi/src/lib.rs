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
  pub struct User;

  new_handle![UserHandle => USER_HANDLES: Arc<RwLock<Interns<User>>>];

  pub struct UserClient {
    pub user: UserHandle,
    pub topic: TopicHandle,
    pub channel: TBufferChannel,
  }

  impl UserClient {
    pub fn create_single_buffer_channel(
      read_capacity: usize,
      write_capacity: usize,
      user: UserHandle,
      topic: TopicHandle,
    ) -> UserClient
    {
      let channel = TBufferChannel::with_capacity(read_capacity, write_capacity);
      UserClient {
        user,
        topic,
        channel,
      }
    }
  }

  new_handle![UserClientHandle => USER_CLIENT_HANDLES: Arc<RwLock<Interns<UserClient>>>];

  #[derive(Debug)]
  pub struct Topic {
    pub user_client_mapping: HashMap<UserHandle, UserClientHandle>,
  }

  impl Topic {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
      Topic {
        user_client_mapping: HashMap::new(),
      }
    }
  }

  new_handle![TopicHandle => TOPIC_HANDLES: Arc<RwLock<Interns<Topic>>>];
}

pub mod user {
  use super::{interning::*, lifecycle::*, model::*};

  #[repr(C)]
  #[derive(Debug)]
  pub struct UserRequest;

  impl ExternallyManagedLifecycle<User, UserHandle, InternError> for UserRequest {
    fn make_instance(&self) -> Result<User, InternError> { Ok(User) }

    fn register_handle(_handle: &UserHandle) -> Result<(), InternError> { Ok(()) }

    fn deregister_handle(_handle: &UserHandle) -> Result<(), InternError> { Ok(()) }
  }

  #[no_mangle]
  pub extern "C" fn create_user(request: *const UserRequest) -> InternedObjectCreationResult {
    let request = unsafe { &*request };
    UserRequest::create_handle_ffi(&request)
  }

  #[no_mangle]
  pub extern "C" fn destroy_user(key: InternKey) -> InternedObjectDestructionResult {
    UserRequest::destroy_handle_ffi(key)
  }
}

pub mod topic {
  use super::{interning::*, lifecycle::*, model::*};

  #[repr(C)]
  #[derive(Debug)]
  pub struct TopicRequest;

  impl ExternallyManagedLifecycle<Topic, TopicHandle, InternError> for TopicRequest {
    fn make_instance(&self) -> Result<Topic, InternError> { Ok(Topic::new()) }

    fn register_handle(_handle: &TopicHandle) -> Result<(), InternError> { Ok(()) }

    fn deregister_handle(_handle: &TopicHandle) -> Result<(), InternError> { Ok(()) }
  }

  #[no_mangle]
  pub extern "C" fn create_topic(request: *const TopicRequest) -> InternedObjectCreationResult {
    let request = unsafe { &*request };
    TopicRequest::create_handle_ffi(&request)
  }

  #[no_mangle]
  pub extern "C" fn destroy_topic(key: InternKey) -> InternedObjectDestructionResult {
    TopicRequest::destroy_handle_ffi(key)
  }
}

pub mod user_client {
  use super::{interning::*, lifecycle::*, model::*};

  use std::{
    convert::From,
    io::{self, Read, Write},
  };

  #[repr(C)]
  #[derive(Debug, Clone, Copy)]
  pub struct UserClientRequest {
    pub read_capacity: u64,
    pub write_capacity: u64,
    user_key: InternKey,
    topic_key: InternKey,
  }

  impl UserClientRequest {
    fn user(&self) -> UserHandle { UserHandle::from_key(self.user_key) }

    fn topic(&self) -> TopicHandle { TopicHandle::from_key(self.topic_key) }
  }

  #[cfg(test)]
  impl UserClientRequest {
    #[allow(clippy::new_without_default)]
    pub fn new(
      read_capacity: usize,
      write_capacity: usize,
      user: UserHandle,
      topic: TopicHandle,
    ) -> Self
    {
      UserClientRequest {
        read_capacity: read_capacity as u64,
        write_capacity: write_capacity as u64,
        user_key: user.as_key(),
        topic_key: topic.as_key(),
      }
    }
  }

  #[derive(Debug)]
  pub struct UserClientHandleError(String);

  impl From<InternError> for UserClientHandleError {
    fn from(e: InternError) -> Self { UserClientHandleError(format!("{:?}", e)) }
  }

  impl ExternallyManagedLifecycle<UserClient, UserClientHandle, UserClientHandleError>
    for UserClientRequest
  {
    fn make_instance(&self) -> Result<UserClient, UserClientHandleError> {
      let user = self.user();
      let topic = self.topic();

      Ok(UserClient::create_single_buffer_channel(
        self.read_capacity as usize,
        self.write_capacity as usize,
        user,
        topic,
      ))
    }

    fn register_handle(handle: &UserClientHandle) -> Result<(), UserClientHandleError> {
      let user_client_ref = handle.dereference()?;
      let user_client_lock = user_client_ref.lock();

      let UserClient { user, topic, .. } = &*user_client_lock;

      let topic_ref = topic.dereference()?;
      let mut topic_lock = topic_ref.lock();

      if let Some(user_client) = topic_lock.user_client_mapping.get(&user) {
        Err(UserClientHandleError(format!(
          "topic {:?} already contained a client {:?} for user {:?}",
          topic, user_client, &user
        )))
      } else {
        topic_lock.user_client_mapping.insert(*user, *handle);
        Ok(())
      }
    }

    fn deregister_handle(handle: &UserClientHandle) -> Result<(), UserClientHandleError> {
      let user_client_ref = handle.dereference()?;
      let user_client_lock = user_client_ref.lock();

      let UserClient { user, topic, .. } = &*user_client_lock;

      let topic_ref = topic.dereference()?;
      let mut topic_lock = topic_ref.lock();

      if topic_lock.user_client_mapping.remove(&*user).is_some() {
        Ok(())
      } else {
        Err(UserClientHandleError(format!(
          "user {:?} has no client in topic {:?}!",
          user, topic
        )))
      }
    }
  }

  #[no_mangle]
  pub extern "C" fn create_user_client(
    request: *const UserClientRequest,
  ) -> InternedObjectCreationResult {
    let request = unsafe { &*request };
    UserClientRequest::create_handle_ffi(&request)
  }

  #[no_mangle]
  pub extern "C" fn destroy_user_client(key: InternKey) -> InternedObjectDestructionResult {
    UserClientRequest::destroy_handle_ffi(key)
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

  impl From<UserClientHandleError> for ThriftTransportError {
    fn from(e: UserClientHandleError) -> Self { ThriftTransportError(format!("{:?}", e)) }
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
      client.channel.write(byte_slice)?
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
    Read(u64),
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
      client.channel.read(byte_slice)?
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
        ThriftReadResult::Read(chunk.len)
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
  mod user_client {
    use super::super::{interning::*, lifecycle::*, model::*, topic::*, user::*, user_client::*};

    use std::{convert::From, fmt::Debug};

    fn extract_handle<
      T,
      H: Handle<T>+Debug,
      E: From<InternError>+Debug,
      R: ExternallyManagedLifecycle<T, H, E>,
    >(
      request: R,
    ) -> H {
      match R::create_handle_ffi(&request) {
        InternedObjectCreationResult::Created(key) => H::from_key(key),
        InternedObjectCreationResult::Failed => unreachable!(),
      }
    }

    #[test]
    fn invalid_gc_sequence() {
      let user = extract_handle(UserRequest);
      let topic = extract_handle(TopicRequest);

      let handle = extract_handle(UserClientRequest::new(0, 0, user, topic));

      /* Destroy the user and topic. The user client handle should fail to deregister. */
      assert_eq!(
        UserRequest::destroy_handle_ffi(user.as_key()),
        InternedObjectDestructionResult::Succeeded,
      );
      assert_eq!(
        TopicRequest::destroy_handle_ffi(topic.as_key()),
        InternedObjectDestructionResult::Succeeded,
      );

      assert_eq!(
        UserClientRequest::destroy_handle_ffi(handle.as_key()),
        InternedObjectDestructionResult::Failed,
      );
    }

    #[test]
    fn write_then_read() -> Result<(), ThriftTransportError> {
      let user = extract_handle(UserRequest);
      let topic = extract_handle(TopicRequest);

      let message = "hello! this is a test!".as_bytes();
      let mut handle = extract_handle(UserClientRequest::new(
        message.len(),
        message.len(),
        user,
        topic,
      ));

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
        let UserClient {
          ref mut channel, ..
        } = *client;
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
        UserClientRequest::destroy_handle_ffi(handle.as_key()),
        InternedObjectDestructionResult::Succeeded
      );
      /* Attempting to garbage collect again should fail! */
      assert_eq!(
        UserClientRequest::destroy_handle_ffi(handle.as_key()),
        InternedObjectDestructionResult::Failed,
      );

      assert_eq!(
        UserRequest::destroy_handle_ffi(user.as_key()),
        InternedObjectDestructionResult::Succeeded
      );
      assert_eq!(
        TopicRequest::destroy_handle_ffi(topic.as_key()),
        InternedObjectDestructionResult::Succeeded
      );

      Ok(())
    }
  }
}
