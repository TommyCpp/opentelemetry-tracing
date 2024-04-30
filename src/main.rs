use opentelemetry_tracing::opentelemetry_sdk;
use tracing::{debug, error, field, info, span, trace, warn, Level};
use tracing_subscriber::prelude::*;

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
    let _guard = span.enter();
    let span_inner = span!(
        Level::TRACE,
        "Inner Span",
        attribute1 = "v1",
        attribute2 = "v2",
        attribute3 = field::Empty
    );
    let _guard_inner = span_inner.enter();
    span_inner.record("attribute3", "value3");
    warn!(name: "my-event-name", target: "my-system2", event_id = 20, user_name = "otel", user_email = "otel@opentelemetry.io");
}
