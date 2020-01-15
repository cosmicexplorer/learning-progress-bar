use parking_lot::{Condvar, Mutex};

use std::{
  convert::From,
  fmt::{self, Debug},
  io::{self, Read, Write},
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

pub struct StateVar<T> {
  mutex: Mutex<T>,
  cvar: Condvar,
}

impl<T> StateVar<T> {
  pub fn get_conditional_locking_state(&self) -> (&Mutex<T>, &Condvar) {
    let StateVar { mutex, cvar } = self;
    (mutex, cvar)
  }

  pub fn new(value: T) -> Self {
    StateVar {
      mutex: Mutex::new(value),
      cvar: Condvar::new(),
    }
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BlockingStates {
  Ready,
  Blocked,
}

pub trait ReadWriteBufferable: Read+Write {
  fn read_is_starved(&self) -> bool;
  fn write_is_starved(&self) -> bool;
  fn copy_write_to_read(&mut self);
}

pub struct SynchronizedReadWriteBuffer<IoObject: ReadWriteBufferable> {
  io_object: IoObject,
  read: StateVar<BlockingStates>,
  write: StateVar<BlockingStates>,
}

impl<IoObject: ReadWriteBufferable> Debug for SynchronizedReadWriteBuffer<IoObject> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "SynchronizedReadWriteBuffer(...)")
  }
}

impl<IoObject: ReadWriteBufferable> SynchronizedReadWriteBuffer<IoObject> {
  pub fn new(io_object: IoObject) -> Self {
    SynchronizedReadWriteBuffer {
      io_object,
      read: StateVar::new(BlockingStates::Blocked),
      write: StateVar::new(BlockingStates::Ready),
    }
  }

  ///
  /// Corresponds to 'flush'.
  /* TODO: Have an optional timeout arg, and return a Result? */
  fn wait_until_read_is_empty(&self) {
    let SynchronizedReadWriteBuffer {
      write, io_object, ..
    } = self;
    let (write_mutex, write_cvar) = write.get_conditional_locking_state();
    let mut write_state = write_mutex.lock();

    /* Wait *twice* for the "write is starved" flag. This means that the channel
     * has been cleared twice, which means that anything which was in the
     * write buffer has now been read successfully (out of the read buffer). */
    while !io_object.write_is_starved() {
      assert_eq!(*write_state, BlockingStates::Ready);
      write_cvar.wait(&mut write_state);
    }

    while !io_object.write_is_starved() {
      assert_eq!(*write_state, BlockingStates::Ready);
      write_cvar.wait(&mut write_state);
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
    let (write_mutex, write_cvar) = write.get_conditional_locking_state();
    let mut write_state = write_mutex.lock();

    let mut written: usize = 0;
    while written < byte_slice.len() {
      while *write_state != BlockingStates::Ready {
        write_cvar.wait(&mut write_state);
      }
      assert!(io_object.read_is_starved());

      let (read_mutex, read_cvar) = read.get_conditional_locking_state();
      let mut read_state = read_mutex.lock();

      let written_new = io_object.write(&byte_slice[written..])?;
      assert!(written_new > 0);

      *read_state = BlockingStates::Ready;
      read_cvar.notify_all();

      written += written_new;

      if io_object.write_is_starved() {
        /* Any other threads of execution which were previously waiting on
         * `write_cvar()` will immediately check the `write_state` and
         * immediately go back to `write_cvar.wait()` in the while loop
         * above. */
        *write_state = BlockingStates::Blocked;
      }
    }

    Ok(())
  }

  ///
  /// Corresponds to 'read'.
  /* FIXME: could this one be restructured to look more like the write()
   * impl??? */
  fn read_as_many_possible(&mut self, byte_slice: &mut [u8]) -> Result<usize, CoroutineError> {
    let SynchronizedReadWriteBuffer {
      ref read,
      ref write,
      ref mut io_object,
    } = self;

    let (read_mutex, read_cvar) = read.get_conditional_locking_state();
    let mut read_state = read_mutex.lock();

    /* A notification to `read_cvar` means: "I just wrote something to your
     * `channel`!" */
    while *read_state != BlockingStates::Ready {
      read_cvar.wait(&mut read_state);
    }
    assert!(!io_object.read_is_starved());
    let mut read: usize = 0;
    read += io_object.read(&mut byte_slice[read..])?;

    /* (1) Check to see if we can copy anything from the write buffer to complete
     * this attempted     read. */
    if read < byte_slice.len() {
      /* Since the read buffer is now (presumably) empty, let's copy everything
       * from the write buffer! */
      let (write_mutex, write_cvar) = write.get_conditional_locking_state();
      let mut write_state = write_mutex.lock();

      io_object.copy_write_to_read();

      /* This always completely clears the write buffer, so we can be sure to let
       * *all* writer know they can begin to write again (once they get the lock!). */
      /* See https://docs.rs/parking_lot/0.7.1/parking_lot/struct.Condvar.html#differences-from-the-standard-library-condvar:
       * Condvar::notify_all will only wake up a single thread, the rest are
       * requeued to wait for the Mutex to be unlocked by the thread that
       * was woken up. */
      if !io_object.write_is_starved() {
        *write_state = BlockingStates::Ready;
        write_cvar.notify_all();
      }

      /* If the write buffer was non-empty, those bytes are now in the read buffer,
       * and reads can begin again! */
      /* NB: THIS `byte_slice[read..]` BELOW SHOULD BE INDEXED STARTING AT THE `read` INDEX, *NOT*
       * AT 0!!!! */
      let read_new = io_object.read(&mut byte_slice[read..])?;
      /* NB: This `read_new` may be 0, if there was nothing in the write buffer to copy over!! */
      read += read_new;
    }
    assert!(read > 0);
    assert!(read <= byte_slice.len());

    /* (2) Check again to see whether we were able to add enough bytes to
     * complete the read from     copying over the write buffer. */
    if io_object.read_is_starved() {
      /* The next time this method is called, it will block at `read_cvar.wait()`! */
      *read_state = BlockingStates::Blocked;
      read_cvar.notify_one();
    }

    assert!(read <= byte_slice.len());

    Ok(read)
  }
}

impl<IoObject: ReadWriteBufferable> Read for SynchronizedReadWriteBuffer<IoObject> {
  fn read(&mut self, byte_slice: &mut [u8]) -> io::Result<usize> {
    self
      .read_as_many_possible(byte_slice)
      .map_err(|e| e.into_io_error())
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
    self.io_object.flush()?;
    self.wait_until_read_is_empty();
    Ok(())
  }
}
