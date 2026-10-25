#![forbid(unsafe_code)]
//! Local HTTP API and static dashboard host for ContextDrop.

pub mod fixtures;

use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path as UrlPath, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD,
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HOST, ORIGIN, VARY,
};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use contextdrop_core::{
    compress, inspect_protection, protection_rule_catalog, run_benchmark, BenchmarkCase,
    CompressionConfig, Error as EngineError, InputFormat, Pricing, ProtectionRuleInfo, Strategy,
    TokenizerKind,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::fixtures::{FixtureDetail, FixtureSet};

pub const API_VERSION: u32 = 1;
pub const HARD_MAX_INPUT_BYTES: usize = 1_000_000;
pub const MAX_REQUEST_BODY_BYTES: usize = 4 * 1024 * 1024;
const WORK_TIMEOUT: Duration = Duration::from_secs(20);
const CONCURRENT_JOBS: usize = 4;

#[derive(Clone)]
pub struct AppState {
    fixtures: FixtureSet,
}

impl AppState {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            fixtures: FixtureSet::load()?,
        })
    }
}

pub fn build_app(state: AppState, web_dir: Option<PathBuf>) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/rules", get(protection_rules))
        .route("/fixtures", get(list_fixtures))
        .route("/fixtures/{id}", get(get_fixture))
        .route("/compress", post(compress_input))
        .route("/benchmark", post(benchmark_input))
        .route("/inspect", post(inspect_input))
        .fallback(api_not_found)
        .layer(middleware::from_fn(api_guard))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(ConcurrencyLimitLayer::new(CONCURRENT_JOBS));

    let mut app = Router::new()
        .nest("/api", api)
        .with_state(state)
        .layer(middleware::from_fn(security_headers));

    if let Some(web_dir) = web_dir {
        let index = web_dir.join("index.html");
        if index.is_file() {
            // Client-side routes need the SPA shell on a direct navigation or
            // refresh. Register each known route explicitly so a missing asset
            // still returns a truthful 404 instead of HTML with a 200 status.
            app = app
                .route_service("/", ServeFile::new(&index))
                .route_service("/workspace", ServeFile::new(&index))
                .route_service("/benchmark", ServeFile::new(&index))
                .route_service("/rules", ServeFile::new(&index))
                .route_service("/settings", ServeFile::new(&index))
                .fallback_service(ServeDir::new(&web_dir));
        } else {
            app = app.route("/", get(dashboard_not_built));
        }
    } else {
        app = app.route("/", get(dashboard_not_built));
    }

    app
}

async fn health() -> Response {
    Json(json!({
        "status": "ok",
        "api_version": API_VERSION,
        "engine_version": contextdrop_core::VERSION,
        "max_input_bytes": HARD_MAX_INPUT_BYTES,
    }))
    .into_response()
}

#[derive(Serialize)]
struct RuleCatalogResponse {
    rules: Vec<ProtectionRuleInfo>,
    user_patterns: &'static str,
    note: &'static str,
}

async fn protection_rules() -> Response {
    Json(RuleCatalogResponse {
        rules: protection_rule_catalog(),
        user_patterns:
            "Custom preserve patterns are literal, case-insensitive substrings. They are not regular expressions and are never executed.",
        note: "Protection is heuristic and rule-based. Adjacent context around protected evidence is retained. It does not prove semantic equivalence.",
    })
    .into_response()
}

async fn list_fixtures(State(state): State<AppState>) -> Response {
    let fixtures = state
        .fixtures
        .all()
        .iter()
        .map(|fixture| fixture.summary())
        .collect::<Vec<_>>();
    Json(fixtures).into_response()
}

