use std::{cell::RefCell, collections::HashMap, time::SystemTime};
use std::fmt::format;

use rand::{rngs, Rng, SeedableRng};
use tracing::{field::Visit, span, Event, Span};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, Registry};

thread_local! {
    static CURRENT_RNG: RefCell<rngs::SmallRng> = RefCell::new(rngs::SmallRng::from_entropy());
}

#[derive(Clone, PartialEq, Eq, Copy, Hash, Debug, Default)]
pub struct TraceId(u128);

#[derive(Clone, PartialEq, Eq, Copy, Hash, Debug, Default)]
pub struct SpanId(u64);

impl From<u128> for TraceId {
    fn from(value: u128) -> Self {
        TraceId(value)
    }
}

impl From<u64> for SpanId {
    fn from(value: u64) -> Self {
        SpanId(value)
    }
}

#[derive(Debug)]
pub struct OTelSpan {
    pub name: String,
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub attributes: HashMap<String, String>,
    pub is_recording: bool,
}

impl OTelSpan {
    pub fn new(
        name: String,
        trace_id: TraceId,
        parent_span_id: Option<SpanId>,
        is_recording: bool,
    ) -> OTelSpan {
        OTelSpan {
            name,
            trace_id: trace_id,
            span_id: CURRENT_RNG.with(|rng| SpanId::from(rng.borrow_mut().gen::<u64>())),
            parent_span_id,
            start_time: SystemTime::now(),
            end_time: SystemTime::now(),
            attributes: HashMap::new(),
            is_recording,
        }
    }
}

impl Visit for OTelSpan {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.attributes
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.attributes
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

pub trait ShouldSample {
    fn should_sample(&self, trace_id: &TraceId) -> bool;
}

pub struct OTelSampler;

impl ShouldSample for OTelSampler {
    fn should_sample(&self, _trace_id: &TraceId) -> bool {
        true
    }
}

#[derive(PartialEq, Eq)]
pub enum EventExportMode {
    LogRecord,
    SpanEvent,
}

pub struct OpenTelemetrySdk {
    sampler: OTelSampler,
    event_export_mode: EventExportMode,
}

impl Default for OpenTelemetrySdk {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenTelemetrySdk {
    pub fn new() -> OpenTelemetrySdk {
        OpenTelemetrySdk {
            sampler: OTelSampler,
            event_export_mode: EventExportMode::SpanEvent,
        }
    }
}

impl<S> Layer<S> for OpenTelemetrySdk
    where
        S: tracing::Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span expected here");
        let mut extensions = span.extensions_mut();

        let parent_span = ctx.current_span();
        if let Some(parent_id) = parent_span.id() {
            // parent span exists.
            // reuse traceid for the new span being created
            // and store parent span id to the new span being created.
            let parent_span = ctx.span(parent_id).expect("Parent span expected here");
            let mut parent_extensions = parent_span.extensions_mut();
            let parent_span = parent_extensions
                .get_mut::<OTelSpan>()
                .expect("Parent span data expected here");

            let parent_trace_id = parent_span.trace_id;
            let parent_span_id = parent_span.span_id;

            // Overly simplified sampling logic for POC.
            let sampling_result = self.sampler.should_sample(&parent_trace_id);
            let mut span = OTelSpan::new(
                attrs.metadata().name().to_string(),
                parent_trace_id,
                Some(parent_span_id),
                sampling_result,
            );
            attrs.record(&mut span);

            // store span in span extension.
            extensions.insert(span);
        } else {
            // parent span does not exist.
            // TODO: This is where remote parent's span context needs to be extracted, if any.
            let trace_id_to_be_created_span =
                CURRENT_RNG.with(|rng| TraceId::from(rng.borrow_mut().gen::<u128>()));
            let sampling_result = self.sampler.should_sample(&trace_id_to_be_created_span);
            let mut span = OTelSpan::new(
                attrs.metadata().name().to_string(),
                trace_id_to_be_created_span,
                None,
                sampling_result,
            );
            attrs.record(&mut span);

            // store span in span extension.
            extensions.insert(span);
        }

