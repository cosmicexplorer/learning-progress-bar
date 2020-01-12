/* NB: Any nightly-only features go here >=]! */
#![feature(vec_into_raw_parts)]
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

/* NB: See the note in the `terminal` crate's `lib.rs` file about why
 * exporting all of these symbols is necessary for any downstream rust
 * crates, because the cdylibs that dependees export will all need to have
 * this crate's symbols *also* exported on top of whatever *other* FFI they
 * may want to have. */
pub mod all {
  pub use super::{interning::*, lifecycle::*, topic::*, user::*, user_client::*, util::*};
}

pub mod util;

#[macro_use]
pub mod interning;

pub mod lifecycle;

pub mod user {
  use super::{interning::*, lifecycle::*};

  /* FIXME: The new_handle![] macro doesn't create a new scope to locally
   * import everything it uses to define the '*Handle' structs! This is super
   * hacky, but ALSO FINE FOR NOW!!! */
  use lazy_static::lazy_static;
  use parking_lot::RwLock;

  use std::sync::Arc;

  #[derive(Debug, Eq, PartialEq, Hash)]
  pub struct User;

  /* FIXME: The '*Handle' types created by new_handle![] don't appear to be
   * exported by cbindgen!! This requires methods like send_topic_message()
   * to accept InternKey instances, which each method converts by itself. */
  new_handle![UserHandle => USER_HANDLES: Arc<RwLock<Interns<User>>>];

  #[repr(C)]
  #[derive(Debug)]
  pub struct UserRequest;

  impl ExternallyManagedLifecycle<User, UserHandle, InternError> for UserRequest {
    fn make_instance(&self) -> Result<User, InternError> { Ok(User) }

    fn register_handle(_handle: &UserHandle) -> Result<(), InternError> { Ok(()) }

    fn deregister_handle(_handle: &UserHandle) -> Result<(), InternError> { Ok(()) }
  }

  #[no_mangle]
  pub extern "C" fn create_user(
    request: *const UserRequest,
    result: *mut InternedObjectCreationResult,
  )
  {
    let request = unsafe { &*request };
    let ret = UserRequest::create_handle_ffi(&request);
    unsafe {
      *result = ret;
    }
  }

  #[no_mangle]
  pub extern "C" fn destroy_user(key: InternKey, result: *mut InternedObjectDestructionResult) {
    let ret = UserRequest::destroy_handle_ffi(key);
    unsafe {
      *result = ret;
    }
  }
}

pub mod topic {
  use super::{interning::*, lifecycle::*, user::*, user_client::*};

  /* FIXME: The new_handle![] macro doesn't create a new scope to locally
   * import everything it uses to define the '*Handle' structs! This is super
   * hacky, but ALSO FINE FOR NOW!!! */
  use lazy_static::lazy_static;
  use parking_lot::RwLock;

  use std::{collections::HashMap, sync::Arc};

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
  use super::{interning::*, lifecycle::*, topic::*, user::*, util::*};

  /* FIXME: The new_handle![] macro doesn't create a new scope to locally
   * import everything it uses to define the '*Handle' structs! This is super
   * hacky, but ALSO FINE FOR NOW!!! */
  use lazy_static::lazy_static;
  use parking_lot::RwLock;
  use thrift::transport::TBufferChannel;

  use std::{
    convert::From,
    io::{self, Read, Write},
    sync::Arc,
  };

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
      let (user, mut topic) = handle
        .extract::<(UserHandle, TopicHandle), UserClientHandleError, _>(
          |UserClient { user, topic, .. }| Ok((*user, *topic)),
        )?;