async fn get_fixture(State(state): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    match state.fixtures.get(&id) {
        Some(fixture) => Json(FixtureDetail {
            fixture: fixture.summary(),
            content: fixture.content.to_string(),
        })
        .into_response(),
        None => api_error(
            StatusCode::NOT_FOUND,
            "fixture_not_found",
            "No embedded fixture has that identifier.",
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompressionRequest {
    text: String,
    #[serde(default)]
    strategy: Strategy,
    #[serde(default)]
    input_format: InputFormat,
    #[serde(default)]
    tokenizer: TokenizerKind,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    #[serde(default)]
    pricing: Option<Pricing>,
    #[serde(default = "default_true")]
    collapse_repetitions: bool,
    #[serde(default = "default_true")]
    collapse_passing_tests: bool,
    #[serde(default = "default_true")]
    collapse_progress_updates: bool,
    #[serde(default = "default_true")]
    structural_json: bool,
    #[serde(default = "default_max_input")]
    max_input_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkRequest {
    fixture_id: Option<String>,
    text: Option<String>,
    name: Option<String>,
    #[serde(default = "default_auto_format")]
    input_format: InputFormat,
    #[serde(default)]
    tokenizer: TokenizerKind,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    #[serde(default)]
    pricing: Option<Pricing>,
    #[serde(default = "default_true")]
    collapse_repetitions: bool,
    #[serde(default = "default_true")]
    collapse_passing_tests: bool,
    #[serde(default = "default_true")]
    collapse_progress_updates: bool,
    #[serde(default = "default_true")]
    structural_json: bool,
    #[serde(default = "default_max_input")]
    max_input_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectRequest {
    text: String,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    #[serde(default = "default_max_input")]
    max_input_bytes: usize,
}

fn default_true() -> bool {
    true
}

fn default_max_input() -> usize {
    HARD_MAX_INPUT_BYTES
}

fn default_auto_format() -> InputFormat {
    InputFormat::Auto
}

async fn compress_input(
    State(_state): State<AppState>,
    payload: Result<Json<CompressionRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return json_rejection(rejection),
    };
    let config = match make_config(
        request.strategy,
        request.input_format,
        request.tokenizer,
        request.preserve_patterns,
        request.pricing,
        request.collapse_repetitions,
        request.collapse_passing_tests,
        request.collapse_progress_updates,
        request.structural_json,
        request.max_input_bytes,
    ) {
        Ok(config) => config,
        Err(response) => return *response,
    };
    if request.text.len() > config.max_input_bytes {
        return api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "input_too_large",
            &format!(
                "Input is {} bytes; the configured limit is {} bytes.",
                request.text.len(),
                config.max_input_bytes
            ),
        );
    }

    let text = request.text;
    let task = tokio::task::spawn_blocking(move || compress(&text, &config));
    match tokio::time::timeout(WORK_TIMEOUT, task).await {
        Ok(Ok(Ok(result))) => Json(result).into_response(),
        Ok(Ok(Err(error))) => engine_error(error),
        Ok(Err(_)) => internal_worker_error(),
        Err(_) => api_error(
            StatusCode::GATEWAY_TIMEOUT,
            "processing_timeout",
            "Local processing exceeded the time limit.",
        ),
    }
}

async fn benchmark_input(
    State(state): State<AppState>,
    payload: Result<Json<BenchmarkRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return json_rejection(rejection),
    };
    let config = match make_config(
        Strategy::Balanced,
        request.input_format,
        request.tokenizer,
        request.preserve_patterns,
        request.pricing,
        request.collapse_repetitions,
        request.collapse_passing_tests,
        request.collapse_progress_updates,
        request.structural_json,
        request.max_input_bytes,
    ) {
        Ok(config) => config,
        Err(response) => return *response,
    };

    let (case, content_len) = match (&request.fixture_id, &request.text) {
        (Some(id), None) => match state.fixtures.get(id) {
            Some(fixture) => (
                BenchmarkCase::from_fixture(&fixture.spec, fixture.content.to_string()),
                fixture.content.len(),
            ),
            None => {
                return api_error(
                    StatusCode::NOT_FOUND,
                    "fixture_not_found",
                    "No embedded fixture has that identifier.",
                )
            }
        },
        (None, Some(text)) => {
            let text_len = text.len();
            (
                BenchmarkCase::user(
                    clean_name(request.name.as_deref()),
                    text.clone(),
                    request.input_format,
                ),
                text_len,
            )
        }
        _ => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "invalid_source",
                "Provide exactly one benchmark source: fixture_id or text.",
            )
        }
    };

    if content_len > config.max_input_bytes {
        return api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "input_too_large",
            &format!(
                "Benchmark input is {content_len} bytes; the configured limit is {} bytes.",
                config.max_input_bytes
            ),
        );
    }

    let task = tokio::task::spawn_blocking(move || run_benchmark(&case, &config));
    match tokio::time::timeout(WORK_TIMEOUT, task).await {
        Ok(Ok(Ok(result))) => Json(result).into_response(),
        Ok(Ok(Err(error))) => engine_error(error),
        Ok(Err(_)) => internal_worker_error(),
        Err(_) => api_error(
            StatusCode::GATEWAY_TIMEOUT,
            "processing_timeout",
            "Local benchmark exceeded the time limit.",
        ),
    }
}

