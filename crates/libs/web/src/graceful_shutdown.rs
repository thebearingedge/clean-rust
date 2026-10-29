use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tokio::signal;

/// # Panics
///
/// Will panic if Ctrl+C or SIGTERM handler cannot be installed.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

/// Shared flag flipped when the process starts draining: set by
/// [`graceful_shutdown`] once a shutdown signal arrives, read by the
/// readiness endpoint to stop reporting ready.
#[derive(Clone, Default)]
pub struct Draining(Arc<AtomicBool>);

impl Draining {
    /// Whether a shutdown signal has been received.
    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Flips the flag so readiness probes start failing.
    pub fn begin(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Resolves once the process should stop serving: signal received,
/// readiness flipped, then `drain_delay` elapsed with the server still
/// handling requests.
pub async fn graceful_shutdown(draining: Draining, drain_delay: Duration) {
    shutdown_signal().await;
    draining.begin();
    tracing::info!(
        delay_secs = drain_delay.as_secs(),
        "shutdown signal received, draining"
    );
    tokio::time::sleep(drain_delay).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_draining() {
        assert!(!Draining::default().is_draining());
    }

    #[test]
    fn begin_flips_the_flag() {
        let draining = Draining::default();
        draining.begin();
        assert!(draining.is_draining());
    }

    #[test]
    fn clone_shares_the_flag() {
        let draining = Draining::default();
        let clone = draining.clone();
        clone.begin();
        assert!(draining.is_draining());
    }
}
