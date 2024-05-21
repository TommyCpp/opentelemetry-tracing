use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tracing::{field, Level, span, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use opentelemetry_tracing::opentelemetry_sdk;
use opentelemetry_tracing::opentelemetry_sdk::OtelSpanExt;


// An async function that consumes a request, does nothing with it and returns a
// response.
async fn hello(req: Request<impl hyper::body::Body>) -> Result<Response<Full<Bytes>>, Infallible> {
    let span = span!(
        Level::TRACE,
        "Main Span",
        attribute1 = "v1",
        attribute2 = "v2"
    );

    // NOTE(tommycpp): The reason why we need this function to change parent post span creation is
    // there is no way in tracing to create a "fake span"(a span that doesn't really in Registry or
    // localhost). But in distributed tracing, we need to create a span that doesn't exist in the
    // localhost.
    //
    // To support "fake span" we need:
    // 1. Add some information in Regitry to represent the "fake span", assign a tracing span Id for it
    // 2. Fake span cannot be entered or exited, users cannot add events onto it because it doesnt' exist in localhost
    // 3. Fake span can be used as parent for new spans.
    req.headers().get("uber-trace-id").map(|trace_id| {
        span.set_parent(trace_id.to_str().unwrap().to_string());
    });

    let _guard = span.enter();
    warn!(name: "my-event-name-inside-outer-span", event_id = 10, user_name = "otel");
    let span_inner = span!(
        Level::TRACE,
        "Inner Span",
        attribute1 = "v1",
        attribute2 = "v2",
    );
    let _guard_inner = span_inner.enter();
    span_inner.record("attribute3", "value3");
    warn!(name: "my-event-name-inside-inner-span", event_id = 20, user_name = "otel");

    Ok(Response::new(Full::new(Bytes::from("Hello World!"))))
}


#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let otel_sdk_layer = opentelemetry_sdk::OpenTelemetrySdk::new();
    tracing_subscriber::registry()
        // .with(fmt::layer()) // Uncomment this line to see the fmt layer in action
        .with(otel_sdk_layer)
        .init();


    // This address is localhost
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        // When an incoming TCP connection is received grab a TCP stream for
        // client<->server communication.
        //
        // Note, this is a .await point, this loop will loop forever but is not a busy loop. The
        // .await point allows the Tokio runtime to pull the task off of the thread until the task
        // has work to do. In this case, a connection arrives on the port we are listening on and
        // the task is woken up, at which point the task is then put back on a thread, and is
        // driven forward by the runtime, eventually yielding a TCP stream.
        let (tcp, _) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(tcp);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `hello` function
            if let Err(err) = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(io, service_fn(hello))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}