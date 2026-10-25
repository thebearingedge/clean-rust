use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc, time::Duration};

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use futures_util::future::join_all;
use serde::Serialize;

/// The result type of a readiness check: errors are erased to a `String`
/// message here so the generic router can log and report them without
/// knowing the underlying error types.
type CheckFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// A named readiness check (e.g. "database") wrapping an arbitrary async
/// fallible closure. Checks are only ever asked to succeed or fail; the
/// error message is surfaced via logs, not the response body.
#[derive(Clone)]
pub struct HealthCheck {
    name: &'static str,
    run: Arc<dyn Fn() -> CheckFuture + Send + Sync>,
}

impl HealthCheck {
    /// Creates a check from a closure producing a fallible future.
    pub fn new<F, Fut, E>(name: &'static str, check: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Display,
    {
        Self {
            name,
            run: Arc::new(move || {
                let fut = check();
                Box::pin(async move { fut.await.map_err(|e| e.to_string()) })
            }),
        }
    }

    /// Creates a check whose closure captures a cloned resource (e.g. a
    /// database pool), cloning it anew for each probe.
    pub fn with_resource<R, F, Fut, E>(name: &'static str, resource: R, check: F) -> Self
    where
        R: Clone + Send + Sync + 'static,
        F: Fn(R) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
        E: std::fmt::Display,
    {
        Self::new(name, move || check(resource.clone()))
    }
}

#[derive(Clone)]
struct HealthState {
    checks: Arc<[HealthCheck]>,
    timeout: Duration,
}

#[derive(Serialize)]
struct Readiness {
    status: &'static str,
    checks: BTreeMap<&'static str, &'static str>,
}

/// The health-probe router.
///
/// Exposes two routes:
///
/// - `/livez` — liveness; always returns `200 OK` with no body. If the
///   process can serve this, it is alive.
/// - `/readyz` — readiness; runs every [`HealthCheck`] concurrently, each
///   bounded by `timeout`. Returns `200 OK` with `{"status": "ok", ...}` when
///   every check passes, or `503` with `{"status": "unavailable", ...}` and a
///   per-check `ok`/`failed` map (timed-out checks report `"failed"`) when
///   any check fails.
pub fn router(checks: Vec<HealthCheck>, timeout: Duration) -> Router {
    let state = HealthState {
        checks: checks.into(),
        timeout,
    };
    Router::new()
        .route("/livez", get(|| async { StatusCode::OK }))
        .route("/readyz", get(readyz))
        .with_state(state)
}

async fn readyz(State(state): State<HealthState>) -> (StatusCode, Json<Readiness>) {
    let timeout = state.timeout;
    let outcomes = join_all(state.checks.iter().map(|check| async move {
        let result = match tokio::time::timeout(timeout, (check.run)()).await {
            Ok(result) => result,
            Err(_) => Err("timed out".to_owned()),
        };
        (check.name, result)
    }))
    .await;

    let mut checks = BTreeMap::new();
    for (name, result) in outcomes {
        if let Err(error) = &result {
            tracing::warn!(check = name, %error, "readiness check failed");
        }
        checks.insert(name, if result.is_ok() { "ok" } else { "failed" });
    }

    let ready = checks.values().all(|status| *status == "ok");
    let (code, status) = if ready {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    };
    (code, Json(Readiness { status, checks }))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{self as health};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn readyz_is_ok_when_every_check_passes() {
        let router = health::router(
            vec![health::HealthCheck::new("up", || async {
                Ok::<(), String>(())
            })],
            Duration::from_secs(2),
        );
        let res = router.oneshot(get("/readyz")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_is_503_when_a_check_fails_but_livez_stays_ok() {
        let router = health::router(
            vec![health::HealthCheck::new("down", || async {
                Err::<(), _>("boom")
            })],
            Duration::from_secs(2),
        );
        let res = router.clone().oneshot(get("/readyz")).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let res = router.oneshot(get("/livez")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
