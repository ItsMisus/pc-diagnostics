//! Rough cost estimate for one request, shown in `FixPreview` as an estimate.

use models::AppError;

use crate::profile::profile;
use crate::prompt::{SYSTEM_PROMPT_EN, SYSTEM_PROMPT_IT};

/// Rough bytes-per-token ratio for mixed Italian/English JSON.
const BYTES_PER_TOKEN: f64 = 3.5;

/// Estimated input tokens: system prompt (the longer language version) + output schema +
/// payload.
#[allow(clippy::cast_precision_loss)]
pub fn estimate_input_tokens(payload_bytes: usize) -> u32 {
    let schema_bytes = crate::schema::fix_plan_schema().to_string().len();
    let prompt_bytes = SYSTEM_PROMPT_IT.len().max(SYSTEM_PROMPT_EN.len());
    let total = (prompt_bytes + schema_bytes + payload_bytes) as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let tokens = (total / BYTES_PER_TOKEN).ceil() as u32;
    tokens
}

/// USD at list price, rounded to 4 decimals. An estimate: actual usage depends on the
/// answer length and on thinking.
pub fn estimate_cost_usd(model: &str, payload_bytes: usize) -> Result<f64, AppError> {
    let p = profile(model)?;
    let input = f64::from(estimate_input_tokens(payload_bytes));
    let output = f64::from(p.estimated_output_tokens);
    let usd = (input * p.input_usd_per_mtok + output * p.output_usd_per_mtok) / 1_000_000.0;
    Ok((usd * 10_000.0).round() / 10_000.0)
}

/// Actual cost of a finished request from reported usage (list price, no caching discount).
pub fn usage_cost_usd(model: &str, input_tokens: u32, output_tokens: u32) -> Result<f64, AppError> {
    let p = profile(model)?;
    Ok((f64::from(input_tokens) * p.input_usd_per_mtok
        + f64::from(output_tokens) * p.output_usd_per_mtok)
        / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_follow_price_table() {
        let bytes = 3_500;
        let input = f64::from(estimate_input_tokens(bytes));
        let expect = |i: f64, o: f64, out: f64| ((input * i + out * o) / 1e6 * 1e4).round() / 1e4;
        assert_eq!(
            estimate_cost_usd("claude-opus-4-5", bytes).expect("c"),
            expect(5.0, 25.0, 6_500.0)
        );
        assert_eq!(
            estimate_cost_usd("claude-opus-5-5", bytes).expect("c"),
            expect(4.0, 20.0, 2_500.0)
        );
        assert_eq!(
            estimate_cost_usd("claude-sonnet-5-5", bytes).expect("c"),
            expect(2.0, 10.0, 2_500.0)
        );
        assert_eq!(
            estimate_cost_usd("claude-haiku-5-5", bytes).expect("c"),
            expect(0.1, 0.5, 2_500.0)
        );
        assert!(estimate_cost_usd("nope", bytes).is_err());
        // Order of magnitude sanity: a plan with Opus 4.5 costs well under a dollar.
        assert!(estimate_cost_usd("claude-opus-4-5", bytes).expect("c") < 0.5);
        assert!(input > 1_000.0);
    }

    #[test]
    fn usage_cost() {
        let c = usage_cost_usd("claude-sonnet-5-5", 1_000_000, 100_000).expect("c");
        assert!((c - 3.0).abs() < 1e-9);
    }
}
