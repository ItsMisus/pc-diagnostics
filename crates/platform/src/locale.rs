//! Windows display language, used to resolve `LanguagePreference::System`.

/// Whether the user's Windows display (UI) language is Italian, in any region.
/// `false` on other platforms and when the language cannot be read.
pub fn display_language_is_italian() -> bool {
    #[cfg(windows)]
    {
        // SAFETY: no arguments, no preconditions; returns a LANGID.
        let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        is_italian_lang_id(lang_id)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// The primary language is the low 10 bits of a LANGID; `LANG_ITALIAN` is 0x10.
#[cfg_attr(not(windows), allow(dead_code))]
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
}
