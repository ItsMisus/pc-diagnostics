//! System (operating system) coverage. No OS-level rules exist in rules v1.

use models::component::ComponentKind;

use super::{Cov, Ctx, Outcome, by_os};

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::System);
    let lang = ctx.lang;
    match ctx.input.system {
        None => cov.not_evaluated(lang.pick(
            "informazioni sul sistema operativo non disponibili",
            "operating system information not available",
        )),
        Some(s) => {
            let os = s
                .os_long_version
                .clone()
                .or_else(|| s.os_version.clone())
                .unwrap_or_else(|| {
                    lang.pick("versione non rilevata", "version not detected")
                        .into()
                });
            cov.partial(by_os(
                ctx.platform,
                lang.pick(
                    format!("sistema operativo rilevato ({os}); aggiornamenti di Windows e integrità dei file di sistema non verificati"),
                    format!("operating system detected ({os}); Windows Update status and system file integrity not checked"),
                ),
                lang.pick(
                    format!("sistema operativo rilevato ({os}); aggiornamenti di macOS e integrità del sistema non verificati"),
                    format!("operating system detected ({os}); macOS update status and system integrity not checked"),
                ),
            ));
        }
    }
    out.coverage.push(cov.finish());
    out
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::component::ComponentKind;
    use models::diagnostics::DiagnosticReport;
    use models::settings::Language;

    fn note(r: &DiagnosticReport) -> String {
        r.coverage
            .iter()
            .find(|c| c.component == ComponentKind::System)
            .map(|c| c.note.clone())
            .expect("cov")
    }

    #[test]
    fn coverage_note_in_both_languages() {
        let sys = system();
        let mut inp = input(&[]);
        assert_eq!(
            note(&evaluate(&inp)),
            "informazioni sul sistema operativo non disponibili"
        );
        inp.system = Some(&sys);
        let it = note(&evaluate(&inp));
        assert!(it.starts_with("sistema operativo rilevato ("), "{it}");
        inp.language = Language::English;
        let en = note(&evaluate(&inp));
        assert!(en.starts_with("operating system detected ("), "{en}");
        assert!(en.ends_with("Windows Update status and system file integrity not checked"));
    }

    #[test]
    fn coverage_note_names_macos_on_a_mac() {
        let mut sys = system();
        sys.os_long_version = Some("macOS 27.2.1".into());
        let mut inp = input(&[]);
        inp.platform = models::app::Platform::Macos;
        inp.system = Some(&sys);
        let it = note(&evaluate(&inp));
        assert!(
            it.ends_with("aggiornamenti di macOS e integrità del sistema non verificati"),
            "{it}"
        );
        inp.language = Language::English;
        let en = note(&evaluate(&inp));
        assert!(
            en.ends_with("macOS update status and system integrity not checked"),
            "{en}"
        );
        assert!(!en.contains("Windows"), "{en}");
    }
}
