use itertools::Itertools;

use std::{fmt::Debug, iter::Iterator};

///
/// A quick extension trait for iterators to make it easier to zip down a list
/// and capture all errors in execution of a function, not just failing on the
/// first one.
pub trait ResultSequence<T>: Iterator<Item=T>+Sized {
  fn split_result_sequence<ReturnType, E, F: Fn(&Self::Item) -> Result<ReturnType, E>>(
    self,
    f: F,
  ) -> (Vec<ReturnType>, Vec<E>)
  {
    self.fold((vec![], vec![]), |(mut oks, mut errs), ref it| {
      match f(it) {
        Ok(value) => {
          oks.push(value);
          (oks, errs)
        },
        Err(e) => {
          errs.push(e);
          (oks, errs)
        },
      }
    })
  }

  fn split_result_sequence_mut<
    ReturnType,
    E,
    F: FnMut(&mut Self::Item) -> Result<ReturnType, E>,
  >(
    self,
    mut f: F,
  ) -> (Vec<ReturnType>, Vec<E>)
  {
    self.fold(
      (vec![], vec![]),
      |(mut oks, mut errs), ref mut it| match f(it) {
        Ok(value) => {
          oks.push(value);
          (oks, errs)
        },
        Err(e) => {
          errs.push(e);
          (oks, errs)
        },
      },
    )
  }
}

impl<T, I: Iterator<Item=T>+Sized> ResultSequence<T> for I {}

pub trait ConsumedSequence<T>: ResultSequence<T>+Iterator<Item=T>+Sized {
  /* TODO(PERFORMANCE): This iteration should/could all be in parallel! */
  fn handle_split_result_sequence<E: Debug, F: FnMut(&mut Self::Item) -> Result<(), E>>(
    self,
    f: F,
  ) -> Result<(), String>
  {
    let (_, errors): (Vec<()>, Vec<E>) = self.split_result_sequence_mut::<(), E, _>(f);

    if errors.is_empty() {
      Ok(())
    } else {
      Err(format!(
        "errors writing:\n{:?}",
        errors.into_iter().map(|e| format! {"{:?}", e}).format("\n")
      ))
    }
  }
}


impl<T, I: Iterator<Item=T>+Sized> ConsumedSequence<T> for I {}
