use std::{env, time::Duration};

use opentelemetry::{KeyValue, trace::TracerProvider};
use opentelemetry_otlp::{MetricExporter, SpanExporter};
use opentelemetry_sdk::metrics::{SdkMeterProvider, periodic_reader_with_async_runtime};
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, span_processor_with_async_runtime};
use opentelemetry_sdk::{Resource, runtime};
use opentelemetry_semantic_conventions::resource::SERVICE_VERSION;
use tracing::{Subscriber, subscriber::set_global_default};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Registry;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, MakeWriter},
    layer::SubscriberExt,
};

pub fn get_telemetry_subscriber<Sink>(
    name: &'static str,
    version: &'static str,
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

    let span_exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Could not create tracer");

    let batch_processor = span_processor_with_async_runtime::BatchSpanProcessor::builder(
        span_exporter,
        runtime::Tokio,
    )
    .build();

    let metrics_exporter = MetricExporter::builder()
        .with_tonic()
        .build()
        .expect("could not create Metrics exporter");

    let periodic_reader = periodic_reader_with_async_runtime::PeriodicReader::builder(
        metrics_exporter,
        runtime::Tokio,
    )
    .with_interval(Duration::from_secs(2))
    .build();

    let tracer = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_span_processor(batch_processor)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_max_events_per_span(64)
        .with_max_attributes_per_span(16)
        .with_resource(get_resource(name, version))
        .build()
        .tracer(name);

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(periodic_reader)
        .with_resource(get_resource(name, version))
        .build();

    let opentelemetry_layer: OpenTelemetryLayer<Registry, _> = OpenTelemetryLayer::new(tracer);

    opentelemetry::global::set_meter_provider(meter_provider);

    tracing_subscriber::registry()
        .with(opentelemetry_layer)
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .with(maybe_file_layer)
}

/// Generate a resource with all the common markers for our traces and metrics
fn get_resource(service_name: &str, version: &str) -> Resource {
    Resource::builder()
        .with_service_name(service_name.to_owned())
        .with_attribute(KeyValue::new(SERVICE_VERSION, version.to_owned()))
        .build()
}

/// Initialise the telemetry stack by setting up the global
/// default telemetry subscriber. The subscriber will handle log and tracing
/// events based on the pre-configured layers.
pub fn init_telemetry_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Could not set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
