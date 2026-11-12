//! The user's preferred UI language on macOS (System Settings > General > Language &
//! Region > Preferred Languages), read with `CFLocaleCopyPreferredLanguages`. No
//! subprocess, no `defaults`.

use objc2_core_foundation::{CFArray, CFLocale, CFString, CFType};

/// First entry of the user's preferred languages as a BCP 47 tag (`it-IT`, `en-US`,
/// `zh-Hans-CN`), or `None` when the list is empty or unreadable.
pub fn first_preferred_language() -> Option<String> {
    let langs = CFLocale::preferred_languages()?;
    // SAFETY: a CFArray stores CF objects (CFTypeRef), so viewing its elements as
    // `CFType` is valid; each element is then type-checked by `downcast`. The array is
    // our own +1 copy and is not mutated while borrowed.
    let langs: &CFArray<CFType> = unsafe { langs.cast_unchecked::<CFType>() };
    let first = langs.get(0)?.downcast::<CFString>().ok()?;
    Some(first.to_string())
}

/// Whether the primary language subtag of a BCP 47 / POSIX tag is `it`
/// (`it`, `it-IT`, `it_CH`, `IT-ch` → true; `en-IT`, `ita`, `` → false).
pub fn is_italian_tag(tag: &str) -> bool {
    tag.trim()
        .split(['-', '_'])
        .next()
        .is_some_and(|primary| primary.eq_ignore_ascii_case("it"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_subtag_decides() {
        for yes in ["it", "it-IT", "it-CH", "it_IT", "IT-ch", " it-IT "] {
            assert!(is_italian_tag(yes), "{yes}");
        }
        for no in [
            "",
            "en",
            "en-IT",
            "en-US",
            "ita",
            "fr-IT",
            "zh-Hans-IT",
            "-it",
        ] {
            assert!(!is_italian_tag(no), "{no}");
        }
    }

    /// Reads the real preference list of the user running the tests.
    #[test]
    fn host_preferred_language_is_readable() {
        let first = first_preferred_language();
        eprintln!("first preferred language: {first:?}");
        if let Some(tag) = first {
            assert!(!tag.trim().is_empty());
            assert!(tag.is_ascii(), "{tag}");
        }
    }
}
