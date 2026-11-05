//! End-to-end against a local mock of the Messages API (no network, no real key).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant::{ClaudeClient, PayloadInput, SessionStore, prepare_preview, request_fix};
use models::ErrorCode;
use models::assistant::{RemediationAction, StepRisk};
use models::component::ComponentKind;
use models::diagnostics::{Confidence, Finding, FindingKind, Severity, SimpleExplanation};
use models::settings::Language;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
struct Recorded {
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

struct Canned {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn canned(status: u16, body: Value) -> Canned {
    Canned {
        status,
        headers: vec![],
        body: body.to_string(),
    }
}

struct Mock {
    base_url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

async fn mock(responses: Vec<Canned>) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
    let rec = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let rec = rec.clone();
            let queue = queue.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let header_end = loop {
                    let n = sock.read(&mut chunk).await.expect("read");
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(p) = find(&buf, b"\r\n\r\n") {
                        break p + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
                let mut lines = head.split("\r\n");
                let request_line = lines.next().unwrap_or_default().to_string();
                let headers: HashMap<String, String> = lines
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                    .collect();
                let len: usize = headers
                    .get("content-length")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                while buf.len() < header_end + len {
                    let n = sock.read(&mut chunk).await.expect("read body");
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let body: Value =
                    serde_json::from_slice(&buf[header_end..header_end + len]).expect("json body");
                rec.lock().expect("lock").push(Recorded {
                    path: request_line
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_string(),
                    headers,
                    body,
                });
                let c = queue
                    .lock()
                    .expect("lock")
                    .pop_front()
                    .unwrap_or_else(|| canned(500, json!({"type":"error","error":{"type":"api_error","message":"no canned response"}})));
                let mut resp = format!(
                    "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                    c.status,
                    c.body.len()
                );
                for (k, v) in &c.headers {
                    resp.push_str(&format!("{k}: {v}\r\n"));
                }
                resp.push_str("\r\n");
                resp.push_str(&c.body);
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    Mock {
        base_url: format!("http://{addr}"),
        requests,
    }
}

fn finding() -> Finding {
    Finding {
        rule_id: "storage.volume.low_space".into(),
        title: "Spazio quasi esaurito su C:".into(),
        component: ComponentKind::Storage,
        severity: Severity::Critical,
        kind: FindingKind::Confirmed,
        confidence: Confidence::High,
        description: r"Restano 1.0 GiB su C:\ (cartella C:\Users\Mario\Downloads molto grande)."
            .into(),
        evidence: vec![],
        possible_causes: vec![],
        recommendation: "Liberare spazio".into(),
        limitations: vec![],
        simple: SimpleExplanation::default(),
    }
}

fn plan_text() -> String {
    json!({
        "summary": "Il disco C: è quasi pieno.",
        "confidence": "high",
        "steps": [
            {"id": "s1", "title": "Pulizia disco", "explanation": "Avvia Pulizia disco.",
             "risk": "low", "requiresAdmin": false, "reversible": false,
             "action": {"kind": "openDiskCleanup", "drive": "C:"}, "manualInstructions": null},
            {"id": "s2", "title": "Svuota cestino", "explanation": "Svuota il Cestino.",
             "risk": "low", "requiresAdmin": false, "reversible": true,
             "action": {"kind": "emptyRecycleBin"}, "manualInstructions": null},
            {"id": "s3", "title": "Chiudi programma", "explanation": "Chiude un programma.",
             "risk": "low", "requiresAdmin": false, "reversible": true,
             "action": {"kind": "closeProgram", "pid": 777, "name": "ghost.exe"}, "manualInstructions": null},
            {"id": "s4", "title": "Controlla Download", "explanation": "Sposta file grandi.",
             "risk": "low", "requiresAdmin": false, "reversible": true,
             "action": null, "manualInstructions": "Apri Esplora file e controlla Download."}
        ],
        "warnings": ["Controlla il Cestino prima di svuotarlo."]
    })
    .to_string()
}

fn ok_response(model: &str, text: &str, extra_usage: Value) -> Canned {
    let mut usage = json!({"input_tokens": 1200, "output_tokens": 800});
    if let (Some(u), Some(e)) = (usage.as_object_mut(), extra_usage.as_object()) {
        for (k, v) in e {
            u.insert(k.clone(), v.clone());
        }
    }
    canned(
        200,
        json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": model,
            "stop_reason": "end_turn", "stop_details": null,
            "content": [
                {"type": "thinking", "thinking": "", "signature": "sig"},
                {"type": "text", "text": text}
            ],
            "usage": usage
        }),
    )
}

fn validator(a: &RemediationAction, lang: Language) -> Result<(), String> {
    match a {
        RemediationAction::CloseProgram { .. } => Err(lang
            .pick("processo non in esecuzione", "process not running")
            .into()),
        _ => Ok(()),
    }
}

async fn run(
    mock: &Mock,
    model: &str,
) -> (Result<assistant::FixOutcome, models::AppError>, String) {
    run_in(mock, model, Language::Italian).await
}

async fn run_in(
    mock: &Mock,
    model: &str,
    language: Language,
) -> (Result<assistant::FixOutcome, models::AppError>, String) {
    let store = SessionStore::new();
    let f = finding();
    let users = vec!["giulia".to_string()];
    let input = PayloadInput {
        finding: &f,
        coverage: &[],
        system: None,
        inventory: None,
        processes: None,
        user_names: &users,
        language,
    };
    let preview =
        prepare_preview(&store, &input, model, models::time::now_unix_ms()).expect("preview");
    let client = ClaudeClient::with_base_url(&mock.base_url)
        .expect("client")
        .with_retry_delay(Duration::from_millis(10));
    let res = request_fix(
        &store,
        &client,
        "sk-ant-test-key",
        &preview.token,
        &validator,
    )
    .await;
    // One-shot: the same token cannot be sent again.
    let again = request_fix(
        &store,
        &client,
        "sk-ant-test-key",
        &preview.token,
        &validator,
    )
    .await;
    assert_eq!(again.expect_err("one-shot").code, ErrorCode::NotFound);
    (res, preview.payload)
}

fn assert_common(r: &Recorded, model: &str, payload: &str) {
    assert_eq!(r.path, "/v1/messages");
    assert_eq!(r.headers["x-api-key"], "sk-ant-test-key");
    assert_eq!(r.headers["anthropic-version"], "2026-10-21");
    assert!(r.headers["content-type"].starts_with("application/json"));
    let b = &r.body;
    assert_eq!(b["model"], model);
    assert_eq!(b["max_tokens"], 16000);
    for banned in ["temperature", "top_p", "top_k"] {
        assert!(b.get(banned).is_none(), "{banned}");
    }
    assert_eq!(b["output_config"]["effort"], "high");
    assert_eq!(b["output_config"]["format"]["type"], "json_schema");
    assert_eq!(
        b["output_config"]["format"]["schema"]["additionalProperties"],
        false
    );
    assert_eq!(b["system"][0]["type"], "text");
    assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(b["messages"][0]["role"], "user");
    // Exactly the previewed payload, already redacted.
    assert_eq!(b["messages"][0]["content"], payload);
    let sent = b["messages"][0]["content"].as_str().expect("content");
    assert!(
        !sent.contains("Mario") && !sent.contains("Downloads"),
        "{sent}"
    );
}

#[tokio::test]
async fn valid_plan_for_each_model_shape() {
    for model in [
        "claude-sonnet-5-5",
        "claude-opus-5-5",
        "claude-opus-4-5",
        "claude-haiku-5-5",
    ] {
        let fallback = matches!(model, "claude-sonnet-5-5" | "claude-opus-5-5");
        let served = if model == "claude-opus-5-5" {
            "claude-opus-4-8"
        } else {
            model
        };
        let extra = if model == "claude-opus-5-5" {
            json!({"iterations": [{"type": "message"}, {"type": "fallback_message"}]})
        } else {
            json!({})
        };
        let m = mock(vec![ok_response(served, &plan_text(), extra)]).await;
        let (res, payload) = run(&m, model).await;
        let out = res.expect("plan");
        let reqs = m.requests.lock().expect("lock").clone();
        assert_eq!(reqs.len(), 1, "{model}");
        let r = &reqs[0];
        assert_common(r, model, &payload);

        if model == "claude-opus-4-5" {
            assert_eq!(
                r.body["thinking"],
                json!({"type": "enabled", "budget_tokens": 4000})
            );
        } else {
            assert!(r.body.get("thinking").is_none(), "{model}");
        }
        if fallback {
            assert_eq!(r.body["fallbacks"], "default", "{model}");
            assert_eq!(
                r.headers["anthropic-beta"],
                "server-side-fallback-2026-07-01"
            );
        } else {
            assert!(r.body.get("fallbacks").is_none(), "{model}");
            assert!(!r.headers.contains_key("anthropic-beta"), "{model}");
        }

        let p = &out.plan;
        assert_eq!(p.model, model);
        assert_eq!(p.served_by, served);
        assert_eq!(out.fallback_used, model == "claude-opus-5-5");
        assert_eq!(out.bytes_sent, payload.len() as u64);
        assert_eq!(p.usage.input_tokens, 1200);
        assert_eq!(p.usage.output_tokens, 800);
        assert_eq!(p.steps.len(), 4);
        assert_eq!(
            p.steps[0].action,
            Some(RemediationAction::OpenDiskCleanup { drive: "C:".into() })
        );
        assert_eq!(p.steps[1].risk, StepRisk::Medium);
        assert!(!p.steps[1].reversible);
        assert!(p.steps[2].action.is_none(), "rejected by validator");
        assert!(
            p.steps[2]
                .validation_note
                .as_deref()
                .is_some_and(|n| n.contains("non in esecuzione"))
        );
        assert!(p.steps[3].action.is_none());
    }
}

#[tokio::test]
async fn refusal_is_reported() {
    let m = mock(vec![canned(
        200,
        json!({"model": "claude-sonnet-5-5", "stop_reason": "refusal",
               "stop_details": {"type": "refusal", "category": "cyber", "explanation": "Rifiutato."},
               "content": [], "usage": {"input_tokens": 10, "output_tokens": 0}}),
    )])
    .await;
    let (res, _) = run(&m, "claude-sonnet-5-5").await;
    let e = res.expect_err("refusal");
    assert_eq!(e.code, ErrorCode::NotSupported);
    assert!(e.detail.as_deref().is_some_and(|d| d.contains("cyber")));
    assert_eq!(m.requests.lock().expect("lock").len(), 1, "no retry");
}

#[tokio::test]
async fn unauthorized_is_permission_denied_without_retry() {
    let m = mock(vec![canned(
        401,
        json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
    )])
    .await;
    let (res, _) = run(&m, "claude-opus-4-5").await;
    let e = res.expect_err("401");
    assert_eq!(e.code, ErrorCode::PermissionDenied);
    assert_eq!(e.message, "invalid API key or missing permissions");
    assert!(!e.detail.unwrap_or_default().contains("sk-ant-test-key"));
    assert_eq!(m.requests.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn rate_limit_retries_once_then_fails() {
    let rl = || Canned {
        status: 429,
        headers: vec![("retry-after", "0".into())],
        body:
            json!({"type": "error", "error": {"type": "rate_limit_error", "message": "slow down"}})
                .to_string(),
    };
    let m = mock(vec![rl(), rl()]).await;
    let (res, _) = run(&m, "claude-haiku-5-5").await;
    let e = res.expect_err("429");
    assert_eq!(e.code, ErrorCode::Network);
    assert!(e.retriable);
    assert_eq!(e.message, "too many requests, try again shortly");
    assert!(
        e.detail
            .as_deref()
            .is_some_and(|d| d.contains("retry-after: 0"))
    );
    assert_eq!(
        m.requests.lock().expect("lock").len(),
        2,
        "exactly one retry"
    );
}

#[tokio::test]
async fn rate_limit_then_success() {
    let m = mock(vec![
        Canned {
            status: 529,
            headers: vec![],
            body:
                json!({"type": "error", "error": {"type": "overloaded_error", "message": "busy"}})
                    .to_string(),
        },
        ok_response("claude-sonnet-5-5", &plan_text(), json!({})),
    ])
    .await;
    let (res, _) = run(&m, "claude-sonnet-5-5").await;
    assert!(res.is_ok());
    assert_eq!(m.requests.lock().expect("lock").len(), 2);
}

#[tokio::test]
async fn long_retry_after_is_not_waited() {
    let m = mock(vec![Canned {
        status: 429,
        headers: vec![("retry-after", "120".into())],
        body: "{}".into(),
    }])
    .await;
    let (res, _) = run(&m, "claude-sonnet-5-5").await;
    assert_eq!(res.expect_err("429").code, ErrorCode::Network);
    assert_eq!(m.requests.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn max_tokens_is_incomplete() {
    let m = mock(vec![canned(
        200,
        json!({"model": "claude-opus-4-5", "stop_reason": "max_tokens", "stop_details": null,
               "content": [{"type": "text", "text": "{\"summary\": \"Il dis"}],
               "usage": {"input_tokens": 10, "output_tokens": 16000}}),
    )])
    .await;
    let (res, _) = run(&m, "claude-opus-4-5").await;
    let e = res.expect_err("max_tokens");
    assert!(e.message.contains("incomplete response"));
}

#[tokio::test]
async fn connection_refused_is_retriable_network_error() {
    // Bind then drop: nothing listens on this port.
    let addr = {
        let l = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        l.local_addr().expect("addr")
    };
    let m = Mock {
        base_url: format!("http://{addr}"),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let (res, _) = run(&m, "claude-sonnet-5-5").await;
    let e = res.expect_err("refused");
    assert_eq!(e.code, ErrorCode::Network);
    assert!(e.retriable);
}

#[tokio::test]
async fn plan_follows_the_preview_language() {
    let m = mock(vec![ok_response(
        "claude-sonnet-5-5",
        &plan_text(),
        json!({}),
    )])
    .await;
    let (res, payload) = run_in(&m, "claude-sonnet-5-5", Language::English).await;
    let plan = res.expect("plan").plan;
    assert!(
        payload.contains("Personal data replaced: <utente>"),
        "{payload}"
    );
    let reqs = m.requests.lock().expect("lock").clone();
    let system = reqs[0].body["system"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(system.contains("plain, friendly US English"));
    assert_eq!(
        plan.steps[2].validation_note.as_deref(),
        Some("The app can't run the proposed action (process not running): do this step by hand.")
    );

    let m = mock(vec![ok_response(
        "claude-sonnet-5-5",
        &plan_text(),
        json!({}),
    )])
    .await;
    let (res, payload) = run(&m, "claude-sonnet-5-5").await;
    res.expect("plan");
    assert!(
        payload.contains("Dati personali sostituiti: <utente>"),
        "{payload}"
    );
    let reqs = m.requests.lock().expect("lock").clone();
    let system = reqs[0].body["system"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(system.contains("plain, friendly Italian"));
}

#[test]
fn request_fix_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let store = SessionStore::new();
    let client = ClaudeClient::with_base_url("http://127.0.0.1:9").expect("client");
    let fut = request_fix(&store, &client, "k", "t", &validator);
    assert_send(&fut);
}
