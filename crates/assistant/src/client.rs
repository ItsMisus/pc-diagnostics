//! Raw-HTTP client for the Claude Messages API (no official Rust SDK).
//!
//! One request per plan; at most one retry for 429 / 5xx / 529 / connection errors /
//! timeouts, so cost stays predictable. The API key and payload are never logged at info
//! level (only sizes, model, status and token counts).

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use models::assistant::TokenUsage;
use models::settings::Language;
use models::{AppError, ErrorCode};

use crate::profile::{MAX_TOKENS, profile};
use crate::prompt::system_prompt;
use crate::schema::fix_plan_schema;

pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
pub const ANTHROPIC_VERSION: &str = "2026-10-21";
/// Beta header for `fallbacks: "default"` (server-side refusal fallback).
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(2);
/// A `retry-after` longer than this is not waited for: the error is returned.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(10);

/// The model's answer, before plan validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReply {
    /// Concatenated `text` blocks (the plan JSON).
    pub text: String,
    /// `model` from the response: who actually answered (differs from the requested
    /// model when the server-side fallback ran).
    pub served_by: String,
    pub usage: TokenUsage,
    /// `usage.iterations` contained a `fallback_message` entry or the content had a
    /// `fallback` block.
    pub fallback_used: bool,
}

/// The request body for `model`; the plan's texts are asked in `lang`. Never contains
/// sampling parameters.
pub fn build_request_body(model: &str, payload: &str, lang: Language) -> Result<Value, AppError> {
    let p = profile(model)?;
    let mut body = json!({
        "model": p.id,
        "max_tokens": MAX_TOKENS,
        "system": [{
            "type": "text",
            "text": system_prompt(lang),
            "cache_control": { "type": "ephemeral" },
        }],
        "messages": [{ "role": "user", "content": payload }],
        "output_config": {
            "effort": p.effort,
            "format": { "type": "json_schema", "schema": fix_plan_schema() },
        },
    });
    if let Some(obj) = body.as_object_mut() {
        if let Some(budget) = p.thinking_budget {
            obj.insert(
                "thinking".into(),
                json!({ "type": "enabled", "budget_tokens": budget }),
            );
        }
        if p.server_fallback {
            obj.insert("fallbacks".into(), json!("default"));
        }
    }
    Ok(body)
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    stop_details: Option<StopDetails>,
    #[serde(default)]
    content: Vec<Block>,
    #[serde(default)]
    usage: Option<ApiUsage>,
}

#[derive(Deserialize)]
struct StopDetails {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    explanation: Option<String>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    iterations: Option<Vec<Iteration>>,
}

#[derive(Deserialize)]
struct Iteration {
    #[serde(rename = "type", default)]
    kind: Option<String>,
}

fn sat_u32(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

/// Parses a 200 response. `stop_reason` is checked before the content.
pub fn parse_response(body: &[u8], requested_model: &str) -> Result<ModelReply, AppError> {
    let r: ApiResponse = serde_json::from_slice(body).map_err(|e| {
        AppError::new(ErrorCode::Network, "unreadable response from the service")
            .with_detail(e.to_string())
    })?;
    match r.stop_reason.as_deref() {
        Some("refusal") => {
            let mut detail = Vec::new();
            if let Some(d) = &r.stop_details {
                if let Some(c) = d.category.as_deref().filter(|c| !c.is_empty()) {
                    detail.push(format!("category: {c}"));
                }
                if let Some(x) = d.explanation.as_deref().filter(|x| !x.is_empty()) {
                    detail.push(x.to_owned());
                }
            }
            let mut err = AppError::new(
                ErrorCode::NotSupported,
                "the model declined to propose a plan for this problem",
            );
            if !detail.is_empty() {
                err = err.with_detail(detail.join(" — "));
            }
            return Err(err);
        }
        Some("max_tokens") => {
            return Err(AppError::internal(
                "incomplete response: the model reached the length limit",
            )
            .with_detail(format!("max_tokens = {MAX_TOKENS}")));
        }
        Some("end_turn" | "stop_sequence") | None => {}
        Some(other) => {
            return Err(AppError::internal("unexpected response from the model")
                .with_detail(format!("stop_reason = {other}")));
        }
    }
    let text: String = r
        .content
        .iter()
        .filter(|b| b.kind == "text")
        .filter_map(|b| b.text.as_deref())
        .collect();
    if text.trim().is_empty() {
        return Err(AppError::internal("the model returned no plan"));
    }
    let fallback_block = r.content.iter().any(|b| b.kind == "fallback");
    let (usage, fallback_iter) = match &r.usage {
        Some(u) => (
            TokenUsage {
                // Billed input: uncached + cache writes + cache reads.
                input_tokens: sat_u32(
                    u.input_tokens
                        + u.cache_creation_input_tokens.unwrap_or(0)
                        + u.cache_read_input_tokens.unwrap_or(0),
                ),
                output_tokens: sat_u32(u.output_tokens),
            },
            u.iterations.as_ref().is_some_and(|it| {
                it.iter()
                    .any(|i| i.kind.as_deref() == Some("fallback_message"))
            }),
        ),
        None => (TokenUsage::default(), false),
    };
    Ok(ModelReply {
        text,
        served_by: r
            .model
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| requested_model.to_owned()),
        usage,
        fallback_used: fallback_block || fallback_iter,
    })
}

