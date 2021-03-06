/* NB: Any nightly-only features go here >=]! */
#![feature(vec_into_raw_parts)]
#![feature(proc_macro_hygiene)]
#![feature(get_mut_unchecked)]
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
  pub use super::{
    interning::*, lifecycle::*, topic::*, user::*, user_client::*, user_kind::*, util::*,
  };
}

pub mod util;

#[macro_use]
pub mod interning;

pub mod lifecycle;

pub mod user_kind {
  use super::{interning::*, lifecycle::*};

  #[derive(Debug)]
  pub struct UserKind;

  new_handle![UserKindHandle => USER_KIND_HANDLES: Arc<RwLock<Interns<UserKind>>>];

  #[repr(C)]
  #[derive(Debug)]
  pub struct UserKindRequest;

  impl ExternallyManagedLifecycle<UserKind, UserKindHandle, InternError> for UserKindRequest {
    fn make_instance(&self) -> Result<UserKind, InternError> { Ok(UserKind) }

    fn register_handle(_handle: &UserKindHandle) -> Result<(), InternError> { Ok(()) }

    fn deregister_handle(_handle: &UserKindHandle) -> Result<(), InternError> { Ok(()) }
  }

  #[no_mangle]
  pub extern "C" fn create_user_kind(
    request: *const UserKindRequest,
    result: *mut InternedObjectCreationResult,
  )
  {
    let request = unsafe { &*request };
    let ret = UserKindRequest::create_handle_ffi(&request);
    unsafe {
      *result = ret;
    }
  }

  #[no_mangle]
  pub extern "C" fn destroy_user_kind(
    key: InternKey,
    result: *mut InternedObjectDestructionResult,
  )
  {
    let ret = UserKindRequest::destroy_handle_ffi(key);
    unsafe {
      *result = ret;
    }
  }
}

pub mod user {
  use super::{interning::*, lifecycle::*, user_kind::*};

  #[derive(Debug)]
  pub struct User {
    pub kind: UserKindHandle,
  }

  /* FIXME: The '*Handle' types created by new_handle![] don't appear to be
   * exported by cbindgen!! This requires methods like send_topic_messages()
   * to accept InternKey instances, which each method converts by itself. */
  new_handle![UserHandle => USER_HANDLES: Arc<RwLock<Interns<User>>>];

  #[repr(C)]
  #[derive(Debug)]
  pub struct UserRequest {
    kind: InternKey,
  }

  #[cfg(test)]
  impl UserRequest {
    #[allow(clippy::new_without_default)]
    pub fn new(kind: InternKey) -> Self { UserRequest { kind } }
  }

  impl ExternallyManagedLifecycle<User, UserHandle, InternError> for UserRequest {
    fn make_instance(&self) -> Result<User, InternError> {
      Ok(User {
        kind: UserKindHandle::from_key(self.kind),
      })
    }

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

  use std::collections::HashMap;

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
  pub extern "C" fn create_topic(
    request: *const TopicRequest,
    result: *mut InternedObjectCreationResult,
  )
  {
    let request = unsafe { &*request };
    let ret = TopicRequest::create_handle_ffi(&request);
    unsafe {
      *result = ret;
    }
  }

  #[no_mangle]
  pub extern "C" fn destroy_topic(key: InternKey, result: *mut InternedObjectDestructionResult) {
    let ret = TopicRequest::destroy_handle_ffi(key);
    unsafe {
      *result = ret;
    }
  }
}

pub mod user_client {
  use super::{interning::*, lifecycle::*, topic::*, user::*, user_kind::*, util::*};
  use coroutines::*;

  use thrift::transport::TBufferChannel;

  use std::{
    convert::From,
    fmt::{self, Debug},
    io::{self, Read, Write},
    sync::Arc,
  };

  #[derive(Debug)]
  pub struct ThriftBufferWrapper {
    pub inner: TBufferChannel,
  }

  impl Read for ThriftBufferWrapper {
    fn read(&mut self, byte_slice: &mut [u8]) -> io::Result<usize> { self.inner.read(byte_slice) }
  }

