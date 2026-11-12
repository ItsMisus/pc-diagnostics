//! Redaction of everything that leaves the machine.
//!
//! Same rules and placeholders as the shareable-report redactor in the diagnostics crate
//! (which is crate-private there), plus path stripping, applied to every string of the
//! payload in this order:
//! 1. device instance ids (`PCI\VEN_10DE&DEV_2484&SUBSYS_…\4&1A2B…`): only the bus and
//!    the vendor/device/product parts are kept (`PCI\VEN_10DE&DEV_2484\<id dispositivo>`);
//!    serial-like trailing segments and SUBSYS/REV parts are dropped.
//! 2. absolute file paths (`C:\dir\file`, `\\server\share\…`, `\\?\C:\…`, POSIX paths
//!    under `/Users/`, `/Volumes/`, `/Applications/` and the other system roots): the directory
//!    part is replaced by `<percorso>`; the file name is kept only for `.exe`/`.sys`/`.dll`
//!    (diagnostically useful). Bare drive roots (`C:\`) are kept: actions need them.
//! 3. remaining user profile segments `…\Users\<name>…` → `…\Users\<utente>…`.
//! 4. each known user name (whole word, ASCII case-insensitive) → `<utente>`.
//! 5. the hostname (whole word, ASCII case-insensitive) → `<host>`.

use serde_json::Value;

pub const USER_PLACEHOLDER: &str = "<utente>";
pub const HOST_PLACEHOLDER: &str = "<host>";
pub const DEVICE_ID_PLACEHOLDER: &str = "<id dispositivo>";
pub const PATH_PLACEHOLDER: &str = "<percorso>";

const NON_PERSONAL_PROFILES: [&str; 5] =
    ["public", "default", "default user", "all users", "<utente>"];
const MIN_SECRET_LEN: usize = 2;

/// Enumerator prefixes of Windows device instance ids.
const DEVICE_BUSES: [&str; 22] = [
    "PCI",
    "USB",
    "HID",
    "ACPI",
    "HDAUDIO",
    "SWD",
    "ROOT",
    "BTH",
    "BTHENUM",
    "BTHLE",
    "BTHLEDEVICE",
    "DISPLAY",
    "SCSI",
    "STORAGE",
    "USBSTOR",
    "NVME",
    "UMB",
    "MMDEVAPI",
    "INTELAUDIO",
    "SD",
    "TS_USB_HUB",
    "WPDBUSENUMROOT",
];
/// Parts of the hardware-id segment that identify the model, not the unit.
const KEPT_ID_PARTS: [&str; 4] = ["VEN_", "DEV_", "VID_", "PID_"];
const KEPT_FILE_EXTENSIONS: [&str; 3] = [".exe", ".sys", ".dll"];

#[derive(Debug, Clone, Default)]
pub struct Redactor {
    users: Vec<String>,
    hostname: Option<String>,
}

