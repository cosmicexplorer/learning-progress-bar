/* NB: Any nightly-only features go here >=]! */
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
// We only use unsafe pointer dereference in our no_mangle exposed API, but it is nicer to list
// just the one minor call as unsafe, than to mark the whole function as unsafe which may hide
// other unsafeness.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use coroutines::SyncRwAccessable;
use entities::{
  components::{ParseableObject, ZipkinSpan},
  sink::{SpanSink, SpanSinkError},
};

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::json;

use std::convert::From;

#[derive(Debug)]
pub struct ZipkinBackendError(String);

impl From<reqwest::Error> for ZipkinBackendError {
  fn from(e: reqwest::Error) -> Self { Self(format!("{:?}", e)) }
}

pub struct ZipkinServer {
  url: String,
  http_client: Client,
}

impl ZipkinServer {
  pub fn new(url: String) -> Self {
    Self {
      url,
      http_client: Client::new(),
    }
  }
}

unsafe impl SyncRwAccessable for ZipkinServer {}

#[async_trait]
impl SpanSink for ZipkinServer {
  async fn post_spans(&mut self, spans: &[ZipkinSpan]) -> Result<(), SpanSinkError> {
    let res = self
      .http_client
      .post(&self.url)
      .body(json!([spans.iter().map(|s| s.extract()).collect::<Vec<_>>()]).to_string())
      .send()
      .await
      .map_err(|e| SpanSinkError(format!("{:?}", e)))?;
    if res.status() != StatusCode::ACCEPTED {
      Err(SpanSinkError(format!(
        "non-202 (ACCEPTED) response: {:?}",
        res
      )))
    } else {
      Ok(())
    }
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