      let topic_str = format!("{:?}", topic);
      topic.extract_mut(&mut move |Topic {
                                     user_client_mapping,
                                   }| {
        if let Some(user_client) = user_client_mapping.get(&user) {
          Err(UserClientHandleError(format!(
            "topic {:?} already contained a client {:?} for user {:?}",
            topic_str, user_client, &user
          )))
        } else {
          user_client_mapping.insert(user, *handle);
          Ok(())
        }
      })
    }

    fn deregister_handle(handle: &UserClientHandle) -> Result<(), UserClientHandleError> {
      let (user, mut topic) = handle
        .extract::<(UserHandle, TopicHandle), UserClientHandleError, _>(
          |UserClient { user, topic, .. }| Ok((*user, *topic)),
        )?;

      let topic2 = topic.clone();
      topic.extract_mut(&mut move |Topic {
                                     user_client_mapping,
                                   }| {
        if user_client_mapping.remove(&user).is_some() {
          Ok(())
        } else {
          Err(UserClientHandleError(format!(
            "user {:?} has no client in topic {:?}!",
            user, topic2
          )))
        }
      })
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

  pub fn client_send_message(
    handle: UserClientHandle,
    chunk: &ThriftChunk,
  ) -> Result<usize, ThriftTransportError>
  {
    /* Interpret the incoming chunk as a message to send to the other clients. */
    let ThriftChunk { ptr, len, capacity } = chunk;
    let len = *len as usize;
    assert!(len <= *capacity as usize);
    let byte_slice = unsafe { std::slice::from_raw_parts(*ptr, len) };

    /* Get the topic, and message every other client *synchronously*! */
    let (user, topic) = handle.extract::<(UserHandle, TopicHandle), UserClientHandleError, _>(
      |UserClient { user, topic, .. }| Ok((*user, *topic)),
    )?;

    /* Temporarily lock the topic handle to get all the other clients to message! */
    /* FIXME(PERFORMANCE): This could be cached! */
    let other_clients = topic.extract::<Vec<UserClientHandle>, UserClientHandleError, _>(
      |Topic {
         user_client_mapping,
       }| {
        Ok(
          user_client_mapping
            .iter()
            .filter(|(other_user, _)| user != **other_user)
            .map(|(_, other_user_client)| *other_user_client)
            .collect(),
        )
      },
    )?;

    /* FIXME(CONCURRENCY): what happens if a UserClientHandle is deregistered in
     * between the creation of `other_clients` and this for loop? A spurious
     * error?! */
    /* TODO(PERFORMANCE): This iteration should/could all be in parallel! */
    let (_, write_errors): (_, Vec<_>) =
      other_clients
        .into_iter()
        .split_result_sequence_mut(|mut c| {
          c.extract_mut(&mut |UserClient {
                                channel,
                                user: other_user,
                                ..
                              }| {
            let written = channel.write(byte_slice)?;
            assert!(written <= len);
            /* Find all writes which succeeded, but didn't write as many bytes as we
             * wanted to. */
            /* FIXME: For now we convert these into errors. It's not immediately clear
             * what the right behavior should be for the topic-based multicast model
             * currently used here. */
            if written < len {
              Err(ThriftTransportError(format!(
                "wrote fewer bytes to user {:?} on topic {:?} than expected ({:?} vs {:?})",
                other_user, topic, written, len
              )))
            } else {
              Ok(())
            }
          })
        });

    if write_errors.is_empty() {
      /* If no error, return the expected number of bytes. */
      Ok(len)
    } else {
      /* There are currently many reasons why writes may fail. We currently just
       * log it and move on! */
      use itertools::Itertools;
      Err(ThriftTransportError(format!(
        "errors writing:\n{:?}",
        write_errors.into_iter().format("\n")
      )))
    }
  }

  #[no_mangle]
  pub extern "C" fn send_topic_message(handle: InternKey, chunk: ThriftChunk) -> ThriftWriteResult {
    match client_send_message(UserClientHandle::from_key(handle), &chunk) {
      Ok(len) => ThriftWriteResult::Written(len as u64),
      Err(e) => {
        eprintln!("send_topic_message: {:?}", e);
        ThriftWriteResult::Failed
      },
    }
  }

  #[repr(C)]
  #[derive(Clone, Copy)]
  pub enum ThriftReadResult {
    Read(u64),
    Failed,
  }

  pub fn client_receive_message(
    mut handle: UserClientHandle,
    chunk: &mut ThriftChunk,
  ) -> Result<usize, ThriftTransportError>
  {
    /* The incoming chunk will contain any message(s) from other client(s). */
    let ThriftChunk { ptr, len, capacity } = chunk;
    let len = *len as usize;
    assert!(len <= *capacity as usize);
    let byte_slice = unsafe { std::slice::from_raw_parts_mut(*ptr, len) };

    /* We do *not* lock the topic or any other handles here! We simply read from
     * our read buffer, and we copy over the contents of the write buffer if
     * we haven't read enough! */
    /* Please check out the docs at
     * https://docs.rs/thrift/0.0.4/thrift/transport/struct.TBufferChannel.html for more info on the
     * curious API of the TBufferChannel object (<3 thrift though!!). */
    handle.extract_mut(&mut |UserClient { channel, .. }| {
      let read_initial = channel.read(byte_slice)?;
      Ok(if read_initial < len {
        /* FIXME(CONCURRENCY): Does this *necessarily* mean that the read buffer was
         * emptied? What is the method to check that here (since we are about
         * to overwriite whatever's left of the read buffer)! */
        assert!(!channel.bytes().any(|_| true));
        /* If the read buffer is now indeed empty, let's copy everything from the
         * write buffer! */
        channel.copy_write_buffer_to_read_buffer();
        let read_after_copying_from_write_buffer = channel.read(&mut byte_slice[read_initial..])?;
        read_initial + read_after_copying_from_write_buffer
      } else {
        read_initial
      })
    })
  }

  #[no_mangle]
  pub extern "C" fn receive_topic_message(
    handle: InternKey,
    mut chunk: ThriftChunk,
  ) -> ThriftReadResult
  {
    match client_receive_message(UserClientHandle::from_key(handle), &mut chunk) {
      Ok(len) => {
        chunk.len = len as u64;
        ThriftReadResult::Read(chunk.len)
      },
      Err(e) => {
        eprintln!("receive_topic_message: {:?}", e);
        ThriftReadResult::Failed
      },
    }
  }
}

