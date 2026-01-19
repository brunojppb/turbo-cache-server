use std::sync::{Arc, Mutex};
use std::{env, time::Duration};

use opentelemetry::metrics::ObservableGauge;
use opentelemetry::{KeyValue, trace::TracerProvider};
use opentelemetry_otlp::{MetricExporter, SpanExporter};
use opentelemetry_sdk::metrics::{SdkMeterProvider, periodic_reader_with_async_runtime};
use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, span_processor_with_async_runtime};
use opentelemetry_sdk::{Resource, runtime};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use sysinfo::{Pid, ProcessRefreshKind, System};
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

/// Holds the system metric gauge instances to keep them alive
/// for the duration of the application lifecycle
pub struct SystemMetrics {
    _cpu_gauge: ObservableGauge<f64>,
    _memory_gauge: ObservableGauge<u64>,
    _virtual_memory_gauge: ObservableGauge<u64>,
}

/// Initialize system metrics (CPU, RAM) using OpenTelemetry gauges.
/// This should be called after the meter provider has been set globally.
/// Returns a SystemMetrics struct that must be kept alive for the application's lifetime.
pub fn init_system_metrics(name: &'static str, version: &'static str) -> SystemMetrics {
    let meter = opentelemetry::global::meter(name);
    let system = Arc::new(Mutex::new(System::new_all()));
    let current_pid = Pid::from_u32(std::process::id());

    // Generate standard tags for all gauge events
    let generate_tags = || {
        [
            KeyValue::new(SERVICE_NAME, name.to_owned()),
            KeyValue::new(SERVICE_VERSION, version.to_owned()),
        ]
    };

    // CPU Usage Gauge (percentage)
    let system_cpu = Arc::clone(&system);
    let cpu_gauge = meter
        .f64_observable_gauge("system.cpu.utilization")
        .with_description("CPU utilization of the Sake process")
        .with_unit("%")
        .with_callback(move |observer| {
            let mut sys = system_cpu.lock().unwrap();
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[current_pid]),
                false,
                ProcessRefreshKind::nothing().with_cpu(),
            );

            if let Some(process) = sys.process(current_pid) {
                let cpu_usage = process.cpu_usage();
                observer.observe(cpu_usage as f64, &generate_tags());
            }
        })
        .build();

    // Memory Usage Gauge (bytes)
    let system_mem = Arc::clone(&system);
    let memory_gauge = meter
        .u64_observable_gauge("system.memory.usage")
        .with_description("Memory usage of the Sake process in bytes")
        .with_unit("bytes")
        .with_callback(move |observer| {
            let mut sys = system_mem.lock().unwrap();
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[current_pid]),
                false,
                ProcessRefreshKind::nothing().with_memory(),
            );

            if let Some(process) = sys.process(current_pid) {
                let memory_bytes = process.memory();
                observer.observe(memory_bytes, &generate_tags());
            }
        })
        .build();

    // Virtual Memory Usage Gauge (bytes)
    let system_vmem = Arc::clone(&system);
    let virtual_memory_gauge = meter
        .u64_observable_gauge("system.memory.virtual")
        .with_description("Virtual memory usage of the Sake process in bytes")
        .with_unit("bytes")
        .with_callback(move |observer| {
            let mut sys = system_vmem.lock().unwrap();
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&[current_pid]),
                false,
                ProcessRefreshKind::nothing().with_memory(),
            );

            if let Some(process) = sys.process(current_pid) {
                let virtual_memory_bytes = process.virtual_memory();
                observer.observe(virtual_memory_bytes, &generate_tags());
            }
        })
        .build();

    SystemMetrics {
        _cpu_gauge: cpu_gauge,
        _memory_gauge: memory_gauge,
        _virtual_memory_gauge: virtual_memory_gauge,
    }
}