  impl Write for ThriftBufferWrapper {
    fn write(&mut self, byte_slice: &[u8]) -> io::Result<usize> { self.inner.write(byte_slice) }

    fn flush(&mut self) -> io::Result<()> { self.inner.flush() }
  }

  impl ReadWriteBufferable for ThriftBufferWrapper {
    fn read_has_any(&self) -> bool { self.inner.read_has_any() }

    fn write_is_full(&self) -> bool { self.inner.write_is_full() }

    fn write_has_any(&self) -> bool { self.inner.write_has_any() }

    fn copy_write_buffer_to_read_buffer(&mut self) {
      self.inner.copy_write_buffer_to_read_buffer();
    }
  }

  #[derive(Debug)]
  pub struct UserClient {
    pub user: UserHandle,
    pub topic: TopicHandle,
    pub target_kind: UserKindHandle,
    pub transport_state: SyncRwBuf<SynchronizedReadWriteBuffer<ThriftBufferWrapper>>,
  }

  impl UserClient {
    pub fn create_single_buffer_channel(
      read_capacity: usize,
      write_capacity: usize,
      user: UserHandle,
      topic: TopicHandle,
      target_kind: UserKindHandle,
    ) -> UserClient
    {
      let channel = TBufferChannel::with_capacity(read_capacity, write_capacity);
      UserClient {
        user,
        topic,
        target_kind,
        transport_state: SyncRwBuf::new(Arc::new(SynchronizedReadWriteBuffer::new(
          ThriftBufferWrapper { inner: channel },
        ))),
      }
    }

    pub fn other_matching_clients(&self) -> Result<Vec<UserClientHandle>, UserClientHandleError> {
      let UserClient {
        user,
        topic,
        target_kind,
        ..
      } = self;

      topic.extract(
        |Topic {
           user_client_mapping,
         }| {
          let mut matching_clients: Vec<UserClientHandle> = vec![];
          for (other_user, other_user_client) in user_client_mapping.iter() {
            let other_user_kind = other_user.get(|u| u.kind)?;
            if *other_user != *user && other_user_kind == *target_kind {
              matching_clients.push(*other_user_client);
            }
          }
          dbg!(&matching_clients);
          Ok(matching_clients)
        },
      )
    }
  }

  impl Read for UserClient {
    ///
    /// Load a message of the desired size into the buffer. If that fails,
    #[tracing::instrument]
    fn read(&mut self, byte_slice: &mut [u8]) -> io::Result<usize> {
      /* eprintln!("self: {:?} / read: {:?}", &self, &byte_slice); */
      self.transport_state.read(byte_slice)
    }
  }

  impl Write for UserClient {
    ///
    /// (1) Get the other clients from the same topic which match the currewnt
    /// client's target kind. (2) Write the entire message to each matched
    /// other client, waiting on its `write_cvar` to be     notified in a
    /// separate thread of execution. (3) Try to complete all of the writes
    /// that are possible. Return a merged error message of     all errors
    /// writing to the matching other clients.
    #[tracing::instrument]
    fn write(&mut self, byte_slice: &[u8]) -> io::Result<usize> {
      /* eprintln!("self; {:?} / write: {:?}", &self, &byte_slice); */
      /* Get clients to write to. */
      let matching_clients = self
        .other_matching_clients()
        .map_err(|e| e.into_io_error())?;
      /* eprintln!( */
      /* "self: {:?}, matching_clients: {:?}", */
      /* &self, &matching_clients */
      /* ); */

      /* Write the *entire* message to *all* matching clients *synchronously*!!! */
      matching_clients
        .into_iter()
        .handle_split_result_sequence::<UserClientHandleError, _>(|other_handle| {
          /* dbg!(&other_handle); */
          let mut other_transport = other_handle.get(|c| c.get().transport_state.clone())?;
          /* dbg!(&other_transport); */
          let written = other_transport.write(&byte_slice)?;
          assert_eq!(written, byte_slice.len());
          Ok(())
        })
        .map_err(|e| UserClientHandleError(e).into_io_error())?;
      Ok(byte_slice.len())
    }