impl Redactor {
    /// `users`: known account names (e.g. `USERNAME`); profile folder names found in
    /// `extra_sources` are added. Names shorter than 2 chars are ignored.
    pub fn new<'a>(
        hostname: Option<&str>,
        users: impl IntoIterator<Item = &'a str>,
        extra_sources: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let mut r = Self {
            users: Vec::new(),
            hostname: hostname
                .map(str::trim)
                .filter(|h| h.len() >= MIN_SECRET_LEN)
                .map(str::to_owned),
        };
        for u in users {
            r.add_user(u);
        }
        for s in extra_sources {
            for u in profile_names(s) {
                r.add_user(&u);
            }
        }
        r.users
            .sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        r
    }

    /// Account names of the current user from the environment: on Windows `USERNAME`
    /// and the last segment of `USERPROFILE`; elsewhere (macOS) `USER`, `LOGNAME` and the
    /// last segment of `HOME` (`/Users/<name>`).
    pub fn current_user_names() -> Vec<String> {
        #[cfg(windows)]
        let (names, profile) = (["USERNAME"].as_slice(), "USERPROFILE");
        #[cfg(not(windows))]
        let (names, profile) = (["USER", "LOGNAME"].as_slice(), "HOME");
        let mut out = Vec::new();
        for var in names {
            if let Ok(u) = std::env::var(var) {
                out.push(u);
            }
        }
        if let Ok(p) = std::env::var(profile)
            && let Some(last) = p.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next()
        {
            out.push(last.to_owned());
        }
        out
    }

    fn add_user(&mut self, u: &str) {
        let u = u.trim();
        if u.len() < MIN_SECRET_LEN
            || NON_PERSONAL_PROFILES.contains(&u.to_ascii_lowercase().as_str())
            || self.users.iter().any(|x| x.eq_ignore_ascii_case(u))
        {
            return;
        }
        self.users.push(u.to_owned());
    }

    pub fn scrub(&self, s: &str) -> String {
        let mut out = scrub_device_ids(s);
        out = scrub_paths(&out);
        out = scrub_profile_paths(&out);
        for u in &self.users {
            out = replace_ci(&out, u, USER_PLACEHOLDER, true);
        }
        if let Some(h) = &self.hostname {
            out = replace_ci(&out, h, HOST_PLACEHOLDER, true);
        }
        out
    }

    /// Scrubs every string (not keys) of a JSON value in place.
    pub fn scrub_value(&self, v: &mut Value) {
        match v {
            Value::String(s) => *s = self.scrub(s),
            Value::Array(a) => a.iter_mut().for_each(|x| self.scrub_value(x)),
            Value::Object(o) => o.values_mut().for_each(|x| self.scrub_value(x)),
            _ => {}
        }
    }
}

fn is_hard_separator(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '\n' | '\r' | '\t' | ';' | '|' | '<' | '>' | ',' | '(' | ')' | '[' | ']'
    )
}

// ---- device instance ids ----

fn scrub_device_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        // Next whitespace/separator-delimited token.
        let start = rest
            .char_indices()
            .find(|(_, c)| !c.is_whitespace() && !is_hard_separator(*c))
            .map_or(rest.len(), |(i, _)| i);
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .char_indices()
            .find(|(_, c)| c.is_whitespace() || is_hard_separator(*c))
            .map_or(rest.len(), |(i, _)| i);
        let token = &rest[..end];
        out.push_str(&redact_device_id(token).unwrap_or_else(|| token.to_owned()));
        rest = &rest[end..];
    }
    out
}

/// `Some(redacted)` when `token` looks like a device instance / hardware id.
fn redact_device_id(token: &str) -> Option<String> {
    let mut segs = token.split('\\');
    let bus = segs.next()?;
    let hw = segs.next()?;
    if !DEVICE_BUSES.iter().any(|b| b.eq_ignore_ascii_case(bus)) || hw.is_empty() {
        return None;
    }
    let kept: Vec<&str> = hw
        .split('&')
        .filter(|p| {
            KEPT_ID_PARTS
                .iter()
                .any(|k| p.len() > k.len() && p[..k.len()].eq_ignore_ascii_case(k))
        })
        .collect();
    let mut out = bus.to_owned();
    if !kept.is_empty() {
        out.push('\\');
        out.push_str(&kept.join("&"));
    }
    out.push('\\');
    out.push_str(DEVICE_ID_PLACEHOLDER);
    Some(out)
}

// ---- user profile folders ----

const PROFILE_MARKERS: [&str; 2] = ["\\users\\", "/users/"];

fn is_profile_separator(c: char) -> bool {
    matches!(c, '\\' | '/') || is_hard_separator(c)
}

