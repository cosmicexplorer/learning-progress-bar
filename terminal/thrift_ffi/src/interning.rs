use super::ThriftStreamCreationError;

use std::{collections::HashMap, fmt::Debug};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InternError(String);

impl From<InternError> for ThriftStreamCreationError {
  fn from(e: InternError) -> Self { ThriftStreamCreationError(format!("{:?}", e)) }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct InternKey(u64);

/* Eq, PartialEq, Hash */

pub struct Interns<T> {
  mapping: HashMap<InternKey, T>,
  _idx: u64,
}

impl<T> Interns<T> {
  pub fn new() -> Interns<T> {
    Interns {
      mapping: HashMap::new(),
      _idx: 0,
    }
  }

  pub fn get(&self, key: InternKey) -> Result<&T, InternError> {
    self
      .mapping
      .get(&key)
      .ok_or_else(|| InternError(format!("could not find interned key {:?}", key)))
  }
}

impl<T: Debug> Interns<T> {
  pub fn intern(&mut self, value: T) -> Result<InternKey, InternError> {
    let key = {
      let idx = self._idx;
      self._idx += 1;
      InternKey(idx)
    };
    if let Some(previous_value) = self.mapping.insert(key, value) {
      Err(InternError(format!(
        "key {:?} should not exist already, but did! previous value: {:?}",
        key, previous_value
      )))
    } else {
      Ok(key)
    }
  }
}