    ///
    /// Reach into all matching other clients in the same topic, and wait on
    /// each of their read buffers being *completely* processed.
    #[tracing::instrument]
    fn flush(&mut self) -> io::Result<()> {
      let matching_clients = self
        .other_matching_clients()
        .map_err(|e| e.into_io_error())?;
      /* eprintln!( */
      /* "self: {:?}, matching_clients: {:?}", */
      /* &self, &matching_clients */
      /* ); */

      matching_clients
        .into_iter()
        .handle_split_result_sequence::<UserClientHandleError, _>(|other_handle| {
          let mut other_transport = other_handle.get(|c| c.get().transport_state.clone())?;
          other_transport.flush()?;
          Ok(())
        })
        .map_err(|e| UserClientHandleError(e).into_io_error())
    }
  }

  unsafe impl SyncRwAccessable for UserClient {}

  #[derive(Clone, Debug)]
  pub struct UserClientWrapper(Arc<UserClient>);

  impl UserClientWrapper {
    pub fn new(inner: UserClient) -> Self { UserClientWrapper(Arc::new(inner)) }

    pub fn get(&self) -> &UserClient { &*self.0 }

    pub fn into_sync_buf(self) -> SyncRwBuf<UserClient> {
      /* eprintln!("into_sync_buf: {:?}", &self); */
      SyncRwBuf::new(Arc::clone(&self.0))
    }
  }

  new_handle![UserClientHandle => USER_CLIENT_HANDLES: Arc<RwLock<Interns<UserClientWrapper>>>];

  #[repr(C)]
  #[derive(Debug, Clone, Copy)]
  pub struct UserClientRequest {
    pub read_capacity: u64,
    pub write_capacity: u64,
    user_key: InternKey,
    topic_key: InternKey,
    target_kind_key: InternKey,
  }

  impl UserClientRequest {
    fn user(&self) -> UserHandle { UserHandle::from_key(self.user_key) }

    fn topic(&self) -> TopicHandle { TopicHandle::from_key(self.topic_key) }

    fn target_kind(&self) -> UserKindHandle { UserKindHandle::from_key(self.target_kind_key) }
  }

  #[cfg(test)]
  impl UserClientRequest {
    #[allow(clippy::new_without_default)]
    pub fn new(
      read_capacity: usize,
      write_capacity: usize,
      user: UserHandle,
      topic: TopicHandle,
      target_kind: UserKindHandle,
    ) -> Self
    {
      UserClientRequest {
        read_capacity: read_capacity as u64,
        write_capacity: write_capacity as u64,
        user_key: user.as_key(),
        topic_key: topic.as_key(),
        target_kind_key: target_kind.as_key(),
      }
    }
  }

  #[derive(Debug)]
  pub struct UserClientHandleError(String);

  impl From<String> for UserClientHandleError {
    fn from(e: String) -> Self { UserClientHandleError(e) }
  }

  impl From<InternError> for UserClientHandleError {
    fn from(e: InternError) -> Self { UserClientHandleError(format!("{:?}", e)) }
  }

  impl From<io::Error> for UserClientHandleError {
    fn from(e: io::Error) -> Self { UserClientHandleError(format!("{:?}", e)) }
  }

  impl UserClientHandleError {
    pub fn into_io_error(self) -> io::Error {
      io::Error::new(io::ErrorKind::Other, format!("{:?}", self))
    }
  }

  impl ExternallyManagedLifecycle<UserClientWrapper, UserClientHandle, UserClientHandleError>
    for UserClientRequest
  {
    fn make_instance(&self) -> Result<UserClientWrapper, UserClientHandleError> {
      Ok(UserClientWrapper::new(
        UserClient::create_single_buffer_channel(
          self.read_capacity as usize,
          self.write_capacity as usize,
          self.user(),
          self.topic(),
          self.target_kind(),
        ),
      ))
    }

    fn register_handle(handle: &UserClientHandle) -> Result<(), UserClientHandleError> {
      let (user, mut topic) = handle.get(|c| {
        let UserClient { user, topic, .. } = c.get();
        (*user, *topic)
      })?;

      let topic2 = topic;
      topic.extract_mut(
        move |Topic {
                user_client_mapping,
              }| {
          if let Some(user_client) = user_client_mapping.get(&user) {
            Err(UserClientHandleError(format!(
              "topic {:?} already contained a client {:?} for user {:?}",
              topic2, user_client, &user
            )))
          } else {
            user_client_mapping.insert(user, *handle);
            Ok(())
          }
        },
      )
    }

    fn deregister_handle(handle: &UserClientHandle) -> Result<(), UserClientHandleError> {
      let (user, mut topic) = handle.get(|c| {
        let UserClient { user, topic, .. } = c.get();
        (*user, *topic)
      })?;

      let topic2 = topic;
      topic.extract_mut(
        move |Topic {
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
        },
      )
    }
  }