fn profile_ranges(s: &str) -> Vec<(usize, usize)> {
    let lower = s.to_ascii_lowercase(); // ASCII lowering keeps byte offsets
    let mut ranges = Vec::new();
    for m in PROFILE_MARKERS {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(m) {
            let start = from + pos + m.len();
            let end = s[start..]
                .char_indices()
                .find(|(_, c)| is_profile_separator(*c))
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

/// Profile folder names after each `\Users\` marker.
pub fn profile_names(s: &str) -> Vec<String> {
    profile_ranges(s)
        .into_iter()
        .map(|(a, b)| s[a..b].trim().to_owned())
        .filter(|n| n.len() >= MIN_SECRET_LEN)
        .collect()
}

fn scrub_profile_paths(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (a, b) in profile_ranges(s) {
        if a < last {
            continue;
        }
        out.push_str(&s[last..a]);
        out.push_str(USER_PLACEHOLDER);
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

// ---- file paths ----

/// Top-level folders that start an absolute POSIX path (macOS): only these, so that
/// prose such as "I/O" or "MB/s" is never taken for a path.
const POSIX_ROOTS: [&str; 11] = [
    "/Users/",
    "/Volumes/",
    "/Applications/",
    "/Library/",
    "/System/",
    "/private/",
    "/var/",
    "/tmp/",
    "/opt/",
    "/usr/",
    "/home/",
];

/// Byte index where an absolute path starts at `i` (not preceded by a word char):
/// `X:\` / `X:/`, `\\` (UNC / `\\?\`) or a POSIX path under one of [`POSIX_ROOTS`].
fn path_start_at(s: &str, i: usize) -> bool {
    let b = s.as_bytes();
    let prev_ok = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
    if !prev_ok || i >= b.len() {
        return false;
    }
    if i + 2 < b.len() && b[i].is_ascii_alphabetic() && b[i + 1] == b':' {
        return matches!(b[i + 2], b'\\' | b'/');
    }
    if b[i] == b'/' {
        // Byte comparison: `i` may not be a char boundary when called after whitespace.
        return POSIX_ROOTS.iter().any(|r| b[i..].starts_with(r.as_bytes()));
    }
    i + 1 < b.len() && b[i] == b'\\' && b[i + 1] == b'\\'
}

/// End (exclusive) of the path starting at `start`: the last separator before a hard
/// separator / another path start, then the file name up to whitespace.
fn path_end(s: &str, start: usize) -> usize {
    let b = s.as_bytes();
    let mut region_end = s.len();
    let mut i = start + 2;
    while i < b.len() {
        let c = s[i..].chars().next().unwrap_or(' ');
        if is_hard_separator(c) || (c.is_whitespace() && path_start_at(s, i + 1)) {
            region_end = i;
            break;
        }
        i += c.len_utf8();
    }
    let last_sep = s[start..region_end]
        .rfind(['\\', '/'])
        .map_or(start, |p| start + p);
    let tail_end = s[last_sep + 1..region_end]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map_or(region_end, |(j, _)| last_sep + 1 + j);
    // Trailing punctuation belongs to the sentence.
    let trimmed = s[last_sep + 1..tail_end].trim_end_matches(['.', ':', '!', '?']);
    last_sep + 1 + trimmed.len()
}

fn redact_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let is_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    // A bare root such as `C:\` stays.
    if is_drive && path[2..].trim_matches(['\\', '/']).is_empty() {
        return path.to_owned();
    }
    let file = path.rsplit(['\\', '/']).next().unwrap_or("");
    let lower = file.to_ascii_lowercase();
    if !file.is_empty() && KEPT_FILE_EXTENSIONS.iter().any(|e| lower.ends_with(e)) {
        format!("{PATH_PLACEHOLDER}\\{file}")
    } else {
        PATH_PLACEHOLDER.to_owned()
    }
}

fn scrub_paths(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut last = 0;
    while i < s.len() {
        if s.is_char_boundary(i) && path_start_at(s, i) {
            let end = path_end(s, i);
            out.push_str(&s[last..i]);
            out.push_str(&redact_path(&s[i..end]));
            i = end.max(i + 1);
            last = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&s[last.min(s.len())..]);
    out
}

// ---- whole-word replacement ----

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replaces every ASCII-case-insensitive occurrence of `needle`; with `whole_word` the
/// match must not touch a letter, digit or underscore.
pub fn replace_ci(haystack: &str, needle: &str, repl: &str, whole_word: bool) -> String {
    let n = needle.len();
    if n == 0 || haystack.len() < n {
        return haystack.to_owned();
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

    fn r() -> Redactor {
        Redactor::new(Some("DESKTOP-MARIO1"), ["mario"], [])
    }

    #[test]
    fn device_ids_keep_bus_and_model_parts_only() {
        assert_eq!(
            r().scrub(r"Scheda PCI\VEN_10DE&DEV_2484&SUBSYS_146B10DE&REV_A1\4&2A4C1E0&0&0008 ok"),
            r"Scheda PCI\VEN_10DE&DEV_2484\<id dispositivo> ok"
        );
        assert_eq!(
            r().scrub(r"USB\VID_046D&PID_C52B\5&3A1B2C3D&0&2"),
            r"USB\VID_046D&PID_C52B\<id dispositivo>"
        );
        assert_eq!(
            r().scrub(r"(SWD\MMDEVAPI\{0.0.0.00000000}.{abc})"),
            r"(SWD\<id dispositivo>)"
        );
        // Not a device id.
        assert_eq!(r().scrub(r"rete\locale"), r"rete\locale");
    }

    #[test]
    fn paths_are_reduced_and_roots_kept() {
        let s = r().scrub(r"Il file C:\Program Files\Vendor\App\app.exe usa D:\Dati\foto.jpg.");
        assert_eq!(s, r"Il file <percorso>\app.exe usa <percorso>.");
        assert_eq!(
            r().scrub(r"Volume C:\ quasi pieno"),
            r"Volume C:\ quasi pieno"
        );
        assert_eq!(r().scrub(r"\\SERVER\share\doc.txt"), "<percorso>");
        assert_eq!(
            r().scrub(r"driver C:\Windows\System32\drivers\nvlddmkm.sys, codice 43"),
            r"driver <percorso>\nvlddmkm.sys, codice 43"
        );
    }

    #[test]
    fn users_and_hostname_are_scrubbed() {
        let red = Redactor::new(
            Some("DESKTOP-MARIO1"),
            ["mario"],
            [r"C:\Users\Anna Bianchi\AppData\x.exe"],
        );
        assert_eq!(red.scrub("Cuffie di Mario"), "Cuffie di <utente>");
        assert_eq!(
            red.scrub("Telefono di Anna Bianchi"),
            "Telefono di <utente>"
        );
        assert_eq!(red.scrub("PC desktop-mario1 in rete"), "PC <host> in rete");
        // Whole word only.
        assert_eq!(red.scrub("mariottide"), "mariottide");
        assert_eq!(
            red.scrub(r"profilo c:/users/luca/doc"),
            "profilo <percorso>"
        );
    }

    #[test]
    fn posix_paths_are_reduced() {
        assert_eq!(
            r().scrub("Il file /Users/mario/Progetti/cliente/offerta.pdf è grande."),
            "Il file <percorso> è grande."
        );
        assert_eq!(
            r().scrub("(/Applications/Foo.app/Contents/MacOS/Foo)"),
            "(<percorso>)"
        );
        assert_eq!(
            r().scrub("/Volumes/Backup Mario/foto.jpg; ok"),
            "<percorso>; ok"
        );
        // Prose with slashes is not a path.
        assert_eq!(r().scrub("I/O a 30 MB/s e 1/2"), "I/O a 30 MB/s e 1/2");
        assert_eq!(r().scrub("cartella /Utenti/x"), "cartella /Utenti/x");
        // Multi-byte whitespace before a path start does not split a char.
        assert_eq!(r().scrub("a\u{a0}/Users/mario/x b"), "a\u{a0}<percorso> b");
    }

    #[test]
    fn non_personal_and_short_names_are_ignored() {
        let red = Redactor::new(None, ["a", "Public"], [r"C:\Users\Default\x"]);
        assert_eq!(red.scrub("a Public Default"), "a Public Default");
    }

    #[test]
    fn non_ascii_text_survives() {
        assert_eq!(r().scrub("città è più già"), "città è più già");
        assert_eq!(r().scrub("Ç:\\x"), "Ç:\\x");
    }
}
