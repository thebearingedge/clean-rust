use opentelemetry::{KeyValue, trace::TracerProvider};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{Resource, trace::SdkTracerProvider};
use tracing_opentelemetry::layer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_telemetry() -> SdkTracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("OTLP exporter");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_attribute(KeyValue::new("service.name", "clean-rust"))
                .build(),
        )
        .build();

    let tracer = provider.tracer("clean-rust");

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
