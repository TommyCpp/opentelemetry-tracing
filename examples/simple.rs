use opentelemetry_tracing::opentelemetry_sdk;
use tracing::{field, span, warn, Level};
use tracing_subscriber::prelude::*;
use opentelemetry_tracing::opentelemetry_sdk::OtelSpanExt;

// cargo run --example simple
fn main() {
    let otel_sdk_layer = opentelemetry_sdk::OpenTelemetrySdk::new();
    tracing_subscriber::registry()
        // .with(fmt::layer()) // Uncomment this line to see the fmt layer in action
        .with(otel_sdk_layer)
        .init();

    let span = span!(
        Level::TRACE,
        "Main Span",
        attribute1 = "v1",
        attribute2 = "v2"
    );
    span.set_parent("262603779606908057216172753575155927278:4855502779463763640:0:1".to_string());
    let _guard = span.enter();
    warn!(name: "my-event-name-inside-outer-span", event_id = 10, user_name = "otel");
    let span_inner = span!(
        Level::TRACE,
        "Inner Span",
        attribute1 = "v1",
        attribute2 = "v2",
        attribute3 = field::Empty
    );
    let _guard_inner = span_inner.enter();
    span_inner.record("attribute3", "value3");
    warn!(name: "my-event-name-inside-inner-span", event_id = 20, user_name = "otel");
}
