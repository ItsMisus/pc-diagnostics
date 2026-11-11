//! The OS display language, used to resolve `LanguagePreference::System`.
//!
//! - Windows: the user's display (UI) language (`GetUserDefaultUILanguage`).
//! - macOS: the first of the user's preferred languages (`CFLocaleCopyPreferredLanguages`).

/// Whether the user's display (UI) language is Italian, in any region. `false` on other
/// platforms and when the language cannot be read.
pub fn display_language_is_italian() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: no arguments, no preconditions; returns a LANGID.
        let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        is_italian_lang_id(lang_id)
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::locale::first_preferred_language()
            .is_some_and(|tag| crate::macos::locale::is_italian_tag(&tag))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        false
    }
}

/// The primary language is the low 10 bits of a LANGID; `LANG_ITALIAN` is 0x10.
#[cfg(any(windows, test))]
fn is_italian_lang_id(lang_id: u16) -> bool {
    lang_id & 0x3ff == 0x10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_language_ignores_region() {
        assert!(is_italian_lang_id(0x0410)); // it-IT
        assert!(is_italian_lang_id(0x0810)); // it-CH
        assert!(!is_italian_lang_id(0x0409)); // en-US
        assert!(!is_italian_lang_id(0x040c)); // fr-FR
    }

    #[test]
    fn host_display_language_returns() {
        eprintln!(
            "display language is Italian: {}",
            display_language_is_italian()
        );
    }
}
