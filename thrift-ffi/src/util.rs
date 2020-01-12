use std::iter::Iterator;

///
/// A quick extension trait for iterators to make it easier to zip down a list and capture all
/// errors in execution of a function, not just failing on the first one.
///
pub  trait ResultSequence<T>: Iterator<Item=T>+Sized {
  fn split_result_sequence<ReturnType, E, F: Fn(Self::Item) -> Result<ReturnType, E>>(
    self,
    f: F,
  ) -> (Vec<ReturnType>, Vec<E>)
  {
    self.fold((vec![], vec![]), |(mut oks, mut errs), it| match f(it) {
      Ok(value) => {
        oks.push(value);
        (oks, errs)
      },
      Err(e) => {
        errs.push(e);
        (oks, errs)
      },
    })
  }

  fn split_result_sequence_mut<ReturnType, E, F: FnMut(Self::Item) -> Result<ReturnType, E>>(
    self,
    mut f: F,
  ) -> (Vec<ReturnType>, Vec<E>)
  {
    self.fold((vec![], vec![]), |(mut oks, mut errs), it| match f(it) {
      Ok(value) => {
        oks.push(value);
        (oks, errs)
      },
      Err(e) => {
        errs.push(e);
        (oks, errs)
      },
    })
  }
}

impl<T, I: Iterator<Item=T>+Sized> ResultSequence<T> for I {}
