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

pub mod subscriber {
  use coroutines::{StateVar, SyncRwAccessable, SyncRwBuf};
  use entities::{components::*, sink::*};

  use futures::executor::block_on;
  use parking_lot::Mutex;
  use serde_json::json;
  use tracing::{
    field::{Field, Visit},
    span, Event, Metadata, Subscriber,
  };

  use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{
      atomic::{AtomicUsize, Ordering},
      Arc,
    },
  };

  #[derive(Debug, Clone, Eq, PartialEq, Hash)]
  struct SpanId(span::Id);

  impl SpanId {
    pub fn new(id: &span::Id) -> Self { Self(id.clone()) }

    pub fn as_tracing_id(self) -> span::Id { self.0 }
  }

  #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
  pub enum IntermediateSpansState {
    Ready,
    NeedsSpansFlushed,
    Exited,
  }

  pub struct ZipkinSubscriber<Sink: SpanSink+'static> {
    trace_id: TraceId,
    next_id: AtomicUsize,
    span_map: Mutex<HashMap<SpanId, Mutex<SpanBuilder>>>,
    state: Arc<StateVar<IntermediateSpansState>>,
    waiting_spans: Mutex<Vec<ZipkinSpan>>,
    sink: SyncRwBuf<Sink>,
  }

  pub struct SubscriberWrapper<T: Subscriber+'static> {
    pub inner: Arc<T>,
  }

  impl<Sink: SpanSink> ZipkinSubscriber<Sink> {
    pub fn new(sink: Arc<Sink>) -> Result<Self, SpanGenerationError> {
      Ok(Self {
        trace_id: TraceId::generate_new()?,
        next_id: AtomicUsize::new(1),
        span_map: Mutex::new(HashMap::new()),
        state: Arc::new(StateVar::new(IntermediateSpansState::Ready)),
        waiting_spans: Mutex::new(Vec::new()),
        sink: SyncRwBuf::new(sink),
      })
    }

    fn flush_spans(&mut self) -> Vec<ZipkinSpan> {
      let ZipkinSubscriber {
        ref waiting_spans,
        ref state,
        ..
      } = self;

      let mut ret: Vec<ZipkinSpan> = vec![];

      state.notify_all_of_new_state(|state| match state {
        &IntermediateSpansState::NeedsSpansFlushed => {
          {
            let mut waiting_spans = waiting_spans.lock();
            ret.extend(waiting_spans.drain(..));
          }
          IntermediateSpansState::Ready
        },
        x => *x,
      });

      ret
    }

    pub async fn repeatedly_flush(&mut self) -> Result<(), SpanSinkError> {
      self.state.clone().wait_for_slot_to_completely_execute(
        &IntermediateSpansState::Exited,
        move || {
          let spans_to_flush = self.flush_spans();
          block_on(self.sink.post_spans(&spans_to_flush))?;
          Ok(IntermediateSpansState::Ready)
        },
      )?;
      Ok(())
    }
  }

  impl<T: Subscriber+'static> Subscriber for SubscriberWrapper<T> {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool { self.inner.enabled(metadata) }

    fn new_span(&self, attrs: &span::Attributes<'_>) -> span::Id { self.inner.new_span(attrs) }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) { self.inner.record(span, values) }

    fn event(&self, event: &Event<'_>) { self.inner.event(event) }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
      self.inner.record_follows_from(span, follows)
    }

    fn enter(&self, span: &span::Id) { self.inner.enter(span) }

    fn exit(&self, span: &span::Id) { self.inner.exit(span) }
  }

  unsafe impl<Sink: SpanSink> SyncRwAccessable for ZipkinSubscriber<Sink> {}

  struct RecordVisitor<'a> {
    span_builder: &'a mut SpanBuilder,
  }

  impl<'a> RecordVisitor<'a> {
    pub fn new(span_builder: &'a mut SpanBuilder) -> Self { Self { span_builder } }
  }

  impl<'a> Visit for RecordVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
      self
        .span_builder
        .tags
        .insert(field.as_ref().to_string(), Some(format!("{:?}", value)));
    }
  }

  struct EventVisitor<'a> {
    span_builder: &'a mut SpanBuilder,
  }

  impl<'a> EventVisitor<'a> {
    pub fn new(span_builder: &'a mut SpanBuilder) -> Self { Self { span_builder } }

    fn create_annotation(field: &Field, value: &dyn Debug) -> Annotation {
      let timestamp = ZipkinTimestamp::now().as_micros();
      let value = json!({
        "field": field.as_ref(),
        "value": format!("{:?}", value),
      })
      .to_string();

      Annotation { timestamp, value }
    }
  }

  impl<'a> Visit for EventVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
      self
        .span_builder
        .annotations
        .push(Self::create_annotation(field, value));
    }
  }

  impl<Sink: SpanSink> Subscriber for ZipkinSubscriber<Sink> {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool { true }

    fn new_span(&self, attrs: &span::Attributes<'_>) -> span::Id {
      let span_id = {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let tracing_internal_id = span::Id::from_u64(id as u64);
        SpanId(tracing_internal_id)
      };

      let metadata = attrs.metadata();

      let name = metadata.name().to_string();

      let parent_id = attrs.parent().map(SpanId::new).map(|parent_id| {
        let span_map = self.span_map.lock();
        let parent_span = span_map
          .get(&parent_id)
          .expect(&format!(
            "parent_id {:?} was *definitely* supposed to be in this map!! the mapping was: {:?}!",
            parent_id, *span_map,
          ))
          .lock();
        ParentId(parent_span.id())
      });

      let fields: Vec<String> = metadata
        .fields()
        .iter()
        .map(|s| s.as_ref().to_string())
        .collect();

      let mut span_builder = SpanBuilder::new(Some(name), parent_id, fields).unwrap();

      span_builder.tags.extend(
        vec![
          ("target".to_string(), Some(metadata.target().to_string())),
          ("level".to_string(), Some(format!("{}", metadata.level()))),
          (
            "module_path".to_string(),
            metadata.module_path().map(|s| s.to_string()),
          ),
          ("file".to_string(), metadata.file().map(|s| s.to_string())),
          ("line".to_string(), metadata.line().map(|s| s.to_string())),
        ]
        .into_iter(),
      );

      /* Temporary locking scope. */
      {
        let mut span_map = self.span_map.lock();
        assert!(!span_map.contains_key(&span_id));
        span_map.insert(span_id.clone(), Mutex::new(span_builder));
      }

      span_id.as_tracing_id()
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
      let span_map = self.span_map.lock();
      let mut span_builder = span_map.get(&SpanId(span.clone())).unwrap().lock();
      values.record(&mut RecordVisitor::new(&mut *span_builder));
    }

    /* TODO: consider recording Events as their own instantaneous zipkin
     * span? */
    fn event(&self, event: &Event<'_>) {
      let span_map = self.span_map.lock();
      let mut parent_span_builder = event
        .parent()
        .map(|span| span_map.get(&SpanId(span.clone())).unwrap())
        .expect("an Event is currently expected to always originate from some parent span!")
        .lock();
      event.record(&mut EventVisitor::new(&mut *parent_span_builder));
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
      unimplemented!();
    }

    fn enter(&self, span: &span::Id) {
      let span_map = self.span_map.lock();
      let mut span_builder = span_map.get(&SpanId(span.clone())).unwrap().lock();
      span_builder.enter();
    }

    fn exit(&self, span: &span::Id) {
      let ZipkinSubscriber {
        ref span_map,
        ref trace_id,
        ref waiting_spans,
        ref state,
        ..
      } = self;
      let span_map = span_map.lock();
      let mut span_builder = span_map.get(&SpanId(span.clone())).unwrap().lock();
      let completed_span = span_builder.exit(trace_id.clone()).unwrap();

      state.notify_all_of_new_state(move |state| {
        let mut waiting_spans = waiting_spans.lock();
        waiting_spans.push(completed_span.clone());

        match state {
          IntermediateSpansState::Ready if waiting_spans.len() > 10 => {
            IntermediateSpansState::NeedsSpansFlushed
          },
          x => *x,
        }
      });
    }
  }
}