        // This is where SpanProcessors' OnBegin will be called.
    }

    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("Span expected here");
        let mut extensions = span.extensions_mut();
        let mut span = extensions.remove::<OTelSpan>().expect("Span expected here");
        span.end_time = SystemTime::now();
        println!("Span {:?}", span);
        if span.is_recording {
            // This is where SpanProcessors' OnEnd will be called.
            // SpanProcessors can pass Spans to exporter(s) which can export in OTLP format/others.
        }
    }

    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(span).expect("Span expected here");
        let mut extensions = span.extensions_mut();
        let existing_span = extensions
            .get_mut::<OTelSpan>()
            .expect("Span expected here");
        values.record(existing_span);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if event.metadata().is_event() {
            if let Some(span) = event.parent().and_then(|id| ctx.span(id)).or_else(|| {
                event
                    .is_contextual()
                    .then(|| ctx.lookup_current())
                    .flatten()
            }) {
                let mut extensions = span.extensions_mut();
                let existing_span = extensions
                    .get_mut::<OTelSpan>()
                    .expect("Span expected here");

                if self.event_export_mode == EventExportMode::SpanEvent {
                    if existing_span.is_recording {
                        // Add SpanEvent to the Span.
                        println!("SpanEvent {} for Span with SpanId {}", event.metadata().name(), existing_span.span_id.0);
                    }
                } else {
                    // Emit LogRecord using the Event, similar to how opentelemetry-tracing-appender works today.
                    println!("LogRecord {} for Span with SpanId {}", event.metadata().name(), existing_span.span_id.0);
                }
            }
        }
    }
}

pub trait OtelSpanExt {
    fn set_parent(&self, jaeger_format: String);

    fn tract_id(&self) -> TraceId;

    fn span_id(&self) -> SpanId;

    fn parent_span_id(&self) -> SpanId;

    fn extract_jaeger_propagation(&self) -> String;

    fn with_otel_span<F, T>(&self, f: F) -> T
        where F: Fn(&OTelSpan) -> Option<T>,
              T: Default;
}

impl OtelSpanExt for Span {
    fn set_parent(&self, jaeger_format: String) {
        self.with_subscriber(move |(id, subscriber)| {
            if let Some(registry) = subscriber.downcast_ref::<Registry>() {
                let span = registry
                    .span(id)
                    .expect("registry should have a span for the current ID");

                let mut extensions = span.extensions_mut();
                if let Some(otel_span) = extensions.get_mut::<OTelSpan>() {
                    let (trace_id, span_id) = parse_jaeger_trace_id(&jaeger_format);
                    otel_span.trace_id = trace_id;
                    otel_span.parent_span_id = Some(span_id);
                }
            }
        });
    }

    fn tract_id(&self) -> TraceId {
        self.with_otel_span(|otel_span| Some(otel_span.trace_id))
    }

    fn span_id(&self) -> SpanId {
        self.with_otel_span(|otel_span| Some(otel_span.span_id))
    }

    fn parent_span_id(&self) -> SpanId {
        self.with_otel_span(|otel_span| otel_span.parent_span_id)
    }

    // Get the span, extract trace id, span id, parent span id and sampling decision
    // build a jaeger propagation header.
    fn extract_jaeger_propagation(&self) -> String {
        return format!("{}:{}:{}:{}", self.tract_id().0, self.span_id().0, self.parent_span_id().0, 1);
    }

    fn with_otel_span<F, T>(&self, f: F) -> T
        where F: Fn(&OTelSpan) -> Option<T>,
              T: Default {
        let mut result: Option<T> = None;
        self.with_subscriber(|(id, subscriber)| {
            if let Some(registry) = subscriber.downcast_ref::<Registry>() {
                let span = registry
                    .span(id)
                    .expect("registry should have a span for the current ID");

                let mut extensions = span.extensions_mut();
                if let Some(otel_span) = extensions.get_mut::<OTelSpan>() {
                    result = f(otel_span);
                }
            }
        });
        result.unwrap_or_default()
    }
}


fn parse_jaeger_trace_id(header_value: &str) -> (TraceId, SpanId) {
    let parts: Vec<&str> = header_value.split(':').collect();
    if parts.len() != 4 {
        return (TraceId::default(), SpanId::default());
    }

    let trace_id_str = parts[0];
    let span_id_str = parts[1];

    let trace_id = u128::from_str_radix(trace_id_str, 10).unwrap_or(0);
    let span_id = u64::from_str_radix(span_id_str, 10).unwrap_or(0);

    (TraceId(trace_id), SpanId(span_id))
}