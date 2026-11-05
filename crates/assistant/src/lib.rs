//! "Risolvi" assistant: builds a redacted description of one diagnostic finding, asks
//! Anthropic's Claude (user's own API key) for a fix plan, validates the answer and keeps
//! per-session state so the user can run allowed steps one at a time.
//!
//! Flow for the shell (src-tauri):
//! 1. `prepare_preview` → `FixPreview` (nothing leaves the machine yet), in the user's
//!    language (`PayloadInput::language`), which the plan then follows.
//! 2. On consent, `request_fix(token)` → takes the one-shot preview, sends exactly that
//!    payload, validates the plan with the platform validator, stores it.
//! 3. For each step the user confirms: `SessionStore::begin_step` → re-validate the action
//!    against the live system → run it natively → `SessionStore::record_outcome`.
//!
//! Testable without network: the HTTP base URL is injectable and the action validator is
//! a plain closure.

#![forbid(unsafe_code)]

pub mod client;
pub mod cost;
pub mod payload;
pub mod plan;
pub mod profile;
pub mod prompt;
pub mod redact;
pub mod schema;
pub mod session;

use models::AppError;
use models::assistant::{FixPlan, FixPreview};

pub use client::{ClaudeClient, ModelReply};
pub use payload::{PayloadInput, build_payload};
pub use plan::{ActionValidator, PlanMeta, build_plan};
pub use profile::{DEFAULT_MODEL, supported_models};
pub use redact::Redactor;
pub use session::{PendingRequest, SessionStore};

/// Builds the redacted payload for `input.finding` and stores a one-shot preview. The
/// preview, and later the plan, are in `input.language`.
pub fn prepare_preview(
    store: &SessionStore,
    input: &PayloadInput,
    model: &str,
    now_ms: u64,
) -> Result<FixPreview, AppError> {
    profile::profile(model)?;
    let payload = build_payload(input)?;
    store.create_preview(
        &input.finding.rule_id,
        &input.finding.title,
        model,
        payload,
        input.language,
        now_ms,
    )
}

/// Result of `request_fix`, with what the caller needs for the audit log.
#[derive(Debug, Clone, PartialEq)]
pub struct FixOutcome {
    pub plan: FixPlan,
    pub bytes_sent: u64,
    pub fallback_used: bool,
}

/// Consumes the preview `token` (refused if unknown, used or expired), sends exactly the
/// previewed payload with the previewed model, validates the answer with `validator` and
/// stores the plan under a new `session_id`. The plan is asked, and its app notes and the
/// validator's reasons are written, in the preview's language. The future is `Send`
/// (Tauri async command).
pub async fn request_fix(
    store: &SessionStore,
    client: &ClaudeClient,
    api_key: &str,
    token: &str,
    validator: &ActionValidator<'_>,
) -> Result<FixOutcome, AppError> {
    let pending = store.take_preview(token, models::time::now_unix_ms())?;
    let reply = client
        .create_plan_message(api_key, &pending.model, &pending.payload, pending.language)
        .await?;
    let plan = build_plan(
        &reply.text,
        PlanMeta {
            session_id: session::new_token(),
            rule_id: pending.rule_id.clone(),
            model: pending.model.clone(),
            served_by: reply.served_by.clone(),
            usage: reply.usage,
            created_at: models::time::now_unix_ms(),
            language: pending.language,
        },
        validator,
    )?;
    store.store_plan(plan.clone(), plan.created_at);
    Ok(FixOutcome {
        plan,
        bytes_sent: pending.payload.len() as u64,
        fallback_used: reply.fallback_used,
    })
}
