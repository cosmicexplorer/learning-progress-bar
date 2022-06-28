use super::interning::*;

use std::{convert::From, fmt::Debug};

#[repr(C)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InternedObjectCreationResult {
  Created(InternKey),
  Failed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InternedObjectDestructionResult {
  Succeeded,
  Failed,
}

pub trait ExternallyManagedLifecycle<T, H: Handle<T>+Debug, E: From<InternError>+Debug>:
  Debug {
  fn make_instance(&self) -> Result<T, E>;

  fn register_handle(handle: &H) -> Result<(), E>;

  fn create_handle_ffi(&self) -> InternedObjectCreationResult {
    match self
      .make_instance()
      .and_then(|value| H::intern(value).map_err(|e| e.into()))
      .and_then(|handle| {
        Self::register_handle(&handle)?;
        Ok(handle)
      }) {
      Ok(handle) => InternedObjectCreationResult::Created(handle.as_key()),
      Err(e) => {
        eprintln!("Error creating handle from request {:?}: {:?}", self, e);
        InternedObjectCreationResult::Failed
      },
    }
  }

  fn deregister_handle(handle: &H) -> Result<(), E>;

  fn destroy_handle_ffi(key: InternKey) -> InternedObjectDestructionResult {
    let mut handle = H::from_key(key);
    match Self::deregister_handle(&handle)
      .and_then(|()| handle.garbage_collect().map_err(|e| e.into()))
    {
      Ok(()) => InternedObjectDestructionResult::Succeeded,
      Err(e) => {
        eprintln!("Error destroying handle {:?}: {:?}", handle, e);
        InternedObjectDestructionResult::Failed
      },
    }
  }
}
