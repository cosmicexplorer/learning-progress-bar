use lazy_static::lazy_static;
use parking_lot::RwLock;

use std::{
  collections::HashMap,
  fmt::Debug,
  sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
  },
};

lazy_static! {
  static ref GLOBAL_GENSYM_STATE: AtomicU64 = AtomicU64::new(0);
}

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
}

impl<T> Interns<T> {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    Interns {
      mapping: HashMap::new(),
    }
  }

  fn gen_key() -> InternKey { InternKey(GLOBAL_GENSYM_STATE.fetch_add(1, Ordering::Relaxed)) }

  pub fn intern(&mut self, value: T) -> Result<InternKey, InternError> {
    let key = Self::gen_key();
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

    #[repr(C)]
    #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    pub struct $name {
      key: InternKey,
    }

    ::lazy_static::lazy_static! {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, Clone, Copy, Eq, PartialEq)]
  struct PointedToType(usize);

  new_handle![AHandleType => TABLE_A: Arc<RwLock<Interns<PointedToType>>>];

  new_handle![BHandleType => TABLE_B: Arc<RwLock<Interns<PointedToType>>>];

  ///
  /// Ensure that no Interned<T> implementor will ever be able to dereference a handle created via
  /// Interned::from_key() in the wrong Interns<T> table (since all keys for all intern tables use a
  /// global index).
  #[test]
  fn no_colliding_keys() -> Result<(), InternError> {
    let a = AHandleType::intern(PointedToType(1))?;
    let a_1 = AHandleType::intern(PointedToType(2))?;

    assert_ne!(a, a_1);
    assert_ne!(a.get(|a| *a), a_1.get(|a_1| *a_1));

    let b = BHandleType::intern(PointedToType(1))?;
    let b_1 = BHandleType::intern(PointedToType(1))?;

    assert_ne!(b.as_key(), a.as_key());
    assert_ne!(b.as_key(), a_1.as_key());

    assert_ne!(b, b_1);
    assert_eq!(b.get(|b| *b), b_1.get(|b_1| *b_1));

    Ok(())
  }
}
