//! Telemetry bootstrap: initialization of the global tracing subscriber and
//! OpenTelemetry span exporter, plus trace-context propagation helpers.

/// Trace-context propagation between HTTP headers and [`tracing`] spans.
pub mod propagation;

/// Initialization of the global tracing/OTel stack.
pub mod provider;
