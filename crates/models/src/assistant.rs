//! "Risolvi" assistant contracts: an AI model (Anthropic Claude, user's own API key)
//! proposes a fix plan for one diagnostic finding; the app executes only whitelisted,
//! natively implemented actions, one step at a time, each after explicit consent.
//!
//! Safety model:
//! - Nothing is sent until the user has seen the exact payload (`FixPreview`) and accepted.
//! - The model never runs anything. Its answer is data: steps whose `action` must be one
//!   of the `RemediationAction` variants below, re-validated locally against current state.
//!   Anything else becomes manual instructions.
//! - Each executable step runs only on its own explicit confirmation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::Confidence;

/// Model used when the user has not chosen one (the user's choice: Claude Sonnet 5.5).
pub const DEFAULT_ASSISTANT_MODEL: &str = "claude-sonnet-5-5";
/// Models offered in Settings, default first. Exact API aliases.
pub const ASSISTANT_MODELS: &[&str] = &[
    "claude-sonnet-5-5",
    "claude-opus-5-5",
    "claude-opus-4-5",
    "claude-haiku-5-5",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AssistantStatus {
    /// Settings switch (`AppSettings::assistant_enabled`).
    pub enabled: bool,
    /// An API key is stored in Windows Credential Manager. The key itself never leaves
    /// the backend.
    pub key_configured: bool,
    pub model: String,
    pub available_models: Vec<String>,
}

/// What would be sent, shown to the user before anything leaves the machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FixPreview {
    /// One-shot token: `request_fix` sends exactly this payload and nothing else.
    pub token: String,
    pub rule_id: String,
    pub finding_title: String,
    pub model: String,
    /// The exact user-message content that will be sent (pretty JSON, already redacted:
    /// no host name, user name or file paths).
    pub payload: String,
    #[ts(type = "number")]
    pub payload_bytes: u64,
    /// Where the data goes, in plain Italian.
    pub destination: String,
    /// Rough cost estimate in USD for one request at the model's list price.
    pub estimated_cost_usd: f64,
    /// Unix ms after which the token is no longer accepted.
    #[ts(type = "number")]
    pub expires_at: u64,
}

/// Windows Settings pages the assistant may open (fixed `ms-settings:` URIs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SettingsPage {
    WindowsUpdate,
    WindowsUpdateOptional,
    StorageSense,
    Display,
    Sound,
    Bluetooth,
    Network,
    Power,
    Troubleshoot,
    StartupApps,
    InstalledApps,
    Privacy,
}

/// The only things the app can do on the assistant's behalf. Every variant is
/// implemented natively (no shell, no arbitrary command) and validated before running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum RemediationAction {
    /// Opens a Windows Settings page. Changes nothing by itself.
    OpenSettingsPage { page: SettingsPage },
    /// Opens Device Manager. Changes nothing by itself.
    OpenDeviceManager,
    /// Starts Windows Disk Cleanup for a fixed drive (the user picks what to delete).
    OpenDiskCleanup { drive: String },
    /// Empties the Recycle Bin; Windows shows its own confirmation. Not reversible.
    EmptyRecycleBin,
    /// Deletes files older than `older_than_days` (min 1) from the user's temp folder,
    /// skipping files in use. Not reversible.
    CleanTempFiles { older_than_days: u32 },
    /// Asks a program to close (like clicking its X; unsaved work can be lost only if
    /// the program itself discards it). Never force-kills; never system, protected or
    /// other-user processes, never PC Diagnostics itself.
    CloseProgram { pid: u32, name: String },
    /// Runs Windows System File Checker (`sfc /scannow`) in a visible window. Needs
    /// administrator rights: Windows asks through its own UAC prompt.
    RunSystemFileCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FixStep {
    pub id: String,
    pub title: String,
    /// Plain-Italian explanation of what the step does and why.
    pub explanation: String,
    pub risk: StepRisk,
    pub requires_admin: bool,
    pub reversible: bool,
    /// Executable by the app (after validation). `None` = manual step.
    pub action: Option<RemediationAction>,
    /// What the user does by hand (always set for manual steps).
    pub manual_instructions: Option<String>,
    /// Set when the model proposed an action the app rejected or adjusted, and why.
    pub validation_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FixPlan {
    pub session_id: String,
    pub rule_id: String,
    pub model: String,
    /// Model that actually answered (from the API response).
    pub served_by: String,
    pub summary: String,
    /// The model's own confidence that the plan addresses the finding.
    pub confidence: Confidence,
    pub steps: Vec<FixStep>,
    /// Cautions from the model (e.g. "salva il lavoro prima").
    pub warnings: Vec<String>,
    pub usage: TokenUsage,
    #[ts(type = "number")]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StepOutcome {
    pub session_id: String,
    pub step_id: String,
    pub ok: bool,
    /// Plain-Italian result, e.g. "Liberati 1,2 GB" or "Il programma non si è chiuso".
    pub message: String,
    #[ts(type = "number")]
    pub at: u64,
}
