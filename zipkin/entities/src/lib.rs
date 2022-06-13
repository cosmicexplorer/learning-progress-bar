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

pub mod components {
  use lazy_static::lazy_static;
  use regex::Regex;
  use serde_json::{self, json};
  use uuid::Uuid;

  use std::{collections::HashMap, time::SystemTime};

  #[derive(Debug)]
  pub struct SpanGenerationError(String);

  pub trait ParseableObject<T>: Sized {
    fn parse(input: T) -> Result<Self, SpanGenerationError>;
    fn extract(&self) -> T;
  }

  pub trait UuidGeneratable: Sized {
    fn generate_new() -> Result<Self, SpanGenerationError>;

    fn uuid_string(min_length: usize) -> String {
      let mut ret: String = "".to_string();
      let mut buf = Uuid::encode_buffer();
      while ret.len() < min_length {
        let uuid = Uuid::new_v4();
        let cur_hex_str = uuid.to_simple().encode_lower(&mut buf);
        assert!(cur_hex_str.len() > 0);
        ret = ret + cur_hex_str;
      }
      ret
    }
  }

  /* See https://zipkin.io/zipkin-api/#/default/post_spans for background! */
  lazy_static! {
    static ref TRACE_ID_PATTERN: Regex = Regex::new("[a-f0-9]{16,32}").unwrap();
    static ref SIXTEEN_LENGTH_PATTERN: Regex = Regex::new("[a-f0-9]{16}").unwrap();
  }

  #[derive(Debug, Clone)]
  pub struct TraceId {
    id: String,
  }

  impl UuidGeneratable for TraceId {
    fn generate_new() -> Result<Self, SpanGenerationError> {
      let s = Self::uuid_string(32)[..32].to_string();
      Self::parse(s)
    }
  }

  impl ParseableObject<String> for TraceId {
    fn parse(id: String) -> Result<Self, SpanGenerationError> {
      if TRACE_ID_PATTERN.is_match(&id) {
        Ok(TraceId { id })
      } else {
        Err(SpanGenerationError(format!(
          "trace id {:?} must match the regex: {:?}",
          id, *TRACE_ID_PATTERN
        )))
      }
    }

    fn extract(&self) -> String { self.id.clone() }
  }

  #[derive(Debug, Clone)]
  pub struct ParentId(pub Id);

  #[derive(Debug, Clone)]
  pub struct Id {
    id: String,
  }

  impl UuidGeneratable for Id {
    fn generate_new() -> Result<Self, SpanGenerationError> {
      let s = Self::uuid_string(16)[..16].to_string();
      Self::parse(s)
    }
  }

  impl ParseableObject<String> for Id {
    fn parse(id: String) -> Result<Self, SpanGenerationError> {
      if SIXTEEN_LENGTH_PATTERN.is_match(&id) {
        Ok(Id { id })
      } else {
        Err(SpanGenerationError(format!(
          "id {:?} must match the regex: {:?}",
          id, *SIXTEEN_LENGTH_PATTERN
        )))
      }
    }

    fn extract(&self) -> String { self.id.clone() }
  }

  #[derive(Debug, Clone)]
  pub enum Kind {
    Client,
    Server,
    Producer,
    Consumer,
  }

  impl ParseableObject<String> for Kind {
    fn parse(kind: String) -> Result<Self, SpanGenerationError> {
      match kind.as_str() {
        "CLIENT" => Ok(Self::Client),
        "SERVER" => Ok(Self::Server),
        "PRODUCER" => Ok(Self::Producer),
        "CONSUMER" => Ok(Self::Consumer),
        s => Err(SpanGenerationError(format!(
          "unrecognized span kind {:?}",
          s
        ))),
      }
    }

    fn extract(&self) -> String {
      match self {
        Self::Client => "CLIENT".to_string(),
        Self::Server => "SERVER".to_string(),
        Self::Producer => "PRODUCER".to_string(),
        Self::Consumer => "CONSUMER".to_string(),
      }
    }
  }

  ///
  /// integer($int64)
  /// minimum: 1
  /// https://zipkin.io/zipkin-api/#/default/post_spans
  #[derive(Debug, Clone)]
  pub struct Duration(i64);

  impl Duration {
    pub fn since(timestamp: ZipkinTimestamp) -> Result<Self, SpanGenerationError> {
      let now = ZipkinTimestamp::now();
      Self::parse(now.as_micros() - timestamp.as_micros())
    }
  }

  impl ParseableObject<i64> for Duration {
    fn parse(duration: i64) -> Result<Self, SpanGenerationError> {
      if duration <= 0 {
        Err(SpanGenerationError(format!(
          "invalid duration: {:?} -- the minimum value is 1",
          duration
        )))
      } else {
        Ok(Duration(duration))
      }
    }

    fn extract(&self) -> i64 { self.0 }
  }

  ///
  /// port	integer
  /// Depending on context, this could be a listen port or the client-side of a
  /// socket. Absent if unknown. Please don’t set to zero.
  /// https://zipkin.io/zipkin-api/#/default/post_spans
  #[derive(Debug, Clone)]
  pub struct Port(i64);

  impl ParseableObject<i64> for Port {
    fn parse(port: i64) -> Result<Self, SpanGenerationError> {
      if port == 0 {
        Err(SpanGenerationError(format!(
          "invalid port: {:?} -- cannot be zero",
          port
        )))
      } else {
        Ok(Port(port))
      }
    }

    fn extract(&self) -> i64 { self.0 }
  }

  /* See https://github.com/openzipkin/zipkin-api/blob/master/zipkin2-api.yaml! */
  #[derive(Debug, Clone)]
  #[allow(non_snake_case)]
  pub struct Endpoint {
    pub serviceName: Option<String>,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub port: Option<Port>,
  }

