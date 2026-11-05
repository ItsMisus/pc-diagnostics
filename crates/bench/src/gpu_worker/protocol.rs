//! Private, versioned parent ↔ worker protocol.
//!
//! - **Worker → parent** (stdout): one JSON object per line, `{"v":1,"type":...,...}`.
//!   Lines that do not start with `{` are ignored (e.g. a test harness banner), so the
//!   parent never depends on stdout being otherwise silent. A JSON line with another
//!   protocol version, or one that does not parse, is a protocol error.
//! - **Parent → worker** (stdin): the line `cancel`, or EOF (parent closed stdin or died).
//!   Both mean "stop now".
//!
//! Bump [`PROTOCOL_VERSION`] on any incompatible change. Parent and worker are always the
//! same executable, so a mismatch means a corrupted install or a foreign process.

use models::ErrorCode;
use serde::{Deserialize, Serialize};

pub(crate) const PROTOCOL_VERSION: u32 = 1;

/// `Msg::Progress::phase` keys. The worker sends a key, the parent words it in the UI
/// language.
pub(crate) mod phase {
    pub(crate) const SELECT: &str = "select";
    pub(crate) const COMPILE: &str = "compile";
    pub(crate) const WARMUP: &str = "warmup";
    pub(crate) const MEASURE: &str = "measure";
    pub(crate) const VERIFY: &str = "verify";
}

/// Parent → worker stop request (one line on stdin).
pub(crate) const CANCEL_LINE: &str = "cancel";

/// The adapter a wgpu backend exposes, as reported by the worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdapterReport {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    /// `dx12` or `vulkan`.
    pub backend: String,
    /// `discrete`, `integrated`, `virtual`, `cpu` or `other` (wgpu `DeviceType`).
    pub device_type: String,
    /// `luid:<high>-<low>` (same format as `GpuInfo::id`). DX12 only.
    pub luid: Option<String>,
    pub driver: String,
    pub driver_info: String,
}

/// How the worker matched the requested GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MatchKind {
    /// DX12 adapter with the requested LUID.
    Luid,
    /// No LUID match: the single non-software, non-virtual adapter with the requested PCI
    /// vendor and device id (Vulkan fallback).
    PciIds,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Point {
    /// Since the start of the timed window.
    pub elapsed_ms: u64,
    pub flops_per_s: f64,
}

