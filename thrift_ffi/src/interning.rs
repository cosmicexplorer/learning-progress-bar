use super::ThriftStreamCreationError;

use parking_lot::Mutex;

use std::{collections::HashMap, fmt::Debug, sync::Arc};

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
  mapping: HashMap<InternKey, Arc<Mutex<T>>>,
  idx: u64,
}

impl<T> Interns<T> {
  pub fn new() -> Interns<T> {
    Interns {
      mapping: HashMap::new(),
      idx: 0,
    }
  }

  pub fn get(&self, key: InternKey) -> Result<Arc<Mutex<T>>, InternError> {
    self
      .mapping
      .get(&key)
      .map(|val| Arc::clone(val))
      .ok_or_else(|| InternError(format!("could not find interned key {:?}", key)))
  }
}

pub enum UninternResult {
  SuccessfullyUninterned,
  DoubleFreed,
}

impl<T: Debug> Interns<T> {
  pub fn intern(&mut self, value: T) -> Result<InternKey, InternError> {
    let key = {
      let idx = self.idx;
      self.idx += 1;
      InternKey(idx)
    };
    let wrapped = Arc::new(Mutex::new(value));
    if let Some(previous_value) = self.mapping.insert(key, wrapped) {
      Err(InternError(format!(
        "key {:?} should not exist already, but did! previous value: {:?}",
        key, previous_value
      )))
    } else {
      Ok(key)
    }
  }

  pub fn garbage_collect(&mut self, key: InternKey) -> UninternResult {
    if self.mapping.remove(&key).is_some() {
      UninternResult::SuccessfullyUninterned
    } else {
      UninternResult::DoubleFreed
    }
  }
}