async fn inspect_input(
    State(_state): State<AppState>,
    payload: Result<Json<InspectRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(payload) => payload,
        Err(rejection) => return json_rejection(rejection),
    };
    if request.max_input_bytes < 1 || request.max_input_bytes > HARD_MAX_INPUT_BYTES {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_limit",
            "max_input_bytes must be between 1 and 1,000,000.",
        );
    }
    if request.text.len() > request.max_input_bytes {
        return api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "input_too_large",
            &format!(
                "Input is {} bytes; the configured limit is {} bytes.",
                request.text.len(),
                request.max_input_bytes
            ),
        );
    }

    let config = CompressionConfig {
        preserve_patterns: request.preserve_patterns,
        max_input_bytes: request.max_input_bytes,
        ..CompressionConfig::default()
    };
    if let Err(error) = config.validate() {
        return api_error(
            StatusCode::BAD_REQUEST,
            "invalid_config",
            &error.to_string(),
        );
    }
    let text = request.text;
    let patterns = config.preserve_patterns;
    let task = tokio::task::spawn_blocking(move || inspect_protection(&text, &patterns, 200));
    match tokio::time::timeout(WORK_TIMEOUT, task).await {
        Ok(Ok(inspection)) => Json(inspection).into_response(),
        Ok(Err(_)) => internal_worker_error(),
        Err(_) => api_error(
            StatusCode::GATEWAY_TIMEOUT,
            "processing_timeout",
            "Local inspection exceeded the time limit.",
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn make_config(
    strategy: Strategy,
    input_format: InputFormat,
    tokenizer: TokenizerKind,
    preserve_patterns: Vec<String>,
    pricing: Option<Pricing>,
    collapse_repetitions: bool,
    collapse_passing_tests: bool,
    collapse_progress_updates: bool,
    structural_json: bool,
    max_input_bytes: usize,
) -> Result<CompressionConfig, Box<Response>> {
    if !(1..=HARD_MAX_INPUT_BYTES).contains(&max_input_bytes) {
        return Err(Box::new(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_limit",
            "max_input_bytes must be between 1 and 1,000,000.",
        )));
    }
    let config = CompressionConfig {
        strategy,
        input_format,
        preserve_patterns,
        tokenizer,
        pricing,
        collapse_repetitions,
        collapse_passing_tests,
        collapse_progress_updates,
        structural_json,
        max_input_bytes,
    };
    config.validate().map_err(|error| {
        Box::new(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_config",
            &error.to_string(),
        ))
    })?;
    Ok(config)
}

fn clean_name(name: Option<&str>) -> String {
    let cleaned = name
        .unwrap_or("Pasted input")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(120)
        .collect::<String>();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Pasted input".into()
    } else {
        trimmed.to_string()
    }
}

