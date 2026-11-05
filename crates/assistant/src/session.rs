//! In-memory state between commands: one-shot previews and plans being executed.
//!
//! - A preview token is valid for `PREVIEW_TTL_MS` and can be taken exactly once; the
//!   request then sends exactly the stored payload (never re-built).
//! - A plan's steps can each be started once; only steps with a validated action are
//!   executable. The caller still re-validates the action against the live system right
//!   before running it (state may have changed since the plan was made).

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};

use models::assistant::{FixPlan, FixPreview, FixStep, RemediationAction, StepOutcome};
use models::settings::Language;
use models::{AppError, ErrorCode};

use crate::cost::estimate_cost_usd;

pub const PREVIEW_TTL_MS: u64 = 10 * 60 * 1_000;
/// Plans older than this are dropped.
pub const PLAN_TTL_MS: u64 = 2 * 60 * 60 * 1_000;
pub const MAX_PLANS: usize = 16;
const MAX_PREVIEWS: usize = 16;

/// Where the data goes, shown in the preview.
pub fn destination(lang: Language) -> &'static str {
    lang.pick(
        "Anthropic (api.anthropic.com), il fornitore del modello Claude, tramite la tua chiave API. I dati sono trattati secondo i termini del tuo account Anthropic.",
        "Anthropic (api.anthropic.com), the provider of the Claude model, through your API key. The data is handled under the terms of your Anthropic account.",
    )
}

/// A preview waiting for consent: exactly what `request_fix` will send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRequest {
    pub token: String,
    pub rule_id: String,
    pub finding_title: String,
    pub model: String,
    pub payload: String,
    /// Language the plan is asked in (the preview's language).
    pub language: Language,
    pub expires_at: u64,
}

struct PlanEntry {
    plan: FixPlan,
    started: HashSet<String>,
    outcomes: Vec<StepOutcome>,
    stored_at: u64,
}

#[derive(Default)]
struct Inner {
    previews: HashMap<String, PendingRequest>,
    plans: HashMap<String, PlanEntry>,
}

/// Thread-safe store; keep one in Tauri managed state.
#[derive(Default)]
pub struct SessionStore {
    inner: Mutex<Inner>,
}

pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn not_found(msg: &str) -> AppError {
    AppError::new(ErrorCode::NotFound, msg)
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding the lock cannot leave the maps inconsistent.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Stores a one-shot preview and returns what the UI shows (texts in `lang`).
    pub fn create_preview(
        &self,
        rule_id: &str,
        finding_title: &str,
        model: &str,
        payload: String,
        lang: Language,
        now_ms: u64,
    ) -> Result<FixPreview, AppError> {
        let estimated_cost_usd = estimate_cost_usd(model, payload.len())?;
        let pending = PendingRequest {
            token: new_token(),
            rule_id: rule_id.to_owned(),
            finding_title: finding_title.to_owned(),
            model: model.to_owned(),
            payload,
            language: lang,
            expires_at: now_ms + PREVIEW_TTL_MS,
        };
        let preview = FixPreview {
            token: pending.token.clone(),
            rule_id: pending.rule_id.clone(),
            finding_title: pending.finding_title.clone(),
            model: pending.model.clone(),
            payload: pending.payload.clone(),
            payload_bytes: pending.payload.len() as u64,
            destination: destination(lang).to_owned(),
            estimated_cost_usd,
            expires_at: pending.expires_at,
        };
        let mut inner = self.lock();
        inner.previews.retain(|_, p| p.expires_at > now_ms);
        while inner.previews.len() >= MAX_PREVIEWS {
            let oldest = inner
                .previews
                .values()
                .min_by_key(|p| p.expires_at)
                .map(|p| p.token.clone());
            match oldest {
                Some(t) => inner.previews.remove(&t),
                None => break,
            };
        }
        inner.previews.insert(pending.token.clone(), pending);
        Ok(preview)
    }

    /// Consumes the preview. Unknown, already used or expired tokens are refused.
    pub fn take_preview(&self, token: &str, now_ms: u64) -> Result<PendingRequest, AppError> {
        let mut inner = self.lock();
        let p = inner
            .previews
            .remove(token)
            .ok_or_else(|| not_found("preview not found or already used: prepare a new one"))?;
        if p.expires_at <= now_ms {
            return Err(AppError::invalid_input(
                "preview expired: prepare a new one",
            ));
        }
        Ok(p)
    }

    pub fn store_plan(&self, plan: FixPlan, now_ms: u64) {
        let mut inner = self.lock();
        inner
            .plans
            .retain(|_, e| e.stored_at + PLAN_TTL_MS > now_ms);
        while inner.plans.len() >= MAX_PLANS {
            let oldest = inner
                .plans
                .iter()
                .min_by_key(|(_, e)| e.stored_at)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => inner.plans.remove(&k),
                None => break,
            };
        }
        inner.plans.insert(
            plan.session_id.clone(),
            PlanEntry {
                plan,
                started: HashSet::new(),
                outcomes: Vec::new(),
                stored_at: now_ms,
            },
        );
    }

    pub fn plan(&self, session_id: &str) -> Option<FixPlan> {
        self.lock().plans.get(session_id).map(|e| e.plan.clone())
    }

    /// Marks the step as started and returns it with its action. Refused for unknown
    /// sessions/steps, manual steps, and steps already started (even if they failed).
    pub fn begin_step(
        &self,
        session_id: &str,
        step_id: &str,
    ) -> Result<(FixStep, RemediationAction), AppError> {
        let mut inner = self.lock();
        let entry = inner
            .plans
            .get_mut(session_id)
            .ok_or_else(|| not_found("plan not found or expired"))?;
        let step = entry
            .plan
            .steps
            .iter()
            .find(|s| s.id == step_id)
            .cloned()
            .ok_or_else(|| not_found("step not found in the plan"))?;
        let Some(action) = step.action.clone() else {
            return Err(AppError::invalid_input(
                "this is a manual step: the app cannot run it",
            ));
        };
        if !entry.started.insert(step_id.to_owned()) {
            return Err(AppError::invalid_input("step already run"));
        }
        Ok((step, action))
    }

    pub fn record_outcome(&self, outcome: StepOutcome) {
        if let Some(e) = self.lock().plans.get_mut(&outcome.session_id) {
            e.outcomes.push(outcome);
        }
    }

    pub fn outcomes(&self, session_id: &str) -> Vec<StepOutcome> {
        self.lock()
            .plans
            .get(session_id)
            .map(|e| e.outcomes.clone())
            .unwrap_or_default()
    }

    /// Ids of steps already started, in no particular order.
    pub fn started_steps(&self, session_id: &str) -> Vec<String> {
        self.lock()
            .plans
            .get(session_id)
            .map(|e| e.started.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn discard(&self, session_id: &str) {
        self.lock().plans.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::assistant::{StepRisk, TokenUsage};
    use models::diagnostics::Confidence;

    const NOW: u64 = 1_000_000;
    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    fn plan(session: &str) -> FixPlan {
        let step = |id: &str, action: Option<RemediationAction>| FixStep {
            id: id.into(),
            title: "t".into(),
            explanation: "e".into(),
            risk: StepRisk::Low,
            requires_admin: false,
            reversible: true,
            action,
            manual_instructions: None,
            validation_note: None,
        };
        FixPlan {
            session_id: session.into(),
            rule_id: "r".into(),
            model: "claude-sonnet-5-5".into(),
            served_by: "claude-sonnet-5-5".into(),
            summary: "s".into(),
            confidence: Confidence::Medium,
            steps: vec![
                step("a", Some(RemediationAction::OpenDeviceManager)),
                step("m", None),
            ],
            warnings: vec![],
            usage: TokenUsage::default(),
            created_at: NOW,
        }
    }

    #[test]
    fn preview_is_one_shot() {
        let s = SessionStore::new();
        let p = s
            .create_preview("r", "t", "claude-sonnet-5-5", "{}".into(), IT, NOW)
            .expect("preview");
        assert_eq!(p.payload_bytes, 2);
        assert!(p.destination.contains("il fornitore del modello Claude"));
        assert_eq!(p.expires_at, NOW + PREVIEW_TTL_MS);
        assert!(p.estimated_cost_usd > 0.0);
        let taken = s.take_preview(&p.token, NOW + 1).expect("first take");
        assert_eq!(taken.payload, "{}");
        assert_eq!(taken.language, IT);
        let err = s.take_preview(&p.token, NOW + 2).expect_err("second take");
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn preview_expires_after_ten_minutes() {
        let s = SessionStore::new();
        let p = s
            .create_preview("r", "t", "claude-opus-4-5", "{}".into(), EN, NOW)
            .expect("preview");
        assert!(p.destination.contains("the provider of the Claude model"));
        let err = s
            .take_preview(&p.token, NOW + PREVIEW_TTL_MS)
            .expect_err("expired");
        assert_eq!(err.code, ErrorCode::InvalidInput);
        // Expired and consumed.
        assert!(s.take_preview(&p.token, NOW).is_err());
    }

    #[test]
    fn unknown_model_preview_is_rejected() {
        let s = SessionStore::new();
        let err = s
            .create_preview("r", "t", "gpt", "{}".into(), IT, NOW)
            .expect_err("model");
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn steps_run_once_and_manual_steps_never() {
        let s = SessionStore::new();
        s.store_plan(plan("s1"), NOW);
        let (step, action) = s.begin_step("s1", "a").expect("first");
        assert_eq!(step.id, "a");
        assert_eq!(action, RemediationAction::OpenDeviceManager);
        assert!(s.begin_step("s1", "a").is_err(), "twice");
        assert!(s.begin_step("s1", "m").is_err(), "manual");
        assert_eq!(
            s.begin_step("s1", "zz").expect_err("unknown").code,
            ErrorCode::NotFound
        );
        assert_eq!(
            s.begin_step("nope", "a").expect_err("unknown").code,
            ErrorCode::NotFound
        );
        assert_eq!(s.started_steps("s1"), vec!["a".to_string()]);
        s.record_outcome(StepOutcome {
            session_id: "s1".into(),
            step_id: "a".into(),
            ok: true,
            message: "ok".into(),
            at: NOW,
        });
        assert_eq!(s.outcomes("s1").len(), 1);
        s.discard("s1");
        assert!(s.plan("s1").is_none());
    }

    #[test]
    fn old_plans_are_evicted() {
        let s = SessionStore::new();
        s.store_plan(plan("old"), NOW);
        s.store_plan(plan("new"), NOW + PLAN_TTL_MS + 1);
        assert!(s.plan("old").is_none());
        assert!(s.plan("new").is_some());
        for i in 0..(MAX_PLANS + 3) {
            s.store_plan(plan(&format!("p{i}")), NOW + PLAN_TTL_MS + 2 + i as u64);
        }
        assert!(s.plan("new").is_none());
        assert!(s.plan(&format!("p{}", MAX_PLANS + 2)).is_some());
    }
}
