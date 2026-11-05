//! String redaction for shareable reports.
//!
//! Structural redaction (hostname, volume labels, device instance ids, executable paths)
//! happens in `prepare`. This scrubber then rewrites every remaining string:
//! 1. device instance ids (and their last segment if ≥ 8 chars, often a serial) → `<id dispositivo>`
//! 2. user profile paths `…\Users\<name>…` / `…/Users/<name>…` → `…\Users\<utente>…`
//! 3. each detected user name (whole word, ASCII case-insensitive) → `<utente>`
//! 4. the hostname (whole word, ASCII case-insensitive) → `<host>`
//!
//! The placeholders follow the report language (English: `<device id>`, `<user>`, `<host>`).

use models::settings::Language;
use serde_json::Value;

use super::ReportBundle;

/// Replacement texts, in the report language.
#[derive(Debug, Clone, Copy)]
struct Placeholders {
    user: &'static str,
    host: &'static str,
    device_id: &'static str,
}

impl Placeholders {
    fn new(lang: Language) -> Self {
        Self {
            user: lang.pick("<utente>", "<user>"),
            host: "<host>",
            device_id: lang.pick("<id dispositivo>", "<device id>"),
        }
    }
}

impl Default for Placeholders {
    fn default() -> Self {
        Self::new(Language::Italian)
    }
}

/// Profile folder names that are not personal (including the placeholders themselves).
const NON_PERSONAL_PROFILES: [&str; 6] = [
    "public",
    "default",
    "default user",
    "all users",
    "<utente>",
    "<user>",
];
const MIN_SERIAL_SEGMENT: usize = 8;

#[derive(Debug, Default)]
pub struct Redactor {
    users: Vec<String>,
    hostname: Option<String>,
    device_ids: Vec<String>,
    placeholders: Placeholders,
}

impl Redactor {
    /// Collects the sensitive tokens from the *unredacted* data, including fields that
    /// are never exported (process executable paths, all device instance ids).
    pub fn from_bundle(bundle: &ReportBundle) -> Self {
        let mut r = Self {
            placeholders: Placeholders::new(bundle.language),
            ..Self::default()
        };
        let mut strings = Vec::new();
        let mut push = |v: Result<Value, serde_json::Error>| {
            if let Ok(v) = v {
                collect_strings(&v, &mut strings);
            }
        };
        push(serde_json::to_value(bundle.report));
        push(serde_json::to_value(bundle.system));
        push(serde_json::to_value(bundle.inventory));
        push(serde_json::to_value(bundle.benchmarks));
        push(serde_json::to_value(bundle.drivers));
        push(serde_json::to_value(bundle.processes));
        for s in &strings {
            for u in profile_names(s) {
                if !NON_PERSONAL_PROFILES.contains(&u.to_ascii_lowercase().as_str())
                    && !r.users.contains(&u)
                {
                    r.users.push(u);
                }
            }
        }
        r.hostname = bundle
            .system
            .and_then(|s| s.hostname.clone())
            .filter(|h| h.trim().len() >= 2);
        if let Some(d) = bundle.drivers {
            for dev in &d.devices {
                if !dev.instance_id.is_empty() {
                    r.device_ids.push(dev.instance_id.clone());
                    if let Some(last) = dev.instance_id.rsplit('\\').next()
                        && last.len() >= MIN_SERIAL_SEGMENT
                        && last.len() < dev.instance_id.len()
                    {
                        r.device_ids.push(last.to_string());
                    }
                }
            }
        }
        // Longest first so a full id is replaced before its segment.
        r.device_ids
            .sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        r.device_ids.dedup();
        r.users
            .sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        r
    }

    pub fn scrub(&self, s: &str) -> String {
        let ph = self.placeholders;
        let mut out = s.to_string();
        for id in &self.device_ids {
            out = replace_ci(&out, id, ph.device_id, false);
        }
        out = scrub_profile_paths(&out, ph.user);
        for u in &self.users {
            out = replace_ci(&out, u, ph.user, true);
        }
        if let Some(h) = &self.hostname {
            out = replace_ci(&out, h, ph.host, true);
        }
        out
    }

