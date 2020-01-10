use parking_lot::{Mutex, RwLock};

use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InternError(String);

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct InternKey(u64);

pub trait Interned<T> {
  fn as_key(&self) -> InternKey;
  fn from_key(key: InternKey) -> Self;
  fn interns() -> Arc<RwLock<Interns<T>>>;

  fn dereference(&self) -> Result<Arc<Mutex<T>>, InternError> {
    Ok(Self::interns().read().get(self.as_key())?)
  }

  fn garbage_collect(&mut self) -> Result<(), InternError> {
    Ok(Self::interns().write().garbage_collect(self.as_key())?)
  }
}

/* NB: The `: Sized` is needed so we can return `Self` in `::intern()`! */
pub trait Handle<T>: Interned<T>+Sized {
  fn intern(value: T) -> Result<Self, InternError> {
    let key = Self::interns().write().intern(value)?;
    Ok(Self::from_key(key))
  }
}

pub struct Interns<T> {
  mapping: HashMap<InternKey, Arc<Mutex<T>>>,
  idx: u64,
}

impl<T> Interns<T> {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    Interns {
      mapping: HashMap::new(),
      idx: 0,
    }
  }

  pub fn intern(&mut self, value: T) -> Result<InternKey, InternError> {
    let key = {
      let idx = self.idx;
      self.idx += 1;
      InternKey(idx)
    };
    let wrapped = Arc::new(Mutex::new(value));
    if self.mapping.insert(key, wrapped).is_some() {
      Err(InternError(format!(
        "key {:?} should not exist already, but did!",
        key
      )))
    } else {
      Ok(key)
    }
  }

  pub fn get(&self, key: InternKey) -> Result<Arc<Mutex<T>>, InternError> {
    self
      .mapping
      .get(&key)
      .map(|val| Arc::clone(val))
      .ok_or_else(|| InternError(format!("could not find interned key {:?}", key)))
  }

  pub fn garbage_collect(&mut self, key: InternKey) -> Result<(), InternError> {
    if self.mapping.remove(&key).is_some() {
      Ok(())
    } else {
      Err(InternError(format!(
        "key {:?} did not exist in the mapping when garbage collection was attempted!",
        key
      )))
    }
  }
}

#[macro_export]
macro_rules! new_handle {
  ($name:ident => $interns_name:ident : Arc < RwLock < Interns < $into:ty >> >) => {
    #[repr(C)]
    #[derive(Debug)]
    pub struct $name {
      key: InternKey,
    }

    lazy_static! {
      static ref $interns_name: Arc<RwLock<Interns<$into>>> = Arc::new(RwLock::new(Interns::new()));
    }

    impl Interned<$into> for $name {
      fn as_key(&self) -> InternKey { self.key }

      fn from_key(key: InternKey) -> Self { $name { key } }

      fn interns() -> Arc<RwLock<Interns<$into>>> { Arc::clone(&$interns_name) }
    }

    impl Handle<$into> for $name {}
  };
}
