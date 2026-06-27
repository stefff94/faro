use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

use crate::model::{HookEvent, SessionState};
use crate::source::{ClaudeCodeSource, Source};
use crate::store::SessionStore;

pub type SharedStore = Arc<Mutex<SessionStore>>;
pub type OnChange = Arc<dyn Fn(Vec<SessionState>) + Send + Sync>;

#[derive(Clone)]
struct AppState {
    store: SharedStore,
    on_change: OnChange,
}

pub fn router(store: SharedStore, on_change: OnChange) -> Router {
    let state = AppState { store, on_change };
    Router::new()
        .route("/event", post(post_event))
        .route("/sessions", get(get_sessions))
        .with_state(state)
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

async fn post_event(State(state): State<AppState>, body: String) -> impl IntoResponse {
    match serde_json::from_str::<HookEvent>(&body) {
        Ok(event) => {
            // §11.b(6): log raw Notification payloads until the discriminator key is pinned.
            if event.hook_event_name == "Notification" {
                eprintln!("[faro] Notification payload: {body}");
            }
            let source = ClaudeCodeSource;
            let changed = {
                let mut store = state.store.lock().unwrap();
                store.apply(source.name(), &event, now_ms())
            };
            if changed {
                let snap = state.store.lock().unwrap().snapshot();
                (state.on_change)(snap);
            }
        }
        Err(e) => eprintln!("[faro] bad event body ({e}): {body}"),
    }
    // Always 200 — the reporter must never see an error.
    StatusCode::OK
}

async fn get_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let snap = state.store.lock().unwrap().snapshot();
    Json(snap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    fn test_router() -> (axum::Router, SharedStore) {
        let store: SharedStore = Arc::new(Mutex::new(crate::store::SessionStore::new()));
        let noop: OnChange = Arc::new(|_snap| {});
        (router(store.clone(), noop), store)
    }

    #[tokio::test]
    async fn post_event_applies_to_store() {
        let (app, store) = test_router();
        let body = r#"{"hook_event_name":"UserPromptSubmit","session_id":"abc","cwd":"/x/proj"}"#;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/event")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let snap = store.lock().unwrap().snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, "claude-code:abc");
    }

    #[tokio::test]
    async fn malformed_body_still_returns_200() {
        let (app, _store) = test_router();
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/event")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_sessions_returns_snapshot() {
        let (app, store) = test_router();
        store.lock().unwrap().apply(
            "claude-code",
            &crate::model::HookEvent {
                hook_event_name: "Stop".into(),
                session_id: "z".into(),
                cwd: Some("/x/proj".into()),
                transcript_path: None,
                notification_type: None,
                type_field: None,
            },
            1000,
        );
        let res = app
            .oneshot(Request::builder().uri("/sessions").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("\"status\":\"done\""));
    }
}