/// Final numbers of a completed workload. Statistics are derived by the parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkerResult {
    pub flops_per_invocation: u64,
    pub invocations_per_dispatch: u32,
    pub dispatches_per_batch: u32,
    /// Batches and FLOPs in the timed window (warm-up excluded).
    pub batches: u64,
    pub total_flops: f64,
    pub timed_ms: u64,
    pub warmup_ms: u64,
    /// Per-second throughput over the timed window.
    pub points: Vec<Point>,
    /// Output elements compared with the CPU reference (all matched, or the run fails).
    pub verified_elements: u32,
    pub max_rel_error: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Msg {
    Hello {
        pid: u32,
    },
    /// Answer to `--probe`: every hardware adapter wgpu sees (DX12 and Vulkan).
    Adapters {
        adapters: Vec<AdapterReport>,
    },
    Selected {
        adapter: AdapterReport,
        matched_by: MatchKind,
    },
    /// The warm-up ended; the timed window starts now.
    Measuring,
    Progress {
        /// One of the [`phase`] keys.
        phase: String,
        /// 0–1 over warm-up + timed window.
        fraction: f64,
        /// Throughput of the last completed second, if any.
        flops_per_s: Option<f64>,
    },
    Done {
        result: WorkerResult,
    },
    Error {
        code: ErrorCode,
        message: String,
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Envelope {
    v: u32,
    #[serde(flatten)]
    msg: Msg,
}

/// One protocol line (no trailing newline).
pub(crate) fn encode(msg: &Msg) -> String {
    let env = Envelope {
        v: PROTOCOL_VERSION,
        msg: msg.clone(),
    };
    // Serialization of these plain types cannot fail; keep a valid line regardless.
    serde_json::to_string(&env).unwrap_or_else(|e| {
        format!(
            r#"{{"v":{PROTOCOL_VERSION},"type":"error","code":"internal","message":"encode failed","detail":{:?}}}"#,
            e.to_string()
        )
    })
}

/// `Ok(None)` for non-protocol noise, `Err` for a malformed or foreign-version message.
pub(crate) fn decode(line: &str) -> Result<Option<Msg>, String> {
    let line = line.trim();
    if !line.starts_with('{') {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("invalid JSON: {e}"))?;
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(PROTOCOL_VERSION) => {}
        Some(v) => return Err(format!("protocol version {v}, expected {PROTOCOL_VERSION}")),
        None => return Err("missing protocol version".into()),
    }
    serde_json::from_value::<Envelope>(value)
        .map(|e| Some(e.msg))
        .map_err(|e| format!("invalid message: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> AdapterReport {
        AdapterReport {
            name: "NVIDIA GeForce RTX 4070 SUPER".into(),
            vendor_id: 0x10DE,
            device_id: 0x2783,
            backend: "dx12".into(),
            device_type: "discrete".into(),
            luid: Some("luid:00000000-0000D1B3".into()),
            driver: String::new(),
            driver_info: "581.57".into(),
        }
    }

    #[test]
    fn every_message_roundtrips() {
        let msgs = vec![
            Msg::Hello { pid: 42 },
            Msg::Adapters {
                adapters: vec![adapter()],
            },
            Msg::Selected {
                adapter: adapter(),
                matched_by: MatchKind::Luid,
            },
            Msg::Measuring,
            Msg::Progress {
                phase: phase::MEASURE.into(),
                fraction: 0.5,
                flops_per_s: Some(3.2e13),
            },
            Msg::Progress {
                phase: phase::WARMUP.into(),
                fraction: 0.0,
                flops_per_s: None,
            },
            Msg::Done {
                result: WorkerResult {
                    flops_per_invocation: 32_768,
                    invocations_per_dispatch: 4_194_304,
                    dispatches_per_batch: 8,
                    batches: 400,
                    total_flops: 1.0e15,
                    timed_ms: 20_000,
                    warmup_ms: 2_000,
                    points: vec![Point {
                        elapsed_ms: 1_000,
                        flops_per_s: 3.1e13,
                    }],
                    verified_elements: 64,
                    // Dyadic value: exact through JSON without float_roundtrip.
                    max_rel_error: 0.000_244_140_625,
                },
            },
            Msg::Error {
                code: ErrorCode::NotSupported,
                message: "no adapter".into(),
                detail: Some("x".into()),
            },
        ];
        for m in msgs {
            let line = encode(&m);
            assert!(!line.contains('\n'), "{line}");
            assert!(line.starts_with(r#"{"v":1,"type":"#), "{line}");
            assert_eq!(decode(&line), Ok(Some(m)));
        }
    }

    #[test]
    fn wire_format_is_camel_case() {
        let line = encode(&Msg::Selected {
            adapter: adapter(),
            matched_by: MatchKind::PciIds,
        });
        assert!(line.contains(r#""type":"selected""#), "{line}");
        assert!(line.contains(r#""matchedBy":"pciIds""#), "{line}");
        assert!(line.contains(r#""vendorId":4318"#), "{line}");
    }

    #[test]
    fn noise_is_skipped_and_bad_lines_rejected() {
        assert_eq!(decode("running 1 test"), Ok(None));
        assert_eq!(decode(""), Ok(None));
        assert!(decode(r#"{"v":2,"type":"hello","pid":1}"#).is_err());
        assert!(decode(r#"{"type":"hello","pid":1}"#).is_err());
        assert!(decode(r#"{"v":1,"type":"nope"}"#).is_err());
        assert!(decode("{not json").is_err());
        assert_eq!(
            decode("  {\"v\":1,\"type\":\"hello\",\"pid\":7}\r"),
            Ok(Some(Msg::Hello { pid: 7 }))
        );
    }
}