    pub fn scrub_value(&self, v: &mut Value) {
        match v {
            Value::String(s) => *s = self.scrub(s),
            Value::Array(a) => a.iter_mut().for_each(|x| self.scrub_value(x)),
            Value::Object(o) => o.values_mut().for_each(|x| self.scrub_value(x)),
            _ => {}
        }
    }
}

fn collect_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| collect_strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

const PROFILE_MARKERS: [&str; 2] = ["\\users\\", "/users/"];

fn is_separator(c: char) -> bool {
    matches!(
        c,
        '\\' | '/' | '"' | '\'' | '\n' | '\r' | '\t' | ';' | '|' | '<' | '>'
    )
}

/// Byte ranges `(start, end)` of the profile folder name after each `\Users\` marker.
fn profile_ranges(s: &str) -> Vec<(usize, usize)> {
    let lower = s.to_ascii_lowercase(); // ASCII lowering keeps byte offsets identical
    let mut ranges = Vec::new();
    for m in PROFILE_MARKERS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(m) {
            let start = from + pos + m.len();
            let end = s[start..]
                .char_indices()
                .find(|(_, c)| is_separator(*c))
                .map_or(s.len(), |(i, _)| start + i);
            if end > start {
                ranges.push((start, end));
            }
            from = start;
        }
    }
    ranges.sort_unstable();
    ranges
}

fn profile_names(s: &str) -> Vec<String> {
    profile_ranges(s)
        .into_iter()
        .map(|(a, b)| s[a..b].trim().to_string())
        .filter(|n| n.len() >= 2)
        .collect()
}

fn scrub_profile_paths(s: &str, user_placeholder: &str) -> String {
    let ranges = profile_ranges(s);
    if ranges.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (a, b) in ranges {
        if a < last {
            continue;
        }
        out.push_str(&s[last..a]);
        out.push_str(user_placeholder);
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replaces every ASCII-case-insensitive occurrence of `needle`. With `whole_word`, the
/// match must not be preceded or followed by a letter, digit or underscore.
pub fn replace_ci(haystack: &str, needle: &str, repl: &str, whole_word: bool) -> String {
    let n = needle.len();
    if n == 0 || haystack.len() < n {
        return haystack.to_string();
    }
    let hb = haystack.as_bytes();
    let nb = needle.as_bytes();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    let mut last = 0;
    while i + n <= hb.len() {
        if haystack.is_char_boundary(i)
            && haystack.is_char_boundary(i + n)
            && hb[i..i + n].eq_ignore_ascii_case(nb)
        {
            let before_ok = !whole_word
                || haystack[..i]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !is_word_char(c));
            let after_ok = !whole_word
                || haystack[i + n..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word_char(c));
            if before_ok && after_ok {
                out.push_str(&haystack[last..i]);
                out.push_str(repl);
                i += n;
                last = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&haystack[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_paths_are_replaced() {
        assert_eq!(
            scrub_profile_paths(r"C:\Users\Mario Rossi\AppData\x.exe", "<utente>"),
            r"C:\Users\<utente>\AppData\x.exe"
        );
        assert_eq!(
            scrub_profile_paths("c:/users/anna/doc", "<utente>"),
            "c:/users/<utente>/doc"
        );
        assert_eq!(
            scrub_profile_paths(r"\\?\C:\USERS\bob", "<utente>"),
            r"\\?\C:\USERS\<utente>"
        );
        assert_eq!(
            scrub_profile_paths("nessun percorso", "<utente>"),
            "nessun percorso"
        );
        assert_eq!(
            profile_names(r"C:\Users\Public\x"),
            vec!["Public".to_string()]
        );
    }

    #[test]
    fn whole_word_replacement() {
        assert_eq!(
            replace_ci("anna e annata", "anna", "<u>", true),
            "<u> e annata"
        );
        assert_eq!(replace_ci("ANNA.log", "anna", "<u>", true), "<u>.log");
        assert_eq!(replace_ci("aXb", "x", "-", false), "a-b");
        assert_eq!(replace_ci("città anna", "anna", "<u>", true), "città <u>");
        assert_eq!(replace_ci("", "x", "-", true), "");
    }
}