#[cfg(test)]
mod tests {
  mod user_client {
    use super::super::all::*;

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

      /* Destroy the user and topic. The user client handle should fail to
       * deregister. */
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

    fn create_new_user_with_client_for_topic(
      capacity: usize,
      topic: TopicHandle,
      message: &[u8],
    ) -> (UserHandle, UserClientHandle, ThriftChunk)
    {
      assert!(capacity >= message.len());

      /* Create new user! */
      let user = extract_handle(UserRequest);
      /* Connect the new user to the same topic! */
      let user_client = extract_handle(UserClientRequest::new(capacity, capacity, user, topic));

      let chunk = {
        let mut copied: Vec<u8> = message.iter().cloned().collect();
        copied.reserve(capacity);
        let (ptr, len, new_capacity) = copied.into_raw_parts();
        assert_eq!(len, message.len());
        assert!(new_capacity >= capacity);
        ThriftChunk {
          ptr,
          len: len as u64,
          capacity: new_capacity as u64,
        }
      };

      (user, user_client, chunk)
    }

    fn stringify_bytes(bytes: &[u8]) -> &str { std::str::from_utf8(bytes).unwrap() }

    fn zero_out_thrift_chunk(chunk: &mut ThriftChunk, message: &[u8]) {
      let ThriftChunk { ptr, len, capacity } = chunk;
      assert!(len <= capacity);
      assert_eq!(*len as usize, message.len());
      let byte_slice: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(*ptr, *len as usize) };
      assert_eq!(
        byte_slice,
        message,
        "slice was: {}\nmsg was: {}",
        stringify_bytes(byte_slice),
        stringify_bytes(message),
      );
      for byte in byte_slice.iter_mut() {
        *byte = 0;
      }
      assert_ne!(byte_slice, message);
    }