#[derive(Deserialize)]
struct ApiErrorBody {
    error: ApiErrorInner,
}

#[derive(Deserialize)]
struct ApiErrorInner {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    message: String,
}

fn api_error_detail(body: &[u8]) -> Option<String> {
    let e: ApiErrorBody = serde_json::from_slice(body).ok()?;
    let msg = e.error.message.chars().take(500).collect::<String>();
    Some(if e.error.kind.is_empty() {
        msg
    } else {
        format!("{}: {msg}", e.error.kind)
    })
}

/// Maps a non-2xx response to an `AppError` (`detail` = API error).
pub fn map_http_error(status: u16, retry_after: Option<&str>, body: &[u8]) -> AppError {
    let api = api_error_detail(body);
    let with = |err: AppError, extra: Option<String>| {
        let parts: Vec<String> = [Some(format!("HTTP {status}")), api.clone(), extra]
            .into_iter()
            .flatten()
            .collect();
        err.with_detail(parts.join(" — "))
    };
    match status {
        401 | 403 => with(
            AppError::new(
                ErrorCode::PermissionDenied,
                "invalid API key or missing permissions",
            ),
            None,
        ),
        400 => with(
            AppError::invalid_input(match &api {
                Some(m) => format!("request rejected by the service: {m}"),
                None => "request rejected by the service".to_owned(),
            }),
            None,
        ),
        404 => with(
            AppError::new(ErrorCode::NotFound, "model not available for this API key"),
            None,
        ),
        429 => with(
            AppError::new(ErrorCode::Network, "too many requests, try again shortly"),
            retry_after.map(|r| format!("retry-after: {r} s")),
        ),
        s if s >= 500 => with(
            AppError::new(
                ErrorCode::Network,
                "the Anthropic service is unavailable right now, try again later",
            ),
            None,
        ),
        _ => with(
            AppError::new(ErrorCode::Network, "unexpected response from the service"),
            None,
        ),
    }
}

fn is_retriable_status(status: u16) -> bool {
    status == 429 || status >= 500
}

fn map_transport_error(e: &reqwest::Error) -> AppError {
    let msg = if e.is_timeout() {
        "the service did not respond in time (timeout)"
    } else if e.is_connect() {
        "could not reach the service: check the internet connection"
    } else {
        "network error during the request"
    };
    // reqwest's Display names the URL, never headers: no key leak.
    AppError::new(ErrorCode::Network, msg).with_detail(e.to_string())
}