fn engine_error(error: EngineError) -> Response {
    match error {
        EngineError::InputTooLarge { actual, limit } => api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "input_too_large",
            &format!("Input is {actual} bytes; the configured limit is {limit} bytes."),
        ),
        EngineError::InvalidJson { .. } => api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_json",
            "The input was marked as JSON but could not be parsed.",
        ),
        EngineError::InvalidConfig(message) => {
            api_error(StatusCode::BAD_REQUEST, "invalid_config", &message)
        }
        EngineError::Classification(_) | EngineError::Internal(_) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "engine_failure",
            "The local processing engine could not complete the request.",
        ),
    }
}

fn json_rejection(rejection: JsonRejection) -> Response {
    let status = rejection.status();
    let (status, code, message) = if status == StatusCode::PAYLOAD_TOO_LARGE {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "Request body exceeds the 4 MiB HTTP limit.",
        )
    } else if status == StatusCode::UNSUPPORTED_MEDIA_TYPE {
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Requests must use Content-Type: application/json.",
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request JSON does not match the expected schema.",
        )
    };
    api_error(status, code, message)
}

fn internal_worker_error() -> Response {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "worker_failure",
        "The local processing worker stopped unexpectedly.",
    )
}

#[derive(Serialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Serialize)]
struct ApiErrorBody {
    code: String,
    message: String,
}

fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(ApiErrorEnvelope {
            error: ApiErrorBody {
                code: code.into(),
                message: message.into(),
            },
        }),
    )
        .into_response()
}

async fn api_not_found() -> Response {
    api_error(
        StatusCode::NOT_FOUND,
        "not_found",
        "No API endpoint exists at this path.",
    )
}

async fn dashboard_not_built() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Html(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>ContextDrop</title></head>\
<body style=\"font-family:system-ui;max-width:42rem;margin:4rem auto;line-height:1.6\">\
<h1>ContextDrop API is running</h1>\
<p>The dashboard has not been built. From the repository root run:</p>\
<pre>cd web\nnpm ci\nnpm run build</pre>\
<p>Then restart <code>contextdrop-server</code>, or run <code>npm run dev</code> in <code>web</code> for development.</p>\
<p>The local API is available at <code>/api/health</code>.</p></body></html>",
        ),
    )
        .into_response()
}

