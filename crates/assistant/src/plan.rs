//! Turns the model's JSON answer into a validated `FixPlan`.
//!
//! The model is never trusted: every step is re-checked here, every action goes through
//! the injected validator (the platform crate's real one in the app), and anything that
//! fails becomes a manual step with a `validation_note`. Safety flags are forced, never
//! lowered: `RunSystemFileCheck` needs admin; `EmptyRecycleBin`/`CleanTempFiles` are not
//! reversible; those and `CloseProgram` are at least `Medium` risk.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use models::AppError;
use models::assistant::{FixPlan, FixStep, RemediationAction, StepRisk, TokenUsage};
use models::diagnostics::Confidence;
use models::settings::Language;

pub const MAX_STEPS: usize = 8;
pub const MAX_WARNINGS: usize = 6;
const MAX_SUMMARY: usize = 1_200;
const MAX_TITLE: usize = 120;
const MAX_EXPLANATION: usize = 1_500;
const MAX_INSTRUCTIONS: usize = 3_000;
const MAX_WARNING: usize = 300;
const MAX_ID: usize = 32;
const MAX_PROGRAM_NAME: usize = 260;
const MAX_TEMP_DAYS: u32 = 3_650;

/// Validator signature: `Err(reason)` (plain language, in the given language) rejects the
/// action.
pub type ActionValidator<'a> =
    dyn Fn(&RemediationAction, Language) -> Result<(), String> + Sync + 'a;

/// Metadata that does not come from the model's JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanMeta {
    pub session_id: String,
    pub rule_id: String,
    pub model: String,
    pub served_by: String,
    pub usage: TokenUsage,
    pub created_at: u64,
    /// Language of the notes the app adds to the plan (the model writes the rest in the
    /// same language, as the system prompt asks).
    pub language: Language,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlan {
    summary: String,
    confidence: Confidence,
    #[serde(default)]
    steps: Vec<RawStep>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStep {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    explanation: String,
    #[serde(default = "default_risk")]
    risk: StepRisk,
    #[serde(default)]
    requires_admin: bool,
    #[serde(default)]
    reversible: bool,
    #[serde(default)]
    action: Option<Value>,
    #[serde(default)]
    manual_instructions: Option<String>,
}

fn default_risk() -> StepRisk {
    StepRisk::Medium
}

fn clip(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_owned();
    }
    let mut out: String = t.chars().take(max).collect();
    out.push('…');
    out
}

fn sanitize_id(id: &str) -> String {
    id.trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(MAX_ID)
        .collect()
}

/// Structural checks that do not need the live system. Returns the (possibly
/// normalized) action and an optional note, or a rejection reason.
fn local_check(
    action: RemediationAction,
    lang: Language,
) -> Result<(RemediationAction, Option<String>), String> {
    match action {
        RemediationAction::OpenDiskCleanup { drive } => {
            let d = drive.trim().trim_end_matches(['\\', '/']);
            let b = d.as_bytes();
            if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
                Ok((
                    RemediationAction::OpenDiskCleanup {
                        drive: d.to_ascii_uppercase(),
                    },
                    None,
                ))
            } else {
                let shown = clip(&drive, 20);
                Err(lang.pick(
                    format!("unità non valida: «{shown}»"),
                    format!("invalid drive: “{shown}”"),
                ))
            }
        }
        RemediationAction::CleanTempFiles { older_than_days } => {
            if older_than_days == 0 {
                Ok((
                    RemediationAction::CleanTempFiles { older_than_days: 1 },
                    Some(
                        lang.pick(
                            "Età minima dei file portata a 1 giorno.",
                            "Minimum file age raised to 1 day.",
                        )
                        .into(),
                    ),
                ))
            } else if older_than_days > MAX_TEMP_DAYS {
                Ok((
                    RemediationAction::CleanTempFiles {
                        older_than_days: MAX_TEMP_DAYS,
                    },
                    Some(lang.pick(
                        format!("Età dei file limitata a {MAX_TEMP_DAYS} giorni."),
                        format!("File age capped at {MAX_TEMP_DAYS} days."),
                    )),
                ))
            } else {
                Ok((RemediationAction::CleanTempFiles { older_than_days }, None))
            }
        }
        RemediationAction::CloseProgram { pid, name } => {
            let name = name.trim();
            if pid == 0 || pid == 4 {
                Err(lang
                    .pick(
                        "processo di sistema: non può essere chiuso dall'app",
                        "system process: the app can't close it",
                    )
                    .into())
            } else if name.is_empty() || name.chars().count() > MAX_PROGRAM_NAME {
                Err(lang
                    .pick(
                        "nome del programma mancante o non valido",
                        "missing or invalid program name",
                    )
                    .into())
            } else {
                Ok((
                    RemediationAction::CloseProgram {
                        pid,
                        name: name.to_owned(),
                    },
                    None,
                ))
            }
        }
        other => Ok((other, None)),
    }
}

