use opentelemetry::{KeyValue, trace::TracerProvider};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_opentelemetry::layer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// # Panics
///
/// Will panic if [`SpanExporter`] configuration is invalid.
#[must_use]
pub fn get_tracer_provider(service_name: &'static str) -> SdkTracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("invalid otlp exporter configuration");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_attribute(KeyValue::new("service.name", service_name))
                .build(),
        )
        .build();

    let tracer = provider.tracer(service_name);

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(layer().with_tracer(tracer))
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(false)
                .flatten_event(true),
        )
        .init();

    provider
}