    fn validate_client_received_message_consume_chunk(
      chunk: &ThriftChunk,
      expected_message: &[u8],
    )
    {
      let ThriftChunk { ptr, len, capacity } = chunk;
      assert!(len <= capacity);
      let read_bytes: &[u8] = unsafe { std::slice::from_raw_parts(*ptr, *len as usize) };
      assert_eq!(
        read_bytes,
        expected_message,
        "{}/{}",
        stringify_bytes(read_bytes),
        stringify_bytes(expected_message),
      );
    }

    ///
    /// This test vaguely maps out the current communication model. Two 'User's
    /// connect to a single 'Topic' (each thereby creating their own
    /// 'UserClient' connected to the 'Topic'). All messages sent with
    /// 'client_send_message()` are broadcast to all other participants in the
    /// 'Topic', which is then read back in 'client_receive_message()'.
    ///
    /// In this case, the two users are writing their own payload, then reading
    /// the other's payload as they each exchange a single write and read to
    /// the same topic.
    ///
    /// TODO: All of the logic that is being tested here relies on synchronous
    /// execution of the statements in this test. It would be good to think
    /// early on about how/whether to generalize that.
    #[test]
    fn write_then_read() -> Result<(), ThriftTransportError> {
      let capacity: usize = 400;

      let topic = extract_handle(TopicRequest);

      let message1 = "hello! this is a test!".as_bytes();
      let (user1, user_client1, mut chunk1) =
        create_new_user_with_client_for_topic(capacity, topic, &message1);

      /* NB: Using the same topic!! */
      let message2 = "hello! also a test!".as_bytes();
      let (user2, user_client2, mut chunk2) =
        create_new_user_with_client_for_topic(capacity, topic, &message2);

      let written1 = client_send_message(user_client1, &chunk1)?;
      assert_eq!(written1, chunk1.len as usize);

      let written2 = client_send_message(user_client2, &chunk2)?;
      assert_eq!(written2, chunk2.len as usize);

      /* Zero out the original message so we can ensure that it is read back in full from the thrift
       * transport. */
      zero_out_thrift_chunk(&mut chunk1, &message1);
      zero_out_thrift_chunk(&mut chunk2, &message2);

      /* We know how much we expect to be reading -- we will be reading the content
       * of the other user client's message. To signal how much to (try to)
       * read in client_receive_message(), we set the `len` field of the
       * ThriftChunk that we pass in to the function. */
      chunk1.len = message2.len() as u64;
      let read1 = client_receive_message(user_client1, &mut chunk1)?;
      assert_eq!(read1, written2);
      /* Same for the other user client. */
      chunk2.len = message1.len() as u64;
      let read2 = client_receive_message(user_client2, &mut chunk2)?;
      assert_eq!(read2, written1);

      /* Match the chunk contents with the expected messages after reading them! */
      validate_client_received_message_consume_chunk(&chunk1, &message2);
      validate_client_received_message_consume_chunk(&chunk2, &message1);

      assert_eq!(
        UserClientRequest::destroy_handle_ffi(user_client1.as_key()),
        InternedObjectDestructionResult::Succeeded
      );
      /* Attempting to garbage collect again should fail! */
      assert_eq!(
        UserClientRequest::destroy_handle_ffi(user_client1.as_key()),
        InternedObjectDestructionResult::Failed,
      );

      assert_eq!(
        UserClientRequest::destroy_handle_ffi(user_client2.as_key()),
        InternedObjectDestructionResult::Succeeded
      );

      assert_eq!(
        UserRequest::destroy_handle_ffi(user1.as_key()),
        InternedObjectDestructionResult::Succeeded
      );
      assert_eq!(
        UserRequest::destroy_handle_ffi(user2.as_key()),
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