async fn security_headers(request: Request, next: Next) -> Response {
    let is_api = request.uri().path().starts_with("/api");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
    );
    if is_api {
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

async fn api_guard(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let headers = request.headers();

    if let Some(host) = headers.get(HOST) {
        let host = match host.to_str() {
            Ok(value) => value,
            Err(_) => {
                return api_error(
                    StatusCode::FORBIDDEN,
                    "invalid_host",
                    "The Host header is not valid.",
                )
            }
        };
        if !host_is_loopback(host) {
            return api_error(
                StatusCode::FORBIDDEN,
                "host_not_allowed",
                "ContextDrop only accepts requests addressed to a loopback host.",
            );
        }
    }

    let origin = match headers.get(ORIGIN) {
        Some(value) => match value.to_str() {
            Ok(origin) if is_loopback_origin(origin) => Some(origin.to_string()),
            _ => {
                return api_error(
                    StatusCode::FORBIDDEN,
                    "origin_not_allowed",
                    "Browser origins are restricted to loopback hosts.",
                )
            }
        },
        None => None,
    };

    if method == Method::OPTIONS {
        let Some(origin) = origin else {
            return api_error(
                StatusCode::METHOD_NOT_ALLOWED,
                "preflight_requires_origin",
                "CORS preflight requests require an allowed Origin header.",
            );
        };
        if let Some(requested) = headers.get(ACCESS_CONTROL_REQUEST_METHOD) {
            let requested = requested.to_str().unwrap_or_default();
            if !matches!(requested, "GET" | "POST") {
                let mut response = api_error(
                    StatusCode::METHOD_NOT_ALLOWED,
                    "method_not_allowed",
                    "Only GET and POST are allowed.",
                );
                add_cors_headers(response.headers_mut(), &origin);
                return response;
            }
        }
        if let Some(requested) = headers.get(ACCESS_CONTROL_REQUEST_HEADERS) {
            let requested = requested.to_str().unwrap_or_default();
            for header in requested.split(',') {
                let header = header.trim().to_ascii_lowercase();
                if !matches!(header.as_str(), "content-type" | "accept") {
                    let mut response = api_error(
                        StatusCode::FORBIDDEN,
                        "header_not_allowed",
                        "The requested CORS header is not allowed.",
                    );
                    add_cors_headers(response.headers_mut(), &origin);
                    return response;
                }
            }
        }
        let mut response = StatusCode::NO_CONTENT.into_response();
        add_cors_headers(response.headers_mut(), &origin);
        return response;
    }

    if !matches!(method, Method::GET | Method::POST | Method::HEAD) {
        let mut response = api_error(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "Only GET and POST are allowed.",
        );
        if let Some(origin) = &origin {
            add_cors_headers(response.headers_mut(), origin);
        }
        return response;
    }

    if method == Method::POST && !content_type_is_json(headers) {
        let mut response = api_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Requests must use Content-Type: application/json.",
        );
        if let Some(origin) = &origin {
            add_cors_headers(response.headers_mut(), origin);
        }
        return response;
    }

    if method == Method::POST {
        if let Some(length) = headers.get(CONTENT_LENGTH) {
            let parsed = length
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok());
            if parsed.is_some_and(|length| length > MAX_REQUEST_BODY_BYTES as u64) {
                let mut response = api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request_too_large",
                    "Request body exceeds the 4 MiB HTTP limit.",
                );
                if let Some(origin) = &origin {
                    add_cors_headers(response.headers_mut(), origin);
                }
                return response;
            }
        }
    }

    let mut response = next.run(request).await;
    if let Some(origin) = &origin {
        add_cors_headers(response.headers_mut(), origin);
    }
    response
}

fn add_cors_headers(headers: &mut HeaderMap, origin: &str) {
    if let Ok(value) = HeaderValue::from_str(origin) {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, Accept"),
    );
    headers.insert(ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("600"));
    headers.append(VARY, HeaderValue::from_static("Origin"));
}

fn content_type_is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

fn host_is_loopback(host: &str) -> bool {
    let hostname = if host.starts_with('[') {
        match host.find(']') {
            Some(end) => &host[..=end],
            None => return false,
        }
    } else {
        host.split(':').next().unwrap_or(host)
    };
    if hostname.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let ip_candidate = hostname.trim_start_matches('[').trim_end_matches(']');
    ip_candidate
        .parse::<IpAddr>()
        .map(|address| address.is_loopback())
        .unwrap_or(false)
}

