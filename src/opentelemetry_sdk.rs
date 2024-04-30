use std::{cell::RefCell, collections::HashMap, sync::Mutex, time::SystemTime};

use rand::{rngs, Rng, SeedableRng};
use tracing::{field::Visit, span};
use tracing_subscriber::{layer::Context, Layer};

thread_local! {
    static CURRENT_RNG: RefCell<rngs::SmallRng> = RefCell::new(rngs::SmallRng::from_entropy());
}

#[derive(Clone, PartialEq, Eq, Copy, Hash, Debug)]
pub struct TraceId(u128);

#[derive(Clone, PartialEq, Eq, Copy, Hash, Debug)]
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
}

impl OTelSpan {
    pub fn new(
        name: String,
        trace_id: Option<TraceId>,
        parent_span_id: Option<SpanId>,
    ) -> OTelSpan {
        OTelSpan {
            name,
            trace_id: trace_id.unwrap_or_else(|| {
                CURRENT_RNG.with(|rng| TraceId::from(rng.borrow_mut().gen::<u128>()))
            }),
            span_id: CURRENT_RNG.with(|rng| SpanId::from(rng.borrow_mut().gen::<u64>())),
            parent_span_id,
            start_time: SystemTime::now(),
            end_time: SystemTime::now(),
            attributes: HashMap::new(),
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

pub struct OpenTelemetrySdk {
    spans: Mutex<HashMap<span::Id, OTelSpan>>,
}

impl Default for OpenTelemetrySdk {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenTelemetrySdk {
    pub fn new() -> OpenTelemetrySdk {
        OpenTelemetrySdk {
            spans: Mutex::new(HashMap::new()),
        }
    }
}

impl<S> Layer<S> for OpenTelemetrySdk
where
    S: tracing::Subscriber,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        // This is where Samplers are called, and its results can be used to populate fields like is_recording
        // Spans are stored in a self managed HashMap for demo purposes. But this could
        // be replaced with span.extensions like done by `tracing-opentelemetry`.
        let parent_span = ctx.current_span();
        if let Some(parent_id) = parent_span.id() {
            let parent_trace_id = self.spans.lock().unwrap().get(parent_id).unwrap().trace_id;
            let parent_span_id = self.spans.lock().unwrap().get(parent_id).unwrap().span_id;
            let mut span = OTelSpan::new(
                attrs.metadata().name().to_string(),
                Some(parent_trace_id),
                Some(parent_span_id),
            );
            attrs.record(&mut span);
            self.spans.lock().unwrap().insert(id.clone(), span);
        } else {
            let mut span = OTelSpan::new(attrs.metadata().name().to_string(), None, None);
            attrs.record(&mut span);
            self.spans.lock().unwrap().insert(id.clone(), span);
        }

        // This is where SpanProcessors' OnBegin will be called.
    }

    fn on_close(&self, id: span::Id, _ctx: Context<'_, S>) {
        let mut span = self.spans.lock().unwrap().remove(&id).unwrap();
        span.end_time = SystemTime::now();
        // Simply printing the span for now.
        // This is where SpanProcessors' OnEnd will be called.
        // SpanProcessors can pass Spans to exporter(s) which can export in OTLP format/others.
        println!("Span {:?}", span)
    }

    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, _ctx: Context<'_, S>) {
        if let Some(existing_span) = self.spans.lock().unwrap().get_mut(&span) {
            values.record(existing_span);
        }
    }
}