fn join_notes(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (a, b) => a.or(b),
    }
}

fn enforce_flags(step: &mut FixStep, lang: Language) -> Option<String> {
    let Some(action) = &step.action else {
        return None;
    };
    let mut changed = false;
    let min_risk = match action {
        RemediationAction::RunSystemFileCheck
        | RemediationAction::EmptyRecycleBin
        | RemediationAction::CleanTempFiles { .. }
        | RemediationAction::CloseProgram { .. } => StepRisk::Medium,
        _ => StepRisk::Low,
    };
    if step.risk < min_risk {
        step.risk = min_risk;
        changed = true;
    }
    if matches!(action, RemediationAction::RunSystemFileCheck) && !step.requires_admin {
        step.requires_admin = true;
        changed = true;
    }
    if matches!(
        action,
        RemediationAction::EmptyRecycleBin | RemediationAction::CleanTempFiles { .. }
    ) && step.reversible
    {
        step.reversible = false;
        changed = true;
    }
    changed.then(|| {
        lang.pick(
            "Livello di rischio, permessi o reversibilità corretti dall'app.",
            "Risk level, permissions or reversibility corrected by the app.",
        )
        .to_owned()
    })
}

fn to_manual(step: &mut FixStep, reason: &str, lang: Language) {
    step.action = None;
    step.validation_note = join_notes(
        step.validation_note.take(),
        Some(lang.pick(
            format!(
                "Azione proposta non eseguibile dall'app ({reason}): esegui il passaggio a mano."
            ),
            format!("The app can't run the proposed action ({reason}): do this step by hand."),
        )),
    );
}

fn build_step(
    raw: RawStep,
    index: usize,
    validator: &ActionValidator<'_>,
    lang: Language,
) -> FixStep {
    let title = clip(&raw.title, MAX_TITLE);
    let mut step = FixStep {
        id: sanitize_id(&raw.id),
        title: if title.is_empty() {
            let n = index + 1;
            lang.pick(format!("Passaggio {n}"), format!("Step {n}"))
        } else {
            title
        },
        explanation: clip(&raw.explanation, MAX_EXPLANATION),
        risk: raw.risk,
        requires_admin: raw.requires_admin,
        reversible: raw.reversible,
        action: None,
        manual_instructions: raw
            .manual_instructions
            .map(|s| clip(&s, MAX_INSTRUCTIONS))
            .filter(|s| !s.is_empty()),
        validation_note: None,
    };
    match raw.action {
        None | Some(Value::Null) => {}
        Some(v) => match serde_json::from_value::<RemediationAction>(v) {
            Err(_) => to_manual(
                &mut step,
                lang.pick(
                    "azione sconosciuta o malformata",
                    "unknown or malformed action",
                ),
                lang,
            ),
            Ok(action) => match local_check(action, lang) {
                Err(reason) => to_manual(&mut step, &reason, lang),
                Ok((action, note)) => match validator(&action, lang) {
                    Err(reason) => to_manual(&mut step, &clip(&reason, 300), lang),
                    Ok(()) => {
                        step.action = Some(action);
                        step.validation_note = note;
                    }
                },
            },
        },
    }
    let flag_note = enforce_flags(&mut step, lang);
    step.validation_note = join_notes(step.validation_note.take(), flag_note);
    if step.action.is_none() && step.manual_instructions.is_none() {
        step.manual_instructions = Some(if step.explanation.is_empty() {
            step.title.clone()
        } else {
            step.explanation.clone()
        });
    }
    step
}

