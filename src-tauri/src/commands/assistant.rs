//! "Risolvi" assistant commands. See `docs/assistant.md` for the safety model.
//!
//! - The API key is write-only from the UI: it goes to Windows Credential Manager (the
//!   login Keychain on macOS) and is read back only here, right before a request.
//! - `preview_fix` builds the redacted payload; nothing leaves the machine.
//! - `request_fix` accepts only a preview token and sends exactly that payload.
//! - `execute_fix_step` runs one validated action, once.
//!
//! Every preview, request, plan, step and error is recorded in `assistant_events`
//! (no payload content, no key).

use assistant::{ClaudeClient, PayloadInput, Redactor};
use models::assistant::{
    ASSISTANT_MODELS, AssistantStatus, FixPlan, FixPreview, RemediationAction, StepOutcome,
};
use models::{AppError, ErrorCode};
use persistence::{AssistantEvent, AssistantEventKind};
use tauri::{AppHandle, Manager, State};

use super::blocking;
use crate::state::AppState;

fn status_from(state: &AppState) -> Result<AssistantStatus, AppError> {
    let settings = state.db()?.load_settings()?;
    Ok(AssistantStatus {
        enabled: settings.assistant_enabled,
        key_configured: platform::credentials::has_api_key(),
        model: settings.assistant_model,
        available_models: ASSISTANT_MODELS.iter().map(|m| (*m).to_owned()).collect(),
    })
}

fn require_enabled(state: &AppState) -> Result<String, AppError> {
    let settings = state.db()?.load_settings()?;
    if !settings.assistant_enabled {
        return Err(AppError::new(
            ErrorCode::ConsentRequired,
            "the assistant is turned off in Settings",
        ));
    }
    Ok(settings.assistant_model)
}

/// Audit log write. A failure is logged and never hides the command's own result.
fn audit(state: &AppState, event: AssistantEvent) {
    let result = state
        .db()
        .and_then(|db| db.insert_assistant_event(&event).map_err(AppError::from));
    if let Err(e) = result {
        tracing::warn!(error = %e, kind = ?event.kind, "assistant audit write failed");
    }
}

fn error_event(e: &AppError, session_id: Option<&str>, step_id: Option<&str>) -> AssistantEvent {
    let mut ev = AssistantEvent::new(AssistantEventKind::Error, models::time::now_unix_ms());
    ev.session_id = session_id.map(str::to_owned);
    ev.step_id = step_id.map(str::to_owned);
    ev.ok = Some(false);
    ev.message = Some(format!("{:?}: {}", e.code, e.message));
    ev
}

/// HTTP client, built on first use. Debug builds only: `PCDIAG_ASSISTANT_BASE_URL` points
/// it at a local mock server for UI testing. Release builds always use the real API.
fn client(state: &AppState) -> Result<ClaudeClient, AppError> {
    if let Some(c) = state.claude_client() {
        return Ok(c);
    }
    #[cfg(debug_assertions)]
    let built = match std::env::var("PCDIAG_ASSISTANT_BASE_URL") {
        Ok(url) if url.starts_with("http://127.0.0.1:") => {
            tracing::warn!("assistant: using local mock API (debug build)");
            ClaudeClient::with_base_url(url)?
        }
        _ => ClaudeClient::new()?,
    };
    #[cfg(not(debug_assertions))]
    let built = ClaudeClient::new()?;
    Ok(state.set_claude_client(built))
}

#[tauri::command]
pub async fn get_assistant_status(app: AppHandle) -> Result<AssistantStatus, AppError> {
    blocking(move || status_from(&app.state::<AppState>())).await
}

/// Stores the key in Windows Credential Manager or the macOS login Keychain (replacing any
/// previous one). The key is never returned, logged or written to the database.
#[tauri::command]
pub async fn set_assistant_api_key(
    app: AppHandle,
    key: String,
) -> Result<AssistantStatus, AppError> {
    blocking(move || {
        platform::credentials::store_api_key(&key)?;
        drop(key);
        status_from(&app.state::<AppState>())
    })
    .await
}

#[tauri::command]
pub async fn clear_assistant_api_key(app: AppHandle) -> Result<AssistantStatus, AppError> {
    blocking(move || {
        platform::credentials::delete_api_key()?;
        status_from(&app.state::<AppState>())
    })
    .await
}

