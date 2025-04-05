use std::env;

use tracing::{Subscriber, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
};

pub fn get_telemetry_subscriber<Sink>(
    name: &str,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Send + Sync
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name.into(), sink);

    // Only output logs to a file if the runtime has given an output path
    let maybe_file_layer = match env::var("LOGS_DIRECTORY") {
        Ok(logs_dir) => {
            let file_appender =
                tracing_appender::rolling::never(logs_dir, format!("{}.log", &name));
            let file_layer = fmt::layer().with_writer(file_appender);
            Some(file_layer)
        }
        Err(_) => None,
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .with(maybe_file_layer)
}

/// Initialise the telemetry stack by setting up the global
/// default telemetry subscriber. The subscriber will handle log and tracing
/// events based on the pre-configured layers.
pub fn init_telemetry_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Could not set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
