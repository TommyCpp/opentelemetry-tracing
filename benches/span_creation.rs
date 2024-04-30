use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry_tracing::opentelemetry_sdk;
use tracing::{span, Level};
use tracing_subscriber::prelude::*;

pub fn span_creation_benchmark(c: &mut Criterion) {
    let otel_sdk_layer = opentelemetry_sdk::OpenTelemetrySdk::new();
    tracing_subscriber::registry()
        // .with(fmt::layer()) // Uncomment this line to see the fmt layer in action
        .with(otel_sdk_layer)
        .init();

    c.bench_function("span_creation", |b| {
        b.iter(|| {
            let span = span!(
                Level::TRACE,
                "Main Span",
                attribute1 = "v1",
                attribute2 = "v2",
                attribute3 = "v3",
                attribute4 = "v4",
                attribute5 = "v5",
            );
            let _guard = span.enter();
        });
    });
}

criterion_group!(benches, span_creation_benchmark);
criterion_main!(benches);
