#![deny(warnings)]
// Enable all clippy lints except for many of the pedantic ones. It's a shame this needs to be
// copied and pasted across crates, but there doesn't appear to be a way to include inner attributes
// from a common source.
#![deny(
  clippy::all,
  clippy::default_trait_access,
  clippy::expl_impl_clone_on_copy,
  clippy::if_not_else,
  clippy::needless_continue,
  clippy::single_match_else,
  clippy::unseparated_literal_suffix,
  clippy::used_underscore_binding
)]
// It is often more clear to show that nothing is being moved.
#![allow(clippy::match_ref_pats)]
// Subjective style.
#![allow(
  clippy::len_without_is_empty,
  clippy::redundant_field_names,
  clippy::too_many_arguments
)]
// Default isn't as big a deal as people seem to think it is.
#![allow(clippy::new_without_default, clippy::new_ret_no_self)]
// Arc<Mutex> can be more clear than needing to grok Orderings:
#![allow(clippy::mutex_atomic)]
// We only use unsafe pointer derefrences in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(not(feature = "pants-injected"))]
compile_error!("This crate currently requires the \"pants-injected\" feature to be activated!");

pub mod streaming_interface;
/* use streaming_interface::*; */

/* use regex::Regex; */
/* use thrift::transport::{ */
/*   TInputProtocolFactory, TOutputProtocolFactory, TReadTransportFactory, TWriteTransportFactory, */
/* }; */

/* use std::{io, slice}; */

/* #[repr(C)] */
/* #[derive(Clone, Copy)] */
/* struct BasicServer; */

/* impl TerminalWrapperSyncHandler for BasicServer { */
/*   fn handle_begin_execution(&self, exe_req: ProcessExecutionRequest) -> thrift::Result<RunId> { */
/*     Ok(RunId::new("asdf")) */
/*   } */

/*   fn handle_get_next_event(&self) -> thrift::Result<SubprocessEvent> { */
/*     Ok(SubprocessEvent::new(None, None, None, None)) */
/*   } */
/* } */

/* #[repr(C)] */
/* #[derive(Clone, Copy)] */
/* pub enum ServerCreationRequest; */

/* #[repr(C)] */
/* #[derive(Clone, Copy)] */
/* pub enum ServerCreationResponse { */
/*   Success(*mut BasicServer), */
/*   Failure, */
/* } */

/* #[derive(Debug, Clone, Eq, PartialEq)] */
/* pub struct TerminalFFIError(String); */

/* pub fn create_server(request: ServerCreationRequest) -> Result<BasicServer, TerminalFFIError> { */
/*   let processor = TerminalWrapperSyncProcessor::new(BasicServer); */

/*   // instantiate the server */
/*   let i_tr_fact: Box<TReadTransportFactory> = Box::new(TBufferedReadTransportFactory::new()); */
/*   let i_pr_fact: Box<TInputProtocolFactory> = Box::new(TBinaryInputProtocolFactory::new()); */
/*   let o_tr_fact: Box<TWriteTransportFactory> = Box::new(TBufferedWriteTransportFactory::new()); */
/*   let o_pr_fact: Box<TOutputProtocolFactory> = Box::new(TBinaryOutputProtocolFactory::new()); */


/* } */

/* #[no_mangle] */
/* pub extern "C" fn create_basic_thrift_server( */
/*   request: *const ServerCreationRequest, */
/*   response: *mut ServerCreationResponse, */
/* ) */
/* { */


/*   let ret = Box::new(BasicServer { buffer_handle }); */
/*   unsafe { */
/*     *response = ServerCreationResponse::Success(Box::into_raw(ret)); */
/*   } */
/* } */

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
