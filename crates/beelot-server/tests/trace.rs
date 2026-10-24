//! Alone in its binary: a test running beside it without a subscriber would cache the request span as
//! uninteresting.

mod common;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use beelot_server::app::router;
use common::test_state;
use tower::ServiceExt;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Every field value of every span, as text.
#[derive(Clone, Default)]
struct SpanFields(Arc<Mutex<Vec<String>>>);

struct Collect<'a>(&'a mut Vec<String>);

impl Visit for Collect<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push(format!("{}={value:?}", field.name()));
    }
}

impl<S: tracing::Subscriber> Layer<S> for SpanFields {
    fn on_new_span(&self, attrs: &Attributes<'_>, _: &Id, _: Context<'_, S>) {
        attrs.record(&mut Collect(&mut self.0.lock().unwrap()));
    }

    fn on_record(&self, _: &Id, values: &Record<'_>, _: Context<'_, S>) {
        values.record(&mut Collect(&mut self.0.lock().unwrap()));
    }

    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        event.record(&mut Collect(&mut self.0.lock().unwrap()));
    }
}

#[tokio::test]
async fn trace_spans_omit_query_strings() {
    let fields = SpanFields::default();
    let subscriber = tracing_subscriber::registry().with(fields.clone());
    let _guard = tracing::subscriber::set_default(subscriber);

    let response = router(test_state())
        .oneshot(
            Request::get("/stats?token=secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let recorded = fields.0.lock().unwrap();
    assert!(
        recorded.iter().any(|field| field.contains("/stats")),
        "the path is recorded: {recorded:?}"
    );
    assert!(
        recorded.iter().any(|field| field.starts_with("request_id")),
        "the request id is recorded: {recorded:?}"
    );
    assert!(
        recorded.iter().all(|field| !field.contains("secret")),
        "{recorded:?}"
    );
}