  #[no_mangle]
  pub extern "C" fn create_user_client(
    request: *const UserClientRequest,
    result: *mut InternedObjectCreationResult,
  )
  {
    let request = unsafe { &*request };
    let ret = UserClientRequest::create_handle_ffi(&request);
    unsafe {
      *result = ret;
    }
  }

  #[no_mangle]
  pub extern "C" fn destroy_user_client(
    key: InternKey,
    result: *mut InternedObjectDestructionResult,
  )
  {
    let ret = UserClientRequest::destroy_handle_ffi(key);
    unsafe {
      *result = ret;
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

  impl Debug for ThriftChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(
        f,
        "ThriftChunk(len={:?}, capacity={:?}, ptr={:?})",
        self.len, self.capacity, self.ptr
      )
    }
  }

  impl ThriftChunk {
    ///
    /// # Safety: TODO add section!!!
    pub unsafe fn as_slice(&self) -> &[u8] {
      /* TODO: a macro that turns:
       *   <xxx>![let ThriftChunk { capacity: (capacity * 2), .. } = chunk]
       * into:
       *   let ThriftChunk { capacity, .. } = chunk; let capacity = (capacity * 2);
       */
      let ThriftChunk { ptr, len, capacity } = self;
      assert!(len <= capacity);
      std::slice::from_raw_parts(*ptr, *len as usize)
    }

    ///
    /// # Safety: TODO add section!!!
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
      let ThriftChunk { ptr, len, capacity } = self;
      assert!(len <= capacity);
      std::slice::from_raw_parts_mut(*ptr, *len as usize)
    }
  }

  #[repr(C)]
  #[derive(Clone, Copy)]
  pub enum ThriftWriteResult {
    Written(u64),
    Failed,
  }

