/* NB: Any nightly-only features go here >=]! */
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

use parking_lot::{Condvar, Mutex};

use std::{
  clone::Clone,
  convert::From,
  fmt::{self, Debug},
  io::{self, Read, Write},
  ops::{Deref, DerefMut},
  sync::Arc,
};

#[derive(Debug)]
pub struct CoroutineError(String);

impl From<String> for CoroutineError {
  fn from(e: String) -> Self { CoroutineError(e) }
}

impl From<io::Error> for CoroutineError {
  fn from(e: io::Error) -> Self { CoroutineError(format!("{:?}", e)) }
}

impl CoroutineError {
  pub fn into_io_error(self) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("{:?}", self))
  }
}

///
/// A mutex, and a condition variable to use with the mutex.
pub struct StateVar<T> {
  mutex: Mutex<T>,
  cvar: Condvar,
}

impl<T> Debug for StateVar<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "StateVar(...)") }
}

impl<T> StateVar<T> {
  pub fn new(value: T) -> Self {
    StateVar {
      mutex: Mutex::new(value),
      cvar: Condvar::new(),
    }
  }

  pub fn notify_all_of_new_state<F: FnMut(&T) -> T>(&self, mut f: F) {
    let StateVar { mutex, cvar } = self;
    let mut state = mutex.lock();

    *state = f(&*state);
    cvar.notify_all();
  }
}

impl<T: PartialEq+Eq> StateVar<T> {
  pub fn wait_for_slot_to_completely_execute<E, F: FnMut() -> Result<T, E>>(
    &self,
    run_at_state: &T,
    mut f: F,
  ) -> Result<(), E>
  {
    let StateVar { mutex, cvar } = self;
    let mut state = mutex.lock();

    /* If the state is already in the desired position, we will immediately run
     * the method. */
    if *state == *run_at_state {
      *state = f()?;
    }
    while *state != *run_at_state {
      while *state != *run_at_state {
        cvar.wait(&mut state);
      }
      *state = f()?;
    }
    Ok(())
  }
}

///
/// Current interface for pausing and resuming progress via the condition
/// variable/mutex in a StateVar instance. This is somewhat similar to
/// coroutines in other languages.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ReadBlockingStates {
  HasData,
  NeedsData,
  WasJustCompleted,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum WriteBlockingStates {
  Ready,
  Blocked,
  FinishedWrite,
}

///
/// Abstract description of a bidirectional in-memory buffered I/O channel.
/// Intended to work with thrift's TBufferChannel (which currently requires a
/// tiny upstream thrift change).
/* FIXME: upstream the thrift patch to support this interface! */
pub trait ReadWriteBufferable: Read+Write+Debug {
  fn read_has_any(&self) -> bool;
  fn write_is_full(&self) -> bool;
  fn write_has_any(&self) -> bool;
  fn copy_write_buffer_to_read_buffer(&mut self);
}

///
/// A struct wrapping access to some bidirectional IO-like object. The use case
/// is to synchronize read and write attempts to many different read-write
/// buffers to wait as little as possible. Condition variables are used in a
/// coroutine-esque way here.
#[derive(Debug)]
pub struct SynchronizedReadWriteBuffer<IoObject: ReadWriteBufferable> {
  io_object: IoObject,
  read: StateVar<ReadBlockingStates>,
  write: StateVar<WriteBlockingStates>,
}

impl<IoObject: ReadWriteBufferable+Debug> SynchronizedReadWriteBuffer<IoObject> {
  pub fn new(io_object: IoObject) -> Self {
    SynchronizedReadWriteBuffer {
      io_object,
      read: StateVar::new(ReadBlockingStates::NeedsData),
      write: StateVar::new(WriteBlockingStates::Ready),
    }
  }

  ///
  /// Corresponds to 'write'.
  ///
  /// The rationale of writing "until complete" here is under the assumption
  /// that the buffers we get may not be perfectly aligned to thrift structs.
  /// We also don't want to maintain any additional buffering in this struct,
  /// preferring to allow the underlying I/O handle do any buffering.
  fn write_until_complete(&mut self, byte_slice: &[u8]) -> Result<(), CoroutineError> {
    let SynchronizedReadWriteBuffer {
      ref read,
      ref write,
      ref mut io_object,
    } = self;

    let mut written: usize = 0;
    write.wait_for_slot_to_completely_execute(&WriteBlockingStates::FinishedWrite, move || {
      /* eprintln!("written: {:?}", written); */
      if io_object.write_is_full() {
        read.notify_all_of_new_state(|read_state| match read_state {
          ReadBlockingStates::NeedsData => {
            assert!(!io_object.read_has_any());
            io_object.copy_write_buffer_to_read_buffer();
            assert!(io_object.read_has_any());
            ReadBlockingStates::HasData
          },
          x => *x,
        });
        if io_object.write_is_full() {
          return Ok(WriteBlockingStates::Blocked);
        }
      }

      if byte_slice.is_empty() {
        return Ok(WriteBlockingStates::Ready);
      }

      let written_new = io_object.write(&byte_slice[written..])?;
      assert!(written_new > 0);
      dbg!(&written_new);

      written += written_new;
      assert!(written <= byte_slice.len());

      if written == byte_slice.len() {
        Ok(WriteBlockingStates::FinishedWrite)
      } else {
        assert!(io_object.write_is_full());
        Ok(WriteBlockingStates::Blocked)
      }
    })
  }

