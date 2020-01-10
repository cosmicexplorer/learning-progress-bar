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

  fn create_handle_ffi(&self) -> InternedObjectCreationResult {
    match self
      .make_instance()
      .and_then(|value| H::intern(value).map_err(|e| e.into()))
    {
      Ok(handle) => InternedObjectCreationResult::Created(handle.as_key()),
      Err(e) => {
        eprintln!("Error creating handle from request {:?}: {:?}", self, e);
        InternedObjectCreationResult::Failed
      },
    }
  }

  fn destroy_handle_ffi(key: InternKey) -> InternedObjectDestructionResult {
    let mut handle = H::from_key(key);
    match handle.garbage_collect() {
      Ok(()) => InternedObjectDestructionResult::Succeeded,
      Err(e) => {
        eprintln!("Error destroying handle {:?}: {:?}", handle, e);
        InternedObjectDestructionResult::Failed
      },
    }
  }
}