/// Parses and validates the model's JSON text. Extra steps beyond `MAX_STEPS` are
/// dropped; ids are made unique (`step-N` replaces a missing, invalid or duplicate id).
pub fn build_plan(
    model_json: &str,
    meta: PlanMeta,
    validator: &ActionValidator<'_>,
) -> Result<FixPlan, AppError> {
    let raw: RawPlan = serde_json::from_str(model_json).map_err(|e| {
        AppError::internal("the model returned an invalid plan")
            .with_detail(format!("JSON does not match the schema: {e}"))
    })?;
    let mut seen = HashSet::new();
    let mut steps = Vec::new();
    for (i, raw_step) in raw.steps.into_iter().take(MAX_STEPS).enumerate() {
        let mut step = build_step(raw_step, i, validator, meta.language);
        if step.id.is_empty() || seen.contains(&step.id) {
            let mut n = i + 1;
            while seen.contains(&format!("step-{n}")) {
                n += 1;
            }
            step.id = format!("step-{n}");
        }
        seen.insert(step.id.clone());
        steps.push(step);
    }
    let warnings = raw
        .warnings
        .iter()
        .map(|w| clip(w, MAX_WARNING))
        .filter(|w| !w.is_empty())
        .take(MAX_WARNINGS)
        .collect();
    let summary = clip(&raw.summary, MAX_SUMMARY);
    Ok(FixPlan {
        session_id: meta.session_id,
        rule_id: meta.rule_id,
        model: meta.model,
        served_by: meta.served_by,
        summary,
        confidence: raw.confidence,
        steps,
        warnings,
        usage: meta.usage,
        created_at: meta.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta() -> PlanMeta {
        PlanMeta {
            session_id: "s".into(),
            rule_id: "r".into(),
            model: "claude-sonnet-5-5".into(),
            served_by: "claude-sonnet-5-5".into(),
            usage: TokenUsage::default(),
            created_at: 1,
            language: Language::Italian,
        }
    }

    fn accept(_: &RemediationAction, _: Language) -> Result<(), String> {
        Ok(())
    }

    fn step(id: &str, action: Value) -> Value {
        json!({
            "id": id, "title": "T", "explanation": "E", "risk": "low",
            "requiresAdmin": false, "reversible": true,
            "action": action, "manualInstructions": null
        })
    }

    fn plan(steps: Vec<Value>) -> String {
        json!({"summary": "S", "confidence": "medium", "steps": steps, "warnings": ["w"]})
            .to_string()
    }

    #[test]
    fn flags_are_forced_never_lowered() {
        let p = build_plan(
            &plan(vec![
                step("a", json!({"kind": "runSystemFileCheck"})),
                step("b", json!({"kind": "emptyRecycleBin"})),
                step("c", json!({"kind": "cleanTempFiles", "olderThanDays": 7})),
                step(
                    "d",
                    json!({"kind": "closeProgram", "pid": 1234, "name": "chrome.exe"}),
                ),
                step("e", json!({"kind": "openDeviceManager"})),
            ]),
            meta(),
            &accept,
        )
        .expect("plan");
        let s = &p.steps;
        assert!(s[0].requires_admin);
        assert_eq!(s[0].risk, StepRisk::Medium);
        assert!(!s[1].reversible);
        assert!(!s[2].reversible);
        assert_eq!(s[1].risk, StepRisk::Medium);
        assert_eq!(s[2].risk, StepRisk::Medium);
        assert_eq!(s[3].risk, StepRisk::Medium);
        assert!(
            s[0..4]
                .iter()
                .all(|x| x.validation_note.is_some() && x.action.is_some())
        );
        assert_eq!(s[4].risk, StepRisk::Low);
        assert!(s[4].validation_note.is_none());
    }

    #[test]
    fn high_risk_from_model_is_kept() {
        let mut v = step("a", json!({"kind": "emptyRecycleBin"}));
        v["risk"] = json!("high");
        let p = build_plan(&plan(vec![v]), meta(), &accept).expect("plan");
        assert_eq!(p.steps[0].risk, StepRisk::High);
    }

    #[test]
    fn rejected_and_unknown_actions_become_manual() {
        let reject = |a: &RemediationAction, _: Language| match a {
            RemediationAction::CloseProgram { .. } => Err("processo non trovato".to_string()),
            _ => Ok(()),
        };
        let p = build_plan(
            &plan(vec![
                step(
                    "a",
                    json!({"kind": "closeProgram", "pid": 99, "name": "x.exe"}),
                ),
                step("b", json!({"kind": "formatDisk", "drive": "C:"})),
                step(
                    "c",
                    json!({"kind": "openDiskCleanup", "drive": "C:\\Windows"}),
                ),
                step("d", json!(null)),
            ]),
            meta(),
            &reject,
        )
        .expect("plan");
        for s in &p.steps {
            assert!(s.action.is_none(), "{}", s.id);
            assert!(s.manual_instructions.is_some(), "{}", s.id);
        }
        assert!(
            p.steps[0]
                .validation_note
                .as_deref()
                .is_some_and(|n| n.contains("processo non trovato"))
        );
        assert!(p.steps[1].validation_note.is_some());
        assert!(p.steps[2].validation_note.is_some());
        assert!(p.steps[3].validation_note.is_none());
    }

    #[test]
    fn actions_are_normalized() {
        let p = build_plan(
            &plan(vec![
                step("a", json!({"kind": "openDiskCleanup", "drive": "c:\\"})),
                step("b", json!({"kind": "cleanTempFiles", "olderThanDays": 0})),
            ]),
            meta(),
            &accept,
        )
        .expect("plan");
        assert_eq!(
            p.steps[0].action,
            Some(RemediationAction::OpenDiskCleanup { drive: "C:".into() })
        );
        assert_eq!(
            p.steps[1].action,
            Some(RemediationAction::CleanTempFiles { older_than_days: 1 })
        );
    }

    #[test]
    fn steps_are_capped_ids_unique_and_strings_trimmed() {
        let mut steps: Vec<Value> = (0..12).map(|_| step("dup", json!(null))).collect();
        steps[1]["id"] = json!("step-3");
        steps[2]["title"] = json!("x".repeat(500));
        steps[3]["id"] = json!("!!!");
        let p = build_plan(&plan(steps), meta(), &accept).expect("plan");
        assert_eq!(p.steps.len(), MAX_STEPS);
        let ids: HashSet<_> = p.steps.iter().map(|s| s.id.clone()).collect();
        assert_eq!(
            ids.len(),
            MAX_STEPS,
            "{:?}",
            p.steps.iter().map(|s| &s.id).collect::<Vec<_>>()
        );
        assert!(p.steps[2].title.chars().count() <= MAX_TITLE + 1);
        assert_eq!(p.warnings, vec!["w".to_string()]);
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(build_plan("not json", meta(), &accept).is_err());
        assert!(build_plan(r#"{"summary": 1}"#, meta(), &accept).is_err());
    }

    #[test]
    fn app_notes_follow_the_plan_language() {
        let mut v = step("", json!({"kind": "emptyRecycleBin"}));
        v["title"] = json!("");
        let steps = vec![
            v,
            step("b", json!({"kind": "formatDisk"})),
            step("c", json!({"kind": "cleanTempFiles", "olderThanDays": 0})),
            step("d", json!({"kind": "openDiskCleanup", "drive": "D:\\x"})),
        ];
        let en = PlanMeta {
            language: Language::English,
            ..meta()
        };
        let p = build_plan(&plan(steps.clone()), en, &accept).expect("plan");
        let note = |i: usize| p.steps[i].validation_note.clone().unwrap_or_default();
        assert_eq!(p.steps[0].title, "Step 1");
        assert_eq!(
            note(0),
            "Risk level, permissions or reversibility corrected by the app."
        );
        assert_eq!(
            note(1),
            "The app can't run the proposed action (unknown or malformed action): do this \
             step by hand."
        );
        assert_eq!(
            note(2),
            "Minimum file age raised to 1 day. Risk level, permissions or reversibility \
             corrected by the app."
        );
        assert!(note(3).contains("(invalid drive: “D:\\x”)"));

        let p = build_plan(&plan(steps), meta(), &accept).expect("plan");
        assert_eq!(p.steps[0].title, "Passaggio 1");
        assert!(
            p.steps[1]
                .validation_note
                .as_deref()
                .is_some_and(|n| n.contains("esegui il passaggio a mano"))
        );
    }
}