pub mod registration {
  use super::subscriber::*;
  use connect::ZipkinServer;
  use coroutines::SyncRwBuf;

  use futures::executor::block_on;
  use lazy_static::lazy_static;
  use tracing::Subscriber;
  use tracing_subscriber::{layer::Layer, EnvFilter};

  use std::sync::Arc;

  pub async fn set_default_subscriber<T: 'static+Subscriber+Sized+Send+Sync>(
    subscriber: T,
  ) -> Result<(), tracing::dispatcher::SetGlobalDefaultError> {
    let filter = EnvFilter::from_default_env();
    let wrapped = filter.with_subscriber(subscriber);

    tracing::subscriber::set_global_default(wrapped)
  }

  lazy_static! {
    static ref ZIPKIN_SUBSCRIBER: SyncRwBuf<ZipkinSubscriber<ZipkinServer>> = {
      let sink = ZipkinServer::new("localhost:9411/api/v2".to_string());
      let subscriber = ZipkinSubscriber::new(Arc::new(sink)).unwrap();
      SyncRwBuf::new(Arc::new(subscriber))
    };
  }

  #[repr(C)]
  pub enum ReturnValue {
    Success,
    Error,
  }

  #[no_mangle]
  pub extern "C" fn set_default_tracing_subscriber() -> ReturnValue {
    let subscriber_ref = ZIPKIN_SUBSCRIBER.clone();
    let subscriber_wrapper = SubscriberWrapper {
      inner: subscriber_ref.into_arc(),
    };
    match block_on(set_default_subscriber(subscriber_wrapper)) {
      Ok(_) => ReturnValue::Success,
      Err(_) => ReturnValue::Error,
    }
  }

  #[no_mangle]
  pub extern "C" fn wait_on_flushing() -> ReturnValue {
    let subscriber = &mut *ZIPKIN_SUBSCRIBER.clone();
    match block_on(subscriber.repeatedly_flush()) {
      Ok(_) => ReturnValue::Success,
      Err(_) => ReturnValue::Error,
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
