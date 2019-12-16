/* use futures::{stream::Stream, task::Poll}; */

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubprocessError(String);

impl From<SubprocessError> for TerminalError {
  fn from(err: SubprocessError) -> Self {
    TerminalError(format!("subprocess error: {:?}", err))
  }
}

/* struct SubprocessIOWrapper; */

/* impl SubprocessIOWrapper { */
/*   pub fn spawn(req: ProcessExecutionRequest) -> Result<Self, SubprocessError> { */

/*   } */
/* } */

/* impl Stream for SubprocessIOWrapper { */
/*   type Item = SubprocessEvent; */
/*   type Error = TerminalError; */

/*   fn poll_next(&mut self, cx: &mut Context) -> Result<Async<Option<Self::Item>>, Self::Error> { */
/*     ??? */
/*   } */
/* } */
