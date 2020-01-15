use parking_lot::RwLock;

use std::{collections::HashMap, fmt::Debug, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InternError(String);

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct InternKey(u64);

pub trait Interned<T> {
  fn as_key(&self) -> InternKey;
  fn from_key(key: InternKey) -> Self;
  fn interns() -> Arc<RwLock<Interns<T>>>;

  fn dereference(&self) -> Result<Arc<RwLock<T>>, InternError> {
    Ok(Self::interns().read().get(self.as_key())?)
  }

  fn garbage_collect(&mut self) -> Result<(), InternError> {
    Ok(Self::interns().write().garbage_collect(self.as_key())?)
  }

  fn get<ReturnType, F: Fn(&T) -> ReturnType>(&self, f: F) -> Result<ReturnType, InternError> {
    let this_ref = self.dereference()?;
    let this_lock = this_ref.read();
    Ok(f(&*this_lock))
  }

  fn extract<ReturnType, E: From<InternError>, F: Fn(&T) -> Result<ReturnType, E>>(
    &self,
    f: F,
  ) -> Result<ReturnType, E>
  {
    let this_ref = self.dereference()?;
    let this_lock = this_ref.read();
    f(&*this_lock)
  }

  fn extract_mut<ReturnType, E: From<InternError>, F: FnMut(&mut T) -> Result<ReturnType, E>>(
    &mut self,
    mut f: F,
  ) -> Result<ReturnType, E>
  {
    let this_ref = self.dereference()?;
    let mut this_lock = this_ref.write();
    f(&mut *this_lock)
  }
}

/* NB: The `: Sized` is needed so we can return `Self` in `::intern()`! (We
 * may not even need to be doing that, though!) */
pub trait Handle<T>: Interned<T>+Sized {
  fn intern(value: T) -> Result<Self, InternError> {
    let key = Self::interns().write().intern(value)?;
    Ok(Self::from_key(key))
  }
}

pub struct Interns<T> {
  mapping: HashMap<InternKey, Arc<RwLock<T>>>,
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
    let wrapped = Arc::new(RwLock::new(value));
    if self.mapping.insert(key, wrapped).is_some() {
      Err(InternError(format!(
        "key {:?} should not exist already, but did!",
        key
      )))
    } else {
      Ok(key)
    }
  }

  pub fn get(&self, key: InternKey) -> Result<Arc<RwLock<T>>, InternError> {
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
    use lazy_static::lazy_static;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    pub struct $name {
      key: InternKey,
    }

    lazy_static! {
      static ref $interns_name: std::sync::Arc<parking_lot::RwLock<Interns<$into>>> =
        std::sync::Arc::new(parking_lot::RwLock::new(Interns::new()));
    }

    impl Interned<$into> for $name {
      fn as_key(&self) -> InternKey { self.key }

      fn from_key(key: InternKey) -> Self { $name { key } }

      fn interns() -> std::sync::Arc<parking_lot::RwLock<Interns<$into>>> {
        std::sync::Arc::clone(&$interns_name)
      }
    }

    impl Handle<$into> for $name {}
  };
}