fn is_loopback_origin(origin: &str) -> bool {
    let authority = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"));
    let Some(authority) = authority else {
        return false;
    };
    let authority = authority.split('/').next().unwrap_or(authority);
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let hostname = if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => &authority[..=end],
            None => return false,
        }
    } else {
        authority.split(':').next().unwrap_or(authority)
    };
    if hostname.eq_ignore_ascii_case("localhost") {
        return true;
    }
    hostname
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .map(|address| address.is_loopback())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use std::fs;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let state = AppState::load().expect("fixtures must load");
        build_app(state, None)
    }

    fn request(method: Method, uri: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(HOST, "127.0.0.1:8787")
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 20 * 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_reports_local_limits() {
        let response = test_app()
            .oneshot(
                request(Method::GET, "/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["api_version"], API_VERSION);
    }

    #[tokio::test]
    async fn client_routes_serve_shell_but_missing_assets_stay_not_found() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("assets")).unwrap();
        fs::write(
            dir.path().join("index.html"),
            "<!doctype html><title>ContextDrop test shell</title>",
        )
        .unwrap();
        fs::write(dir.path().join("assets/app.js"), "console.log('ok');").unwrap();
        let app = build_app(
            AppState::load().expect("fixtures must load"),
            Some(dir.path().to_path_buf()),
        );

        for route in ["/", "/workspace", "/benchmark", "/rules", "/settings"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, route).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "route {route}");
            assert!(response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/html")));
            let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            assert!(String::from_utf8_lossy(&body).contains("ContextDrop test shell"));
        }

        let response = app
            .clone()
            .oneshot(
                request(Method::GET, "/assets/app.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .oneshot(
                request(Method::GET, "/assets/missing.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn compress_returns_real_metrics_and_preserves_errors() {
        let payload = json!({
            "text": "same\nsame\nsame\nerror: stop now\n",
            "strategy": "balanced",
            "tokenizer": "estimate",
            "max_input_bytes": 100000
        });
        let response = test_app()
            .oneshot(
                request(Method::POST, "/api/compress")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(body["output"].as_str().unwrap().contains("error: stop now"));
        assert_eq!(
            body["metrics"]["original_bytes"],
            payload["text"].as_str().unwrap().len()
        );
        assert!(body["preservation"]["passed"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn non_loopback_origin_is_rejected() {
        let response = test_app()
            .oneshot(
                request(Method::GET, "/api/health")
                    .header(ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(response
            .headers()
            .get(ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[tokio::test]
    async fn loopback_origin_gets_cors_headers() {
        let response = test_app()
            .oneshot(
                request(Method::GET, "/api/health")
                    .header(ORIGIN, "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "http://localhost:5173"
        );
    }

    #[tokio::test]
    async fn preflight_is_handled_without_running_handler() {
        let response = test_app()
            .oneshot(
                request(Method::OPTIONS, "/api/compress")
                    .header(ORIGIN, "http://127.0.0.1:5173")
                    .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .header(ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "http://127.0.0.1:5173"
        );
    }

    #[tokio::test]
    async fn non_loopback_host_is_rejected_to_block_dns_rebinding() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/health")
                    .header(HOST, "evil.example:8787")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn post_requires_json_content_type() {
        let response = test_app()
            .oneshot(
                request(Method::POST, "/api/compress")
                    .header(CONTENT_TYPE, "text/plain")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn fixture_endpoints_expose_embedded_data() {
        let app = test_app();
        let response = app
            .clone()
            .oneshot(
                request(Method::GET, "/api/fixtures")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(body.as_array().unwrap().len() >= 10);

        let response = app
            .oneshot(
                request(Method::GET, "/api/fixtures/rust-build-failure")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(body["content"].as_str().unwrap().contains("error[E0308]"));
    }

    #[tokio::test]
    async fn benchmark_runs_all_strategies() {
        let payload = json!({
            "fixture_id": "rust-build-failure",
            "tokenizer": "estimate",
            "max_input_bytes": 100000
        });
        let response = test_app()
            .oneshot(
                request(Method::POST, "/api/benchmark")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["results"].as_array().unwrap().len(), 3);
        assert!(body["case"]["original_bytes"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn unknown_fields_are_rejected() {
        let payload = json!({"text": "hello", "surprise": true});
        let response = test_app()
            .oneshot(
                request(Method::POST, "/api/compress")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn origin_and_host_parsers_are_strict() {
        assert!(is_loopback_origin("http://localhost:5173"));
        assert!(is_loopback_origin("http://127.0.0.1:8787"));
        assert!(is_loopback_origin("http://[::1]:5173"));
        assert!(!is_loopback_origin("https://localhost.evil.example"));
        assert!(!is_loopback_origin("null"));
        assert!(host_is_loopback("127.0.0.1:8787"));
        assert!(host_is_loopback("localhost:8787"));
        assert!(!host_is_loopback("localhost.evil.example"));
    }
}
