//! The mock LLM upstream: an HTTP server on localhost that replays the loaded scenario.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use parda_spec::scenario::{Api, Scenario};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::engine::{Engine, ScriptedReply, UpstreamLog};
use crate::wire::{api_for_path, mock_error_body, parse_request, response_body, sse_events};

type Shared = Arc<Mutex<Option<Engine>>>;

pub struct MockUpstream {
    addr: SocketAddr,
    engine: Shared,
    server: JoinHandle<()>,
}

impl MockUpstream {
    /// Binds an ephemeral localhost port and starts serving.
    ///
    /// # Errors
    /// Fails if no port can be bound.
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let engine: Shared = Arc::new(Mutex::new(None));
        let app = Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&engine));
        let server = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("mock upstream stopped: {e}");
            }
        });
        Ok(Self {
            addr,
            engine,
            server,
        })
    }

    /// Base URL without a path; both APIs are routed by path suffix.
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Replaces the active scenario, discarding anything recorded for the previous one.
    pub fn load(&self, scenario: Scenario) {
        *lock(&self.engine) = Some(Engine::new(scenario));
    }

    /// Ends the active scenario and returns what the upstream saw.
    #[must_use]
    pub fn finish(&self) -> Option<UpstreamLog> {
        lock(&self.engine).take().map(Engine::into_log)
    }
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn lock(engine: &Shared) -> std::sync::MutexGuard<'_, Option<Engine>> {
    engine.lock().unwrap_or_else(PoisonError::into_inner)
}

async fn handle(State(engine): State<Shared>, uri: Uri, body: Bytes) -> Response {
    let Some(api) = api_for_path(uri.path()) else {
        return (
            StatusCode::NOT_FOUND,
            format!("parda mock upstream: no API at {}", uri.path()),
        )
            .into_response();
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return json_error(
                api,
                StatusCode::BAD_REQUEST,
                &format!("body is not JSON: {e}"),
            );
        }
    };
    let (reply, request) = {
        let mut guard = lock(&engine);
        let Some(engine) = guard.as_mut() else {
            return json_error(api, StatusCode::SERVICE_UNAVAILABLE, "no scenario loaded");
        };
        engine.record_payload(payload.clone());
        match parse_request(api, &payload) {
            Ok(request) => (engine.reply(request.turn, &request.latest_input), request),
            Err(message) => return json_error(api, StatusCode::BAD_REQUEST, &message),
        }
    };
    match reply {
        ScriptedReply::HttpError { status, body } => match StatusCode::from_u16(status) {
            Ok(status) => {
                (status, [(header::CONTENT_TYPE, "application/json")], body).into_response()
            }
            Err(_) => json_error(
                api,
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("invalid scripted status {status}"),
            ),
        },
        ScriptedReply::MockError { message } => {
            json_error(api, StatusCode::INTERNAL_SERVER_ERROR, &message)
        }
        ok if request.stream => stream_response(api, &ok, &request.model),
        ok => match response_body(api, &ok, &request.model) {
            Ok(body) => axum::Json(body).into_response(),
            Err(message) => json_error(api, StatusCode::INTERNAL_SERVER_ERROR, &message),
        },
    }
}

fn stream_response(api: Api, reply: &ScriptedReply, model: &str) -> Response {
    match sse_events(api, reply, model) {
        Ok(events) => {
            let stream = futures_util::stream::iter(
                events
                    .into_iter()
                    .map(|e| Ok::<_, Infallible>(Bytes::from(e))),
            );
            (
                [
                    (header::CONTENT_TYPE, "text/event-stream"),
                    (header::CACHE_CONTROL, "no-cache"),
                ],
                Body::from_stream(stream),
            )
                .into_response()
        }
        Err(message) => json_error(api, StatusCode::INTERNAL_SERVER_ERROR, &message),
    }
}

fn json_error(api: Api, status: StatusCode, message: &str) -> Response {
    (status, axum::Json(mock_error_body(api, message))).into_response()
}
