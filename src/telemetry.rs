use tracing::{subscriber::set_global_default, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
    EnvFilter,
};

pub fn get_subscriber<Sink>(
    name: &str,
    env_filter: String,
    sink: Sink,
) -> (impl Subscriber + Send + Sync, WorkerGuard)
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name.into(), sink);
    let file_appender = tracing_appender::rolling::never("./logs", format!("{}.log", &name));
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::Layer::new().with_writer(non_blocking_appender);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .with(file_layer);

    (subscriber, guard)
}
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Could not set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
