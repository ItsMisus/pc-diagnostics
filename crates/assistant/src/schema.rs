//! JSON schema for the model's answer (`output_config.format`).
//!
//! Structured-output rules: every object has `additionalProperties: false` and `required`
//! listing all of its properties; only object/array/string/integer/number/boolean/null,
//! `enum`, `const`, `anyOf`, `$ref`/`$defs`; no numeric/length bounds, no recursion.
//! Field names match the `models::assistant` serde names, so an action object
//! deserializes straight into `RemediationAction`.

use serde_json::{Value, json};

/// Serde names of `models::assistant::SettingsPage`, in declaration order.
pub const SETTINGS_PAGES: [&str; 12] = [
    "windowsUpdate",
    "windowsUpdateOptional",
    "storageSense",
    "display",
    "sound",
    "bluetooth",
    "network",
    "power",
    "troubleshoot",
    "startupApps",
    "installedApps",
    "privacy",
];

/// Serde `kind` tags of `models::assistant::RemediationAction`.
pub const ACTION_KINDS: [&str; 7] = [
    "openSettingsPage",
    "openDeviceManager",
    "openDiskCleanup",
    "emptyRecycleBin",
    "cleanTempFiles",
    "closeProgram",
    "runSystemFileCheck",
];

fn obj(props: Value) -> Value {
    let required: Vec<Value> = props
        .as_object()
        .map(|m| m.keys().map(|k| Value::String(k.clone())).collect())
        .unwrap_or_default();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": props,
    })
}

fn kind(tag: &str) -> Value {
    json!({ "type": "string", "const": tag })
}