  impl ParseableObject<serde_json::Value> for Endpoint {
    fn parse(_input: serde_json::Value) -> Result<Self, SpanGenerationError> {
      unimplemented!();
    }

    fn extract(&self) -> serde_json::Value {
      json!({
        "serviceName": self.serviceName,
        "ipv4": self.ipv4,
        "ipv6": self.ipv6,
        "port": self.port.as_ref().map(|p| p.extract()),
      })
    }
  }

  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct Annotation {
    pub timestamp: i64,
    pub value: String,
  }

  impl ParseableObject<serde_json::Value> for Annotation {
    fn parse(_input: serde_json::Value) -> Result<Self, SpanGenerationError> {
      unimplemented!();
    }

    fn extract(&self) -> serde_json::Value {
      json!({
        "timestamp": self.timestamp,
        "value": self.value,
      })
    }
  }

  #[derive(Debug, Clone)]
  pub struct ZipkinTimestamp(i64);

  impl ZipkinTimestamp {
    pub fn now() -> Self {
      let micros = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time will never be before the epoch!!")
        .as_micros();
      Self(micros as i64)
    }

    pub fn as_micros(&self) -> i64 { self.0 }
  }

  #[derive(Debug)]
  pub struct SpanBuilder {
    id: Id,
    name: Option<String>,
    parent_id: Option<ParentId>,
    pub timestamp: Option<ZipkinTimestamp>,
    pub annotations: Vec<Annotation>,
    pub tags: HashMap<String, Option<String>>,
  }

  impl SpanBuilder {
    pub fn new(
      name: Option<String>,
      parent_id: Option<ParentId>,
      fields: Vec<String>,
    ) -> Result<Self, SpanGenerationError>
    {
      Ok(Self {
        id: Id::generate_new()?,
        name,
        parent_id,
        timestamp: None,
        annotations: Vec::new(),
        tags: fields.into_iter().map(|f| (f, None)).collect(),
      })
    }

    pub fn id(&self) -> Id { self.id.clone() }

    pub fn enter(&mut self) { self.timestamp = Some(ZipkinTimestamp::now()); }

    pub fn exit(&mut self, trace_id: TraceId) -> Result<ZipkinSpan, SpanGenerationError> {
      let start = self.timestamp.clone().ok_or_else(|| {
        SpanGenerationError(format!(
          "expected a timestamp to have been set upon .enter()!"
        ))
      })?;
      let ret = ZipkinSpan::build(
        trace_id,
        self.id.clone(),
        self.name.clone(),
        self.parent_id.clone(),
        start.clone(),
        Duration::since(start)?,
        self.annotations.clone(),
        self.tags.clone(),
      )?;
      self.timestamp = None;
      Ok(ret)
    }
  }

  /* FIXME: I want to impl Default to create defaults for some, but not all
   * fields. Is this possible? */
  #[derive(Debug, Clone)]
  #[allow(non_snake_case)]
  pub struct ZipkinSpan {
    traceId: TraceId,
    name: Option<String>,
    parentId: Option<ParentId>,
    id: Id,
    kind: Option<Kind>,
    timestamp: ZipkinTimestamp,
    duration: Duration,
    debug: bool,
    shared: bool,
    localEndpoint: Option<Endpoint>,
    remoteEndpoint: Option<Endpoint>,
    annotations: Vec<Annotation>,
    tags: HashMap<String, Option<String>>,
  }

  impl ZipkinSpan {
    pub fn build(
      trace_id: TraceId,
      id: Id,
      name: Option<String>,
      parent_id: Option<ParentId>,
      timestamp: ZipkinTimestamp,
      duration: Duration,
      annotations: Vec<Annotation>,
      tags: HashMap<String, Option<String>>,
    ) -> Result<Self, SpanGenerationError>
    {
      Ok(Self {
        traceId: trace_id,
        name,
        parentId: parent_id,
        id,
        kind: None,
        timestamp,
        duration,
        debug: false,
        shared: false,
        localEndpoint: None,
        remoteEndpoint: None,
        annotations,
        tags,
      })
    }
  }

  impl ParseableObject<serde_json::Value> for ZipkinSpan {
    fn parse(_input: serde_json::Value) -> Result<Self, SpanGenerationError> {
      unimplemented!();
    }

    fn extract(&self) -> serde_json::Value {
      json!({
        "traceId": self.traceId.extract(),
        "name": self.name,
        "parentId": self.parentId.as_ref().map(|p| p.0.extract()),
        "id": self.id.extract(),
        "kind": self.kind.as_ref().map(|k| k.extract()),
        "timestamp": self.timestamp.as_micros(),
        "duration": self.duration.extract(),
        "debug": self.debug,
        "shared": self.shared,
        "localEndpoint": self.localEndpoint.as_ref().map(|e| e.extract()),
        "remoteEndpoint": self.remoteEndpoint.as_ref().map(|e| e.extract()),
        "annotations": self.annotations.iter().map(|a| a.extract()).collect::<Vec<_>>(),
        "tags": self.tags.iter().collect::<HashMap<_, _>>(),
      })
    }
  }
}

/* async_trait makes a lot of things difficult */
#[allow(unused_imports)]
pub mod sink {
  use super::components::*;
  use coroutines::*;

  use async_trait::async_trait;

  #[derive(Debug)]
  pub struct SpanSinkError(pub String);

  #[async_trait]
  pub trait SpanSink: coroutines::SyncRwAccessable {
    async fn post_spans(&mut self, spans: &[ZipkinSpan]) -> Result<(), SpanSinkError>;
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn it_works() {
    assert_eq!(2 + 2, 4);
  }
}
