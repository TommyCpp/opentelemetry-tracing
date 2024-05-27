pub mod opentelemetry_sdk;
mod propagator;


// Define a simple macro
macro_rules! say_hello {
    () => {
        println!("Hello, world!");
    };
}

#[macro_export]
macro_rules! span_with_remote_parent {
    ($remote_parent:expr, $lvl:expr, $name:expr, $($fields:tt)*) => {
        {
            let span = tracing::span!(
                target: module_path!(),
                $lvl,
                $name,
                $($fields)*
            );
            span.set_parent($remote_parent);
            span
        }
    };
}