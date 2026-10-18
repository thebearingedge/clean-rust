use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc, time::Duration};

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use futures_util::future::join_all;
use serde::Serialize;

type CheckFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

#[derive(Clone)]
pub struct HealthCheck {
    name: &'static str,
    run: Arc<dyn Fn() -> CheckFuture + Send + Sync>,
}

impl HealthCheck {
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

pub fn router(checks: Vec<HealthCheck>) -> Router {
    let state = HealthState {
        checks: checks.into(),
        timeout: Duration::from_secs(2),
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
        let router = health::router(vec![health::HealthCheck::new("up", || async {
            Ok::<(), String>(())
        })]);
        let res = router.oneshot(get("/readyz")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_is_503_when_a_check_fails_but_livez_stays_ok() {
        let router = health::router(vec![health::HealthCheck::new("down", || async {
            Err::<(), _>("boom")
        })]);
        let res = router.clone().oneshot(get("/readyz")).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let res = router.oneshot(get("/livez")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