  ///
  /// Corresponds to 'read'.
  fn read_until_complete(&mut self, byte_slice: &mut [u8]) -> Result<(), CoroutineError> {
    /* eprintln!("read_until_complete: {:?}", &self); */
    let SynchronizedReadWriteBuffer {
      ref read,
      ref write,
      ref mut io_object,
    } = self;

    /* Ensure that if we come into this function and the read buffer is empty, we
     * at least try to copy over the write buffer. This lets us avoid
     * blocking at all if there is enough data in the write buffer to satisfy
     * the read bytes request. */
    read.notify_all_of_new_state(|read_state| match read_state {
      ReadBlockingStates::NeedsData => {
        assert!(!io_object.read_has_any());
        write.notify_all_of_new_state(|_| {
          /* panic!("wow?1?"); */
          assert!(!io_object.read_has_any());
          io_object.copy_write_buffer_to_read_buffer();
          WriteBlockingStates::Ready
        });
        if io_object.read_has_any() {
          ReadBlockingStates::HasData
        } else {
          ReadBlockingStates::NeedsData
        }
      },
      ReadBlockingStates::HasData => {
        /* panic!("wow?3?"); */
        if !io_object.read_has_any() {
          write.notify_all_of_new_state(|_| {
            assert!(!io_object.read_has_any());
            io_object.copy_write_buffer_to_read_buffer();
            WriteBlockingStates::Ready
          });
          if !io_object.read_has_any() {
            return ReadBlockingStates::NeedsData;
          }
        }
        ReadBlockingStates::HasData
      },
    });

    let mut all_bytes_read: usize = 0;
    read.wait_for_slot_to_completely_execute(&ReadBlockingStates::WasJustCompleted, move || {
      /* panic!("wow?2?"); */
      if !io_object.read_has_any() {
        write.notify_all_of_new_state(|_| {
          assert!(!io_object.read_has_any());
          io_object.copy_write_buffer_to_read_buffer();
          WriteBlockingStates::Ready
        });
        if !io_object.read_has_any() {
          return Ok(ReadBlockingStates::NeedsData);
        }
      }

      if byte_slice.is_empty() {
        return Ok(ReadBlockingStates::HasData);
      }

      let read_new = io_object.read(&mut byte_slice[all_bytes_read..])?;
      assert!(read_new > 0);

      all_bytes_read += read_new;
      assert!(all_bytes_read <= byte_slice.len());

      if all_bytes_read == byte_slice.len() {
        Ok(ReadBlockingStates::WasJustCompleted)
      } else {
        /* NB: We expect that if the read did not fill out our buffer, that the
         * underlying read buffer must then be empty! */
        assert!(!io_object.read_has_any());
        Ok(ReadBlockingStates::NeedsData)
      }
    })
  }
}

impl<IoObject: ReadWriteBufferable> Read for SynchronizedReadWriteBuffer<IoObject> {
  fn read(&mut self, byte_slice: &mut [u8]) -> io::Result<usize> {
    self
      .read_until_complete(byte_slice)
      .map_err(|e| e.into_io_error())?;
    Ok(byte_slice.len())
  }
}

impl<IoObject: ReadWriteBufferable> Write for SynchronizedReadWriteBuffer<IoObject> {
  fn write(&mut self, byte_slice: &[u8]) -> io::Result<usize> {
    self
      .write_until_complete(byte_slice)
      .map_err(|e| e.into_io_error())?;
    Ok(byte_slice.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    /* FIXME: make flushing the underlying I/O object re-entrant!! */
    /* self.io_object.flush()?; */
    /* TODO: document why this works!!! */
    self
      .write_until_complete(&[])
      .and_then(|()| self.read_until_complete(&mut []))
      .map_err(|e| e.into_io_error())?;
    Ok(())
  }
}

/* FIXME: this should be something *normal* about Send/Sync but I can't
 * figure out how to do that, so replicating it here, probably!!! */
pub unsafe trait SyncRwAccessable {}

unsafe impl<IoObject: ReadWriteBufferable> SyncRwAccessable
  for SynchronizedReadWriteBuffer<IoObject>
{
}

#[derive(Debug)]
pub struct SyncRwBuf<T: SyncRwAccessable> {
  inner: Arc<T>,
}

impl<T: SyncRwAccessable> Clone for SyncRwBuf<T> {
  fn clone(&self) -> Self { Self::new(Arc::clone(&self.inner)) }
}

impl<T: SyncRwAccessable> SyncRwBuf<T> {
  pub fn new(inner: Arc<T>) -> Self { SyncRwBuf { inner } }

  pub fn into_arc(self) -> Arc<T> { self.inner }
}

impl<T: SyncRwAccessable> Deref for SyncRwBuf<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target { self.inner.deref() }
}

impl<T: SyncRwAccessable> DerefMut for SyncRwBuf<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { Arc::get_mut_unchecked(&mut self.inner) }
  }
}

unsafe impl<T: SyncRwAccessable> Send for SyncRwBuf<T> {}
unsafe impl<T: SyncRwAccessable> Sync for SyncRwBuf<T> {}

unsafe impl<T: SyncRwAccessable> SyncRwAccessable for SyncRwBuf<T> {}
unsafe impl<T: SyncRwAccessable> SyncRwAccessable for Arc<T> {}
unsafe impl<T: SyncRwAccessable> SyncRwAccessable for Mutex<T> {}
