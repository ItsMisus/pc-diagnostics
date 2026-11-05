//! Per-model request shape and list prices.
//!
//! The request builder and the cost estimate both read this table, so a model is either
//! fully supported (known shape + price) or rejected with `InvalidInput`.

use models::AppError;

/// Default model when settings name none (mirrors `models::assistant::DEFAULT_ASSISTANT_MODEL`).
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";

/// How the request for one model is shaped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelProfile {
    pub id: &'static str,
    /// List price, USD per 1M input tokens.
    pub input_usd_per_mtok: f64,
    /// List price, USD per 1M output tokens (thinking tokens are billed as output).
    pub output_usd_per_mtok: f64,
    /// `Some(budget)` = send `thinking: {type: "enabled", budget_tokens}` (pre-4.6 models).
    /// `None` = omit `thinking` (adaptive thinking is the model's default).
    pub thinking_budget: Option<u32>,
    /// `output_config.effort`.
    pub effort: &'static str,
    /// Send server-side refusal fallback (`fallbacks: "default"` + beta header).
    pub server_fallback: bool,
    /// Expected output tokens for one plan (visible answer + thinking), for the estimate.
    pub estimated_output_tokens: u32,
}

pub const MAX_TOKENS: u32 = 16_000;
/// Visible plan JSON, typical size.
const PLAN_OUTPUT_TOKENS: u32 = 2_500;
const OPUS_45_THINKING_BUDGET: u32 = 4_000;

pub const MODEL_PROFILES: &[ModelProfile] = &[
    ModelProfile {
        id: "claude-sonnet-5-5",
        input_usd_per_mtok: 2.0,
        output_usd_per_mtok: 10.0,
        thinking_budget: None,
        effort: "high",
        server_fallback: true,
        estimated_output_tokens: PLAN_OUTPUT_TOKENS,
    },
    ModelProfile {
        id: "claude-opus-5-5",
        input_usd_per_mtok: 4.0,
        output_usd_per_mtok: 20.0,
        thinking_budget: None,
        // Default on this model is medium: set it explicitly.
        effort: "high",
        server_fallback: true,
        estimated_output_tokens: PLAN_OUTPUT_TOKENS,
    },
    ModelProfile {
        id: "claude-opus-4-5",
        input_usd_per_mtok: 5.0,
        output_usd_per_mtok: 25.0,
        thinking_budget: Some(OPUS_45_THINKING_BUDGET),
        // Opus 4.5 supports low/medium/high only.
        effort: "high",
        server_fallback: false,
        estimated_output_tokens: PLAN_OUTPUT_TOKENS + OPUS_45_THINKING_BUDGET,
    },
    ModelProfile {
        id: "claude-haiku-5-5",
        // Prices for prompts up to 100K tokens; our prompts are a few thousand.
        input_usd_per_mtok: 0.10,
        output_usd_per_mtok: 0.50,
        thinking_budget: None,
        effort: "high",
        server_fallback: false,
        estimated_output_tokens: PLAN_OUTPUT_TOKENS,
    },
];

/// Model ids this crate can call, in Settings order.
pub fn supported_models() -> Vec<&'static str> {
    MODEL_PROFILES.iter().map(|p| p.id).collect()
}

/// The profile for `model`, or `InvalidInput` for an unknown model.
pub fn profile(model: &str) -> Result<&'static ModelProfile, AppError> {
    MODEL_PROFILES
        .iter()
        .find(|p| p.id == model)
        .ok_or_else(|| {
            AppError::invalid_input(format!("model not supported by the assistant: {model}"))
                .with_detail(format!(
                    "modelli supportati: {}",
                    supported_models().join(", ")
                ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::ErrorCode;

    #[test]
    fn known_models_resolve_and_unknown_is_invalid_input() {
        for m in supported_models() {
            let p = profile(m).expect("known");
            assert!(
                p.thinking_budget
                    .is_none_or(|b| (1024..MAX_TOKENS).contains(&b))
            );
            assert!(["low", "medium", "high"].contains(&p.effort));
        }
        assert!(supported_models().contains(&DEFAULT_MODEL));
        let err = profile("gpt-4").expect_err("unknown");
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[test]
    fn only_pre_46_model_uses_budget_tokens_and_haiku_has_no_fallback() {
        assert!(
            profile("claude-opus-4-5")
                .expect("p")
                .thinking_budget
                .is_some()
        );
        for m in ["claude-sonnet-5-5", "claude-opus-5-5", "claude-haiku-5-5"] {
            assert!(profile(m).expect("p").thinking_budget.is_none(), "{m}");
        }
        assert!(!profile("claude-haiku-5-5").expect("p").server_fallback);
        assert!(!profile("claude-opus-4-5").expect("p").server_fallback);
        assert!(profile("claude-sonnet-5-5").expect("p").server_fallback);
        assert!(profile("claude-opus-5-5").expect("p").server_fallback);
    }
}