  #[tracing::instrument]
  #[no_mangle]
  pub extern "C" fn send_topic_messages(
    handle: InternKey,
    chunk: ThriftChunk,
    result: *mut ThriftWriteResult,
  )
  {
    /* Get the topic, and message every other client *synchronously*! */
    let byte_slice: &[u8] = unsafe { chunk.as_slice() };
    /* dbg!(&byte_slice); */

    let ret = match UserClientHandle::from_key(handle)
      .get(|c| c.clone().into_sync_buf())
      .map_err(UserClientHandleError::from)
      .and_then(|mut client_buf| client_buf.write(byte_slice).map_err(|e| e.into()))
    {
      Ok(len) => ThriftWriteResult::Written(len as u64),
      Err(e) => {
        eprintln!("send_topic_messages: {:?}", e);
        ThriftWriteResult::Failed
      },
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

  #[tracing::instrument]
  #[no_mangle]
  pub extern "C" fn receive_topic_messages(
    handle: InternKey,
    mut chunk: ThriftChunk,
    result: *mut ThriftReadResult,
  )
  {
    /* The incoming chunk will contain any message(s) from other client(s). */
    let byte_slice: &mut [u8] = unsafe { chunk.as_slice_mut() };
    /* dbg!(&byte_slice); */

    let ret = match UserClientHandle::from_key(handle)
      .get(|c| c.clone().into_sync_buf())
      .map_err(UserClientHandleError::from)
      .and_then(|mut client_buf| client_buf.read(byte_slice).map_err(|e| e.into()))
    {
      Ok(len) => {
        chunk.len = len as u64;
        ThriftReadResult::Read(chunk.len)
      },
      Err(e) => {
        eprintln!("receive_topic_messages: {:?}", e);
        ThriftReadResult::Failed
      },
    };
    unsafe {
      *result = ret;
    }
  }
}

#[cfg(test)]
mod tests {
  mod user_client {
    use super::super::all::*;

    use std::{convert::From, fmt::Debug, io::{Read, Write}};

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
      let kind = extract_handle(UserKindRequest);
      let user = extract_handle(UserRequest::new(kind.as_key()));
      let topic = extract_handle(TopicRequest);

      let handle = extract_handle(UserClientRequest::new(0, 0, user, topic, kind));

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
      kind: UserKindHandle,
      message: &[u8],
    ) -> (UserHandle, UserClientHandle, ThriftChunk)
    {
      assert!(capacity >= message.len());

      /* Create new user! */
      let user = extract_handle(UserRequest::new(kind.as_key()));
      /* Connect the new user to the same topic! */
      let user_client = extract_handle(UserClientRequest::new(
        capacity, capacity, user, topic, kind,
      ));

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
    /// 'client_send_messages()` are broadcast to all other participants in the
    /// 'Topic', which is then read back in 'client_receive_messages()'.
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

      let kind = extract_handle(UserKindRequest);

      let topic = extract_handle(TopicRequest);

      let message1 = "asdf".as_bytes();
      let (user1, mut user_client1, mut chunk1) =
        create_new_user_with_client_for_topic(capacity, topic, kind, &message1);
      let mut buf1 = user_client1.extract_mut::<_, UserClientHandleError, _>(|c| {
        Ok(c.clone().into_sync_buf())
      })?;

      /* NB: Using the same topic!! */
      let message2 = "fdsa!!!".as_bytes();
      let (user2, mut user_client2, mut chunk2) =
        create_new_user_with_client_for_topic(capacity, topic, kind, &message2);
      let mut buf2 = user_client2.extract_mut::<_, UserClientHandleError, _>(|c| {
        Ok(c.clone().into_sync_buf())
      })?;

      let byte_slice1 = unsafe { std::slice::from_raw_parts(chunk1.ptr, chunk1.len as usize) };
      let written1 = buf1.write(byte_slice1)?;
      assert_eq!(written1, chunk1.len as usize);

      let byte_slice2 = unsafe { std::slice::from_raw_parts(chunk2.ptr, chunk2.len as usize) };
      let written2 = buf2.write(byte_slice2)?;
      assert_eq!(written2, chunk2.len as usize);

      /* Zero out the original message so we can ensure that it is read back in
       * full from the thrift transport. */
      zero_out_thrift_chunk(&mut chunk1, &message1);
      zero_out_thrift_chunk(&mut chunk2, &message2);

      /* We know how much we expect to be reading -- we will be reading the content
       * of the other user client's message. To signal how much to (try to)
       * read in client_receive_messages(), we set the `len` field of the
       * ThriftChunk that we pass in to the function. */
      chunk1.len = message2.len() as u64;
      let byte_slice1 = unsafe { std::slice::from_raw_parts_mut(chunk1.ptr, chunk1.len as usize) };
      let read1 = buf1.read(byte_slice1)?;
      assert_eq!(read1, written2);

      /* Same for the other user client. */
      chunk2.len = message1.len() as u64;
      let byte_slice2 = unsafe { std::slice::from_raw_parts_mut(chunk2.ptr, chunk2.len as usize) };
      let read2 = buf2.read(byte_slice2)?;
      assert_eq!(read2, written1);

      /* /\* Flush the writes!! *\/ */
      /* user_client1.extract_mut::<_, UserClientHandleError, _>(|c| { */
      /*   let mut buf = c.clone().into_sync_buf(); */
      /*   buf.flush().map_err(|e| e.into()) */
      /* })?; */

      /* user_client2.extract_mut::<_, UserClientHandleError, _>(|c| { */
      /*   let mut buf = c.clone().into_sync_buf(); */
      /*   buf.flush().map_err(|e| e.into()) */
      /* })?; */

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