/// Seconds from a `retry-after` header (integer form only).
fn parse_retry_after(v: Option<&str>) -> Option<Duration> {
    v.and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn check_api_key(key: &str) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::ConsentRequired,
            "missing API key: enter it in Settings",
        ));
    }
    if key.chars().any(|c| c.is_control() || c.is_whitespace()) || !key.is_ascii() {
        return Err(AppError::invalid_input(
            "invalid API key (characters not allowed)",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ClaudeClient {
    http: reqwest::Client,
    base_url: String,
    retry_delay: Duration,
    max_retry_delay: Duration,
}

impl ClaudeClient {
    /// Client for the real API.
    pub fn new() -> Result<Self, AppError> {
        Self::with_base_url(ANTHROPIC_BASE_URL)
    }

    /// Client for another base URL (tests: a local mock server).
    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(concat!("pc-diagnostics/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| {
                AppError::internal("HTTP client initialization failed").with_detail(e.to_string())
            })?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            retry_delay: DEFAULT_RETRY_DELAY,
            max_retry_delay: MAX_RETRY_DELAY,
        })
    }

    /// Delay before the single retry when the server gives no `retry-after`.
    #[must_use]
    pub fn with_retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay = delay;
        self
    }

    /// Sends one plan request (plus at most one retry on transient failures). The plan's
    /// texts are asked in `lang`.
    pub async fn create_plan_message(
        &self,
        api_key: &str,
        model: &str,
        payload: &str,
        lang: Language,
    ) -> Result<ModelReply, AppError> {
        check_api_key(api_key)?;
        let p = profile(model)?;
        let body = serde_json::to_vec(&build_request_body(model, payload, lang)?).map_err(|e| {
            AppError::internal("request serialization failed").with_detail(e.to_string())
        })?;
        let url = format!("{}/v1/messages", self.base_url);
        let mut attempt = 0u8;
        loop {
            attempt += 1;
            let mut req = self
                .http
                .post(&url)
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION);
            if p.server_fallback {
                req = req.header("anthropic-beta", FALLBACK_BETA);
            }
            tracing::debug!(model, bytes = body.len(), attempt, "assistant request");
            let result = req.body(body.clone()).send().await;
            let (err, wait) = match result {
                Err(e) => (map_transport_error(&e), None),
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned);
                    let bytes = resp.bytes().await.map_err(|e| map_transport_error(&e))?;
                    if (200..300).contains(&status) {
                        let reply = parse_response(&bytes, model)?;
                        tracing::info!(
                            model,
                            served_by = %reply.served_by,
                            fallback = reply.fallback_used,
                            input_tokens = reply.usage.input_tokens,
                            output_tokens = reply.usage.output_tokens,
                            "assistant plan received"
                        );
                        return Ok(reply);
                    }
                    tracing::info!(model, status, attempt, "assistant request failed");
                    let err = map_http_error(status, retry_after.as_deref(), &bytes);
                    if !is_retriable_status(status) {
                        return Err(err);
                    }
                    (err, parse_retry_after(retry_after.as_deref()))
                }
            };
            if attempt >= 2 {
                return Err(err);
            }
            let delay = wait.unwrap_or(self.retry_delay);
            if delay > self.max_retry_delay {
                return Err(err);
            }
            tokio::time::sleep(delay).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_mapping() {
        let body = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let e = map_http_error(401, None, body);
        assert_eq!(e.code, ErrorCode::PermissionDenied);
        assert!(!e.retriable);
        assert!(
            e.detail
                .as_deref()
                .is_some_and(|d| d.contains("invalid x-api-key"))
        );
        assert_eq!(
            map_http_error(403, None, b"").code,
            ErrorCode::PermissionDenied
        );

        let body =
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"bad schema"}}"#;
        let e = map_http_error(400, None, body);
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert!(e.message.contains("bad schema"));

        assert_eq!(map_http_error(404, None, b"x").code, ErrorCode::NotFound);

        let e = map_http_error(429, Some("30"), b"");
        assert_eq!(e.code, ErrorCode::Network);
        assert!(e.retriable);
        assert_eq!(e.message, "too many requests, try again shortly");
        assert!(
            e.detail
                .as_deref()
                .is_some_and(|d| d.contains("retry-after: 30"))
        );

        for s in [500, 502, 529] {
            let e = map_http_error(s, None, b"<html>");
            assert_eq!(e.code, ErrorCode::Network);
            assert!(e.retriable);
        }
    }

    #[test]
    fn refusal_and_max_tokens_are_errors() {
        let r = br#"{"model":"m","stop_reason":"refusal","stop_details":{"type":"refusal","category":null,"explanation":null},"content":[]}"#;
        let e = parse_response(r, "m").expect_err("refusal");
        assert_eq!(e.code, ErrorCode::NotSupported);
        assert!(e.detail.is_none());
        let r = br#"{"model":"m","stop_reason":"refusal","stop_details":{"category":"cyber","explanation":"no"},"content":[]}"#;
        let e = parse_response(r, "m").expect_err("refusal");
        assert!(e.detail.as_deref().is_some_and(|d| d.contains("cyber")));
        let r =
            br#"{"model":"m","stop_reason":"max_tokens","content":[{"type":"text","text":"{"}]}"#;
        let e = parse_response(r, "m").expect_err("max");
        assert!(e.message.contains("incomplete response"));
    }

    #[test]
    fn text_blocks_are_joined_and_thinking_ignored() {
        let r = br#"{"model":"claude-opus-4-8","stop_reason":"end_turn",
            "content":[{"type":"thinking","thinking":"hmm","signature":"x"},{"type":"fallback","from":{"model":"a"},"to":{"model":"b"}},{"type":"text","text":"{\"a\":"},{"type":"text","text":"1}"}],
            "usage":{"input_tokens":10,"output_tokens":20,"cache_read_input_tokens":5,"iterations":[{"type":"message"},{"type":"fallback_message"}]}}"#;
        let reply = parse_response(r, "claude-opus-5-5").expect("ok");
        assert_eq!(reply.text, r#"{"a":1}"#);
        assert_eq!(reply.served_by, "claude-opus-4-8");
        assert_eq!(reply.usage.input_tokens, 15);
        assert_eq!(reply.usage.output_tokens, 20);
        assert!(reply.fallback_used);
    }

    #[test]
    fn request_body_shapes() {
        for m in crate::profile::supported_models() {
            let b = build_request_body(m, "{}", Language::Italian).expect("body");
            for banned in ["temperature", "top_p", "top_k"] {
                assert!(b.get(banned).is_none(), "{m}: {banned}");
            }
            assert_eq!(b["output_config"]["effort"], "high");
            assert_eq!(b["output_config"]["format"]["type"], "json_schema");
            assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
            assert_eq!(b["messages"][0]["content"], "{}");
        }
        let it = build_request_body("claude-opus-5-5", "{}", Language::Italian).expect("b");
        let en = build_request_body("claude-opus-5-5", "{}", Language::English).expect("b");
        let text = |b: &Value| {
            b["system"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        assert!(text(&it).contains("plain, friendly Italian"));
        assert!(text(&en).contains("plain, friendly US English"));
        assert_eq!(it["output_config"], en["output_config"]);
        let b = build_request_body("claude-opus-4-5", "{}", Language::English).expect("b");
        assert_eq!(b["thinking"]["type"], "enabled");
        assert_eq!(b["thinking"]["budget_tokens"], 4000);
        assert_eq!(b["max_tokens"], 16000);
        assert!(b.get("fallbacks").is_none());
        for m in ["claude-sonnet-5-5", "claude-opus-5-5"] {
            let b = build_request_body(m, "{}", Language::English).expect("b");
            assert!(b.get("thinking").is_none());
            assert_eq!(b["fallbacks"], "default");
        }
        let b = build_request_body("claude-haiku-5-5", "{}", Language::English).expect("b");
        assert!(b.get("thinking").is_none());
        assert!(b.get("fallbacks").is_none());
        assert_eq!(
            build_request_body("claude-x", "{}", Language::English)
                .expect_err("unknown")
                .code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn api_key_is_checked_before_sending() {
        assert_eq!(
            check_api_key("").expect_err("e").code,
            ErrorCode::ConsentRequired
        );
        assert!(check_api_key("sk-ant x").is_err());
        assert!(check_api_key("sk-ant-abc").is_ok());
    }
}
