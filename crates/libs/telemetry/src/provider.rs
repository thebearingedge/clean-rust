use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{Resource, propagation::TraceContextPropagator, trace::SdkTracerProvider};
use tracing_opentelemetry::layer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Log level used when `RUST_LOG` is unset or unparseable.
const DEFAULT_FILTER_LEVEL: &str = "info";

/// Initializes the global telemetry stack and returns the tracer provider.
///
/// Installs the global tracing subscriber (JSON formatting, OTLP span export,
/// [`EnvFilter`] driven by `RUST_LOG` with a [`DEFAULT_FILTER_LEVEL`] fallback)
/// and the global W3C `TraceContext` propagator, then returns the
/// [`SdkTracerProvider`] for the caller to shut down.
///
/// The caller owns the returned provider and **must call
/// [`SdkTracerProvider::shutdown`] on process exit** to flush in-flight span
/// batches: the installed subscriber holds a clone of the provider's tracer,
/// so dropping the provider alone is not sufficient.
///
/// # Errors
///
/// Configuration failures cannot be recovered here: an invalid [`SpanExporter`]
/// configuration is reported by panicking (see `# Panics`) so that startup
/// fails loudly. All fallible work happens before any global state is
/// installed, so a failure leaves the process unmodified and re-initializing
/// after fixing the configuration is safe.
///
/// # Panics
///
/// Panics if [`SpanExporter`] configuration is invalid.
///
/// Panics if called twice: installing the global subscriber is a one-time,
/// irreversible process-global side effect.
#[must_use]
pub fn init_tracer_provider(service_name: &'static str) -> SdkTracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("invalid otlp exporter configuration");

    global::set_text_map_propagator(TraceContextPropagator::new());

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(Resource::builder().with_service_name(service_name).build())
        .build();

    let tracer = provider.tracer(service_name);

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER_LEVEL)),
        )
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