/// The schema sent with every request.
pub fn fix_plan_schema() -> Value {
    let level = json!({ "type": "string", "enum": ["low", "medium", "high"] });
    let defs = json!({
        "openSettingsPage": obj(json!({
            "kind": kind("openSettingsPage"),
            "page": { "type": "string", "enum": SETTINGS_PAGES },
        })),
        "openDeviceManager": obj(json!({ "kind": kind("openDeviceManager") })),
        "openDiskCleanup": obj(json!({
            "kind": kind("openDiskCleanup"),
            "drive": { "type": "string" },
        })),
        "emptyRecycleBin": obj(json!({ "kind": kind("emptyRecycleBin") })),
        "cleanTempFiles": obj(json!({
            "kind": kind("cleanTempFiles"),
            "olderThanDays": { "type": "integer" },
        })),
        "closeProgram": obj(json!({
            "kind": kind("closeProgram"),
            "pid": { "type": "integer" },
            "name": { "type": "string" },
        })),
        "runSystemFileCheck": obj(json!({ "kind": kind("runSystemFileCheck") })),
        "step": obj(json!({
            "id": { "type": "string" },
            "title": { "type": "string" },
            "explanation": { "type": "string" },
            "risk": level,
            "requiresAdmin": { "type": "boolean" },
            "reversible": { "type": "boolean" },
            "action": {
                "anyOf": ACTION_KINDS
                    .iter()
                    .map(|k| json!({ "$ref": format!("#/$defs/{k}") }))
                    .chain(std::iter::once(json!({ "type": "null" })))
                    .collect::<Vec<_>>(),
            },
            "manualInstructions": { "anyOf": [{ "type": "string" }, { "type": "null" }] },
        })),
    });
    let mut root = obj(json!({
        "summary": { "type": "string" },
        "confidence": level,
        "steps": { "type": "array", "items": { "$ref": "#/$defs/step" } },
        "warnings": { "type": "array", "items": { "type": "string" } },
    }));
    if let Some(m) = root.as_object_mut() {
        m.insert("$defs".into(), defs);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::assistant::{RemediationAction, SettingsPage};

    const ALLOWED_KEYS: [&str; 9] = [
        "type",
        "additionalProperties",
        "required",
        "properties",
        "items",
        "enum",
        "const",
        "anyOf",
        "$ref",
    ];
    const ALLOWED_TYPES: [&str; 7] = [
        "object", "array", "string", "integer", "number", "boolean", "null",
    ];

    /// Walks a schema node (`path` for messages); `in_def` = name of the `$defs` entry
    /// being walked, to detect recursion.
    fn walk(v: &Value, path: &str, in_def: Option<&str>, defs: &serde_json::Map<String, Value>) {
        let m = v
            .as_object()
            .unwrap_or_else(|| panic!("{path}: not an object"));
        for k in m.keys() {
            if path == "#" && k == "$defs" {
                continue;
            }
            assert!(
                ALLOWED_KEYS.contains(&k.as_str()),
                "{path}: keyword {k} not allowed"
            );
        }
        if let Some(t) = m.get("type") {
            let t = t
                .as_str()
                .unwrap_or_else(|| panic!("{path}: type must be a string"));
            assert!(ALLOWED_TYPES.contains(&t), "{path}: type {t}");
            if t == "object" {
                assert_eq!(
                    m.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "{path}"
                );
                let props = m["properties"].as_object().expect("properties");
                let mut req: Vec<&str> = m["required"]
                    .as_array()
                    .expect("required")
                    .iter()
                    .map(|x| x.as_str().expect("str"))
                    .collect();
                let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
                req.sort_unstable();
                keys.sort_unstable();
                assert_eq!(req, keys, "{path}: required must list every property");
                for (k, p) in props {
                    walk(p, &format!("{path}/{k}"), in_def, defs);
                }
            }
            if t == "array" {
                walk(&m["items"], &format!("{path}/items"), in_def, defs);
            }
        }
        if let Some(r) = m.get("$ref") {
            let name = r
                .as_str()
                .and_then(|s| s.strip_prefix("#/$defs/"))
                .unwrap_or_else(|| panic!("{path}: bad $ref"));
            assert!(defs.contains_key(name), "{path}: dangling $ref {name}");
            assert_ne!(Some(name), in_def, "{path}: recursive $ref");
            // `step` may reference actions; actions reference nothing.
            if in_def.is_some_and(|d| d != "step") {
                panic!("{path}: nested $ref from {in_def:?}");
            }
        }
        if let Some(a) = m.get("anyOf") {
            for (i, x) in a.as_array().expect("anyOf array").iter().enumerate() {
                walk(x, &format!("{path}/anyOf/{i}"), in_def, defs);
            }
        }
    }

    #[test]
    fn schema_satisfies_structured_output_constraints() {
        let s = fix_plan_schema();
        let defs = s["$defs"].as_object().expect("$defs").clone();
        walk(&s, "#", None, &defs);
        for (name, d) in &defs {
            walk(d, &format!("#/$defs/{name}"), Some(name), &defs);
        }
        let text = s.to_string();
        for banned in [
            "minimum",
            "maximum",
            "minLength",
            "maxLength",
            "minItems",
            "maxItems",
            "pattern",
            "format",
        ] {
            assert!(!text.contains(&format!("\"{banned}\"")), "{banned}");
        }
    }

    #[test]
    fn action_defs_round_trip_into_remediation_action() {
        let samples = [
            json!({"kind": "openSettingsPage", "page": "windowsUpdate"}),
            json!({"kind": "openDeviceManager"}),
            json!({"kind": "openDiskCleanup", "drive": "C:"}),
            json!({"kind": "emptyRecycleBin"}),
            json!({"kind": "cleanTempFiles", "olderThanDays": 7}),
            json!({"kind": "closeProgram", "pid": 42, "name": "app.exe"}),
            json!({"kind": "runSystemFileCheck"}),
        ];
        let defs = fix_plan_schema()["$defs"].clone();
        for (k, sample) in ACTION_KINDS.iter().zip(samples) {
            let props = defs[k]["properties"].as_object().expect("props");
            let mut a: Vec<&String> = props.keys().collect();
            let mut b: Vec<&String> = sample.as_object().expect("o").keys().collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "{k}");
            let parsed: RemediationAction = serde_json::from_value(sample).expect(k);
            let back = serde_json::to_value(&parsed).expect("ser");
            assert_eq!(back["kind"], *k);
        }
        for p in SETTINGS_PAGES {
            let _: SettingsPage = serde_json::from_value(json!(p)).expect(p);
        }
    }
}