/// Builds the exact, redacted payload for one finding of the latest diagnostic report.
/// Nothing is sent.
#[tauri::command]
pub async fn preview_fix(app: AppHandle, finding_index: u32) -> Result<FixPreview, AppError> {
    blocking(move || {
        let state = app.state::<AppState>();
        let model = require_enabled(&state)?;
        if !platform::credentials::has_api_key() {
            return Err(AppError::new(
                ErrorCode::ConsentRequired,
                "no API key is configured for the assistant",
            ));
        }
        let report = state
            .db()?
            .latest_diagnostic_report()?
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "no diagnostic report yet"))?;
        let finding = report.findings.get(finding_index as usize).ok_or_else(|| {
            AppError::new(
                ErrorCode::NotFound,
                "finding not found in the latest report",
            )
            .with_detail(format!("index {finding_index}"))
        })?;
        let system = platform::system::system_info();
        let inventory = state.monitor()?.inventory(false).ok();
        let processes = state.processes()?.snapshot().ok();
        let user_names = Redactor::current_user_names();
        let input = PayloadInput {
            finding,
            coverage: &report.coverage,
            system: Some(&system),
            inventory: inventory.as_ref(),
            processes: processes.as_ref(),
            user_names: &user_names,
            language: state.language(),
        };
        let now = models::time::now_unix_ms();
        let preview = assistant::prepare_preview(state.assistant_sessions(), &input, &model, now)?;

        let mut ev = AssistantEvent::new(AssistantEventKind::Preview, now);
        ev.rule_id = Some(preview.rule_id.clone());
        ev.model = Some(preview.model.clone());
        ev.bytes_sent = Some(preview.payload_bytes);
        audit(&state, ev);
        Ok(preview)
    })
    .await
}

/// Sends the previewed payload (one-shot `token`) and returns the validated plan.
#[tauri::command]
pub async fn request_fix(state: State<'_, AppState>, token: String) -> Result<FixPlan, AppError> {
    let result = request_fix_inner(&state, &token).await;
    if let Err(e) = &result {
        audit(&state, error_event(e, None, None));
    }
    result
}

async fn request_fix_inner(state: &AppState, token: &str) -> Result<FixPlan, AppError> {
    require_enabled(state)?;
    let key = platform::credentials::load_api_key()?.ok_or_else(|| {
        AppError::new(
            ErrorCode::ConsentRequired,
            "no API key is configured for the assistant",
        )
    })?;
    let client = client(state)?;
    let validator = |a: &RemediationAction, lang| platform::remediation::validate(a, lang);
    let outcome =
        assistant::request_fix(state.assistant_sessions(), &client, &key, token, &validator)
            .await?;
    drop(key);

    let plan = outcome.plan;
    let mut req = AssistantEvent::new(AssistantEventKind::Request, plan.created_at);
    req.session_id = Some(plan.session_id.clone());
    req.rule_id = Some(plan.rule_id.clone());
    req.model = Some(plan.model.clone());
    req.bytes_sent = Some(outcome.bytes_sent);
    audit(state, req);

    let mut ev = AssistantEvent::new(AssistantEventKind::Plan, plan.created_at);
    ev.session_id = Some(plan.session_id.clone());
    ev.rule_id = Some(plan.rule_id.clone());
    ev.model = Some(plan.served_by.clone());
    ev.input_tokens = Some(plan.usage.input_tokens);
    ev.output_tokens = Some(plan.usage.output_tokens);
    ev.ok = Some(true);
    ev.message = Some(format!(
        "{} steps{}",
        plan.steps.len(),
        if outcome.fallback_used {
            ", answered by fallback model"
        } else {
            ""
        }
    ));
    audit(state, ev);
    Ok(plan)
}

/// Runs one step of a plan, once. The action is re-validated against the live system
/// right before it runs; a refusal is an `ok: false` outcome with the reason.
#[tauri::command]
pub async fn execute_fix_step(
    app: AppHandle,
    session_id: String,
    step_id: String,
) -> Result<StepOutcome, AppError> {
    blocking(move || {
        let state = app.state::<AppState>();
        let begun = state.assistant_sessions().begin_step(&session_id, &step_id);
        let (_step, action) = match begun {
            Ok(v) => v,
            Err(e) => {
                audit(&state, error_event(&e, Some(&session_id), Some(&step_id)));
                return Err(e);
            }
        };
        let lang = state.language();
        let (ok, message) = match platform::remediation::validate(&action, lang) {
            Err(reason) => (
                false,
                lang.pick(
                    format!("Non eseguito: {reason}"),
                    format!("Not run: {reason}"),
                ),
            ),
            Ok(()) => match platform::remediation::execute(&action, lang) {
                Ok(msg) => (true, msg),
                Err(failure) => {
                    tracing::info!(error = %failure.error, "assistant step failed");
                    (false, failure.message)
                }
            },
        };
        let outcome = StepOutcome {
            session_id: session_id.clone(),
            step_id: step_id.clone(),
            ok,
            message,
            at: models::time::now_unix_ms(),
        };
        state.assistant_sessions().record_outcome(outcome.clone());

        let mut ev = AssistantEvent::new(AssistantEventKind::Step, outcome.at);
        ev.session_id = Some(session_id);
        ev.step_id = Some(step_id);
        ev.ok = Some(ok);
        ev.message = Some(outcome.message.clone());
        audit(&state, ev);
        Ok(outcome)
    })
    .await
}
