//! Device and driver rules (Configuration Manager problem codes, Windows Update offers).

use models::Source;
use models::component::ComponentKind;
use models::diagnostics::{Confidence, FindingKind, Severity, SimpleExplanation};
use models::drivers::{DeviceCategory, DeviceHealth, DeviceRecord, UpdateStatus};
use models::settings::Language;

use super::{Cov, Ctx, Outcome, ev, finding, simple, strings};
use crate::fmt;

/// Device names listed in the "updates available" finding before summarizing the rest.
pub const UPDATES_LISTED_MAX: usize = 10;

/// Categories whose malfunction usually blocks core use of the PC.
pub fn is_critical_category(c: DeviceCategory) -> bool {
    matches!(
        c,
        DeviceCategory::Gpu
            | DeviceCategory::StorageController
            | DeviceCategory::Disk
            | DeviceCategory::Ethernet
            | DeviceCategory::Wifi
    )
}

pub fn category_label(lang: Language, c: DeviceCategory) -> &'static str {
    match c {
        DeviceCategory::Gpu => lang.pick("Scheda video", "Graphics card"),
        DeviceCategory::Chipset => lang.pick(
            "Chipset e dispositivi di sistema",
            "Chipset and system devices",
        ),
        DeviceCategory::Audio => "Audio",
        DeviceCategory::Ethernet => lang.pick("Rete Ethernet", "Ethernet network"),
        DeviceCategory::Wifi => "Wi-Fi",
        DeviceCategory::Bluetooth => "Bluetooth",
        DeviceCategory::StorageController => {
            lang.pick("Controller di archiviazione", "Storage controller")
        }
        DeviceCategory::Disk => lang.pick("Disco", "Disk"),
        DeviceCategory::Usb => "USB",
        DeviceCategory::Input => lang.pick("Dispositivi di input", "Input devices"),
        DeviceCategory::Display => "Monitor",
        DeviceCategory::Other => lang.pick("Altro", "Other"),
    }
}

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Drivers);
    let lang = ctx.lang;
    let Some(inv) = ctx.input.drivers else {
        cov.not_evaluated(lang.pick(
            "inventario dei dispositivi non raccolto",
            "device inventory not collected",
        ));
        out.coverage.push(cov.finish());
        return out;
    };

    for d in &inv.devices {
        if matches!(
            d.health,
            DeviceHealth::ProblemDetected | DeviceHealth::DriverMissing
        ) {
            out.findings.push(device_finding(lang, d));
        }
    }

    let updates: Vec<&DeviceRecord> = inv
        .devices
        .iter()
        .filter(|d| matches!(d.update, UpdateStatus::UpdateAvailable { .. }))
        .collect();
    if !updates.is_empty() {
        let n = updates.len();
        let mut names: Vec<String> = updates
            .iter()
            .take(UPDATES_LISTED_MAX)
            .map(|d| d.name.clone())
            .collect();
        let plain_names = if n > UPDATES_LISTED_MAX {
            fmt::join_names_and_more(lang, &names, n - UPDATES_LISTED_MAX)
        } else {
            fmt::join_names(lang, &names)
        };
        if n > UPDATES_LISTED_MAX {
            let more = n - UPDATES_LISTED_MAX;
            names.push(lang.pick(format!("e altri {more}"), format!("and {more} more")));
        }
        let mut f = finding(
            "drivers.updates.available",
            ComponentKind::Drivers,
            Severity::Info,
            FindingKind::Observation,
            Confidence::High,
            lang.pick(
                "Aggiornamenti dei driver disponibili",
                "Driver updates available",
            ),
            lang.pick(
                format!("Windows Update offre un driver applicabile per {n} dispositivi."),
                format!(
                    "Windows Update offers an applicable driver for {n} {}.",
                    if n == 1 { "device" } else { "devices" }
                ),
            ),
            simple(
                if n == 1 {
                    lang.pick(
                        "È disponibile un aggiornamento per un driver",
                        "A driver update is available",
                    )
                    .to_string()
                } else {
                    lang.pick(
                        format!("Sono disponibili aggiornamenti per {n} driver"),
                        format!("Updates are available for {n} drivers"),
                    )
                },
                lang.pick(
                    format!("Windows Update offre una versione più recente del driver (il programma che permette a Windows di usare un componente) per: {plain_names}. Non significa che il driver attuale abbia problemi."),
                    format!("Windows Update offers a newer version of the driver (the program that lets Windows use a component) for: {plain_names}. This doesn't mean the current driver has problems."),
                ),
                lang.pick(
                    "Se vuoi, puoi installare gli aggiornamenti da Impostazioni > Windows Update > Opzioni avanzate > Aggiornamenti facoltativi. Se tutto funziona bene, non è obbligatorio.",
                    "If you want, you can install the updates from Settings > Windows Update > Advanced options > Optional updates. If everything works well, you don't have to.",
                ),
            ),
        );
        f.evidence = updates
            .iter()
            .take(UPDATES_LISTED_MAX)
            .map(|d| {
                let offered = match &d.update {
                    UpdateStatus::UpdateAvailable { title, version, .. } => {
                        version.as_deref().map_or_else(
                            || title.clone(),
                            |v| {
                                lang.pick(
                                    format!("{title} (versione {v})"),
                                    format!("{title} (version {v})"),
                                )
                            },
                        )
                    }
                    _ => String::new(),
                };
                ev(d.name.clone(), offered, Source::WindowsUpdate)
            })
            .collect();
        f.evidence.push(ev(
            lang.pick("Dispositivi", "Devices"),
            names.join(", "),
            Source::WindowsUpdate,
        ));
        f.recommendation = lang.pick(
            "Gli aggiornamenti possono essere installati da Impostazioni > Windows Update > Opzioni avanzate > Aggiornamenti facoltativi, se si desidera.",
            "If desired, the updates can be installed from Settings > Windows Update > Advanced options > Optional updates.",
        ).into();
        f.limitations = strings(&[lang.pick(
            "Un aggiornamento disponibile non significa che il driver attuale sia difettoso.",
            "An available update does not mean the current driver is faulty.",
        )]);
        out.findings.push(f);
    }

    match &inv.update_check {
        None => cov.partial(lang.pick(
            "verifica aggiornamenti non eseguita",
            "update check not run",
        )),
        Some(c) if c.error.is_some() => {
            cov.partial(lang.pick("verifica aggiornamenti non riuscita", "update check failed"))
        }
        Some(_) => {}
    }
    let s = &inv.summary;
    let (total, working, problems, missing, disabled) =
        (s.total, s.working, s.problems, s.driver_missing, s.disabled);
    cov.note(lang.pick(
        format!("{total} dispositivi presenti: {working} funzionanti, {problems} con problemi, {missing} senza driver, {disabled} disattivati"),
        format!(
            "{total} {} present: {working} working, {problems} with problems, {missing} without a driver, {disabled} disabled",
            if total == 1 { "device" } else { "devices" }
        ),
    ));
    if s.disabled > 0 {
        cov.note(lang.pick(
            "i dispositivi disattivati non sono considerati malfunzionamenti",
            "disabled devices are not counted as malfunctions",
        ));
    }
    out.coverage.push(cov.finish());
    out
}

fn device_finding(lang: Language, d: &DeviceRecord) -> models::diagnostics::Finding {
    let missing = d.health == DeviceHealth::DriverMissing;
    let gpu = d.category == DeviceCategory::Gpu;
    let rule_id = match (gpu, missing) {
        (true, _) => "gpu.driver.problem",
        (false, true) => "drivers.device.driver_missing",
        (false, false) => "drivers.device.problem",
    };
    let severity = if is_critical_category(d.category) {
        Severity::Critical
    } else {
        Severity::Warning
    };
    let code_text = d
        .problem_description
        .clone()
        .or_else(|| {
            d.problem_code
                .map(|c| lang.pick(format!("Codice {c}"), format!("Code {c}")))
        })
        .unwrap_or_else(|| {
            lang.pick(
                "Windows segnala un problema senza codice",
                "Windows reports a problem without a code",
            )
            .into()
        });
    let name = &d.name;
    let category = category_label(lang, d.category);
    let mut f = finding(
        rule_id,
        if gpu {
            ComponentKind::Gpu
        } else {
            ComponentKind::Drivers
        },
        severity,
        FindingKind::Confirmed,
        // Without a CM problem code the status read itself failed: less authoritative.
        if d.problem_code.is_some() {
            Confidence::High
        } else {
            Confidence::Medium
        },
        if missing {
            lang.pick(
                format!("Driver mancante: {name}"),
                format!("Missing driver: {name}"),
            )
        } else {
            lang.pick(
                format!("Dispositivo con problemi: {name}"),
                format!("Device with problems: {name}"),
            )
        },
        lang.pick(
            format!("Windows segnala un problema per il dispositivo «{name}» ({category})."),
            format!("Windows reports a problem with the device “{name}” ({category})."),
        ),
        device_simple(lang, d),
    );
    f.evidence = vec![ev(
        lang.pick("Stato segnalato da Windows", "Status reported by Windows"),
        code_text,
        Source::ConfigManager,
    )];
    // The status text already reads "Code 28: …"; a separate code row only when it does not.
    if let Some(c) = d
        .problem_code
        .filter(|c| !f.evidence[0].value.contains(&c.to_string()))
    {
        f.evidence.push(ev(
            lang.pick("Codice problema", "Problem code"),
            c.to_string(),
            Source::ConfigManager,
        ));
    }
    if let Some(v) = d.driver.as_ref().and_then(|x| x.version.as_deref()) {
        f.evidence.push(ev(
            lang.pick("Versione driver installata", "Installed driver version"),
            v,
            Source::ConfigManager,
        ));
    }
    f.possible_causes = if missing {
        strings(&[lang.pick(
            "Driver non installato o installazione non completata",
            "Driver not installed or installation not completed",
        )])
    } else {
        strings(lang.pick(
            &[
                "Driver danneggiato o non compatibile",
                "Dispositivo che ha segnalato un errore",
            ],
            &[
                "Damaged or incompatible driver",
                "The device reported an error",
            ],
        ))
    };
    f.recommendation = if missing {
        lang.pick(
            "Installare il driver dal sito del produttore del dispositivo o tramite Windows Update, poi riavviare.",
            "Install the driver from the device manufacturer's website or through Windows Update, then restart.",
        )
    } else {
        lang.pick(
            "Aprire Gestione dispositivi per i dettagli; riavviare il PC e, se il problema persiste, reinstallare il driver dal sito del produttore.",
            "Open Device Manager for details; restart the PC and, if the problem persists, reinstall the driver from the manufacturer's website.",
        )
    }
    .into();
    f.limitations = strings(&[lang.pick(
        "Il codice problema è fornito da Windows; la causa precisa può richiedere verifiche ulteriori.",
        "The problem code is provided by Windows; the exact cause may need further checks.",
    )]);
    f
}

/// What may stop working when a device of this category fails, in everyday words.
pub fn plain_consequence(lang: Language, c: DeviceCategory) -> &'static str {
    match c {
        DeviceCategory::Gpu => lang.pick(
            "Lo schermo, i giochi e i video potrebbero funzionare male o più lentamente.",
            "The screen, games, and videos might work poorly or more slowly.",
        ),
        DeviceCategory::Chipset => lang.pick(
            "Alcune funzioni di base del PC potrebbero non funzionare bene.",
            "Some basic PC functions might not work well.",
        ),
        DeviceCategory::Audio => lang.pick(
            "L'audio (casse, cuffie o microfono) potrebbe non funzionare.",
            "Sound (speakers, headphones, or microphone) might not work.",
        ),
        DeviceCategory::Ethernet => lang.pick(
            "La connessione a Internet via cavo potrebbe non funzionare.",
            "The wired internet connection might not work.",
        ),
        DeviceCategory::Wifi => lang.pick(
            "La connessione Wi-Fi potrebbe non funzionare.",
            "The Wi-Fi connection might not work.",
        ),
        DeviceCategory::Bluetooth => lang.pick(
            "Cuffie, mouse o altri dispositivi Bluetooth potrebbero non collegarsi.",
            "Headphones, mice, or other Bluetooth devices might not connect.",
        ),
        DeviceCategory::StorageController | DeviceCategory::Disk => lang.pick(
            "L'accesso ai dischi e ai file potrebbe non funzionare correttamente.",
            "Access to disks and files might not work properly.",
        ),
        DeviceCategory::Usb => lang.pick(
            "Le periferiche USB (chiavette, mouse, tastiere) potrebbero non funzionare.",
            "USB devices (flash drives, mice, keyboards) might not work.",
        ),
        DeviceCategory::Input => lang.pick(
            "Tastiera, mouse o touchpad potrebbero non funzionare correttamente.",
            "The keyboard, mouse, or touchpad might not work properly.",
        ),
        DeviceCategory::Display => lang.pick(
            "Il monitor potrebbe non funzionare correttamente.",
            "The monitor might not work properly.",
        ),
        DeviceCategory::Other => lang.pick(
            "Le funzioni che dipendono da questo dispositivo potrebbero non essere disponibili.",
            "Features that depend on this device might not be available.",
        ),
    }
}

fn restart_then_reinstall(lang: Language) -> &'static str {
    lang.pick(
        "Riavvia il PC. Se il problema resta, reinstalla il driver scaricandolo dal sito del produttore del dispositivo.",
        "Restart the PC. If the problem remains, reinstall the driver by downloading it from the device manufacturer's website.",
    )
}

/// Name for the plain-language text. A device without a driver often has no friendly
/// name, and Windows then shows its hardware id (e.g. `ACPI\AMDI0204\2&DABA3FF&0`),
/// which means nothing to a non-technical user; the id stays in the technical details.
fn plain_device_name(lang: Language, d: &DeviceRecord) -> String {
    let n = d.name.trim();
    let looks_like_id = n.is_empty()
        || n.eq_ignore_ascii_case(d.instance_id.trim())
        || (n.contains('\\') && !n.contains(' '));
    if looks_like_id {
        // Same wording as Device Manager for devices without a name.
        lang.pick(
            "Dispositivo sconosciuto (il codice è nei dettagli tecnici)",
            "Unknown device (its ID is in the technical details)",
        )
        .into()
    } else {
        n.to_owned()
    }
}

/// Plain-language text for a device finding, chosen by health and Windows problem code.
/// Each arm holds (headline, meaning, action) in Italian and English.
fn device_simple(lang: Language, d: &DeviceRecord) -> SimpleExplanation {
    let name = &plain_device_name(lang, d);
    let restart = restart_then_reinstall(lang);
    let (headline, what, action) = if d.health == DeviceHealth::DriverMissing {
        (
            lang.pick(
                format!("Manca il driver di un dispositivo: {name}"),
                format!("A device is missing its driver: {name}"),
            ),
            lang.pick(
                "Windows non ha installato correttamente il driver, cioè il programma che gli permette di usare questo dispositivo, quindi il dispositivo non può funzionare.",
                "The driver (the program that lets Windows use this device) wasn't installed correctly, so the device can't work.",
            ),
            lang.pick(
                "Installa il driver dal sito del produttore del dispositivo o da Windows Update, poi riavvia il PC.",
                "Install the driver from the device manufacturer's website or from Windows Update, then restart the PC.",
            ),
        )
    } else {
        match d.problem_code {
            None => (
                lang.pick(
                    format!("Windows non è riuscito a controllare un dispositivo: {name}"),
                    format!("Windows couldn't check a device: {name}"),
                ),
                lang.pick(
                    "Windows non ha fornito lo stato di questo dispositivo, quindi non si sa se funziona correttamente.",
                    "Windows didn't report the status of this device, so it's not known whether it works properly.",
                ),
                restart,
            ),
            Some(14) => (
                lang.pick(
                    format!("Un dispositivo ha bisogno di un riavvio del PC: {name}"),
                    format!("A device needs the PC to restart: {name}"),
                ),
                lang.pick(
                    "Il dispositivo non può funzionare correttamente finché il PC non viene riavviato.",
                    "The device can't work properly until the PC is restarted.",
                ),
                lang.pick("Riavvia il PC.", "Restart the PC."),
            ),
            Some(43) => (
                lang.pick(
                    format!("Windows ha fermato un dispositivo perché ha dato errori: {name}"),
                    format!("Windows stopped a device because it reported errors: {name}"),
                ),
                lang.pick(
                    "Il dispositivo ha segnalato un problema e Windows lo ha bloccato, quindi al momento non funziona.",
                    "The device reported a problem and Windows blocked it, so it isn't working right now.",
                ),
                restart,
            ),
            Some(45) => (
                lang.pick(
                    format!("Un dispositivo non risulta collegato: {name}"),
                    format!("A device doesn't appear to be connected: {name}"),
                ),
                lang.pick(
                    "Windows conosce questo dispositivo ma al momento non lo trova collegato al PC.",
                    "Windows knows this device, but it can't find it connected to the PC right now.",
                ),
                lang.pick(
                    "Se è un dispositivo esterno, controlla che sia collegato bene e riavvia il PC. Se non lo usi più, puoi ignorare questo avviso.",
                    "If it's an external device, check that it's plugged in properly and restart the PC. If you no longer use it, you can ignore this notice.",
                ),
            ),
            Some(47) => (
                lang.pick(
                    format!("Un dispositivo è pronto per essere scollegato: {name}"),
                    format!("A device is ready to be unplugged: {name}"),
                ),
                lang.pick(
                    "Windows ha fermato il dispositivo per poterlo scollegare in sicurezza, quindi al momento non è utilizzabile.",
                    "Windows stopped the device so it can be unplugged safely, so it can't be used right now.",
                ),
                lang.pick(
                    "Scollega il dispositivo e ricollegalo, oppure riavvia il PC.",
                    "Unplug the device and plug it back in, or restart the PC.",
                ),
            ),
            Some(48) => (
                lang.pick(
                    format!("Windows ha bloccato il driver di un dispositivo: {name}"),
                    format!("Windows blocked a device's driver: {name}"),
                ),
                lang.pick(
                    "Il driver (il programma che fa funzionare il dispositivo) è noto per causare problemi con Windows ed è stato bloccato, quindi il dispositivo non funziona.",
                    "The driver (the program that makes the device work) is known to cause problems with Windows and was blocked, so the device doesn't work.",
                ),
                lang.pick(
                    "Cerca una versione aggiornata del driver sul sito del produttore del dispositivo, poi riavvia il PC.",
                    "Look for an updated version of the driver on the device manufacturer's website, then restart the PC.",
                ),
            ),
            Some(52) => (
                lang.pick(
                    format!("Windows ha bloccato il driver di un dispositivo: {name}"),
                    format!("Windows blocked a device's driver: {name}"),
                ),
                lang.pick(
                    "Windows non riesce a verificare che il driver (il programma che fa funzionare il dispositivo) sia autentico e non lo avvia, quindi il dispositivo non funziona.",
                    "Windows can't verify that the driver (the program that makes the device work) is genuine and won't start it, so the device doesn't work.",
                ),
                lang.pick(
                    "Reinstalla il driver scaricandolo dal sito del produttore del dispositivo, poi riavvia il PC.",
                    "Reinstall the driver by downloading it from the device manufacturer's website, then restart the PC.",
                ),
            ),
            Some(3 | 10 | 19 | 31 | 32 | 37 | 38 | 39 | 40 | 41) => (
                lang.pick(
                    format!("Windows non riesce ad avviare un dispositivo: {name}"),
                    format!("Windows can't start a device: {name}"),
                ),
                lang.pick(
                    "Il driver (il programma che fa funzionare il dispositivo) non si avvia correttamente e potrebbe essere danneggiato, quindi il dispositivo non funziona.",
                    "The driver (the program that makes the device work) doesn't start properly and might be damaged, so the device doesn't work.",
                ),
                restart,
            ),
            Some(44) => (
                lang.pick(
                    format!("Un programma ha spento un dispositivo: {name}"),
                    format!("A program turned off a device: {name}"),
                ),
                lang.pick(
                    "Un programma o un servizio ha fermato questo dispositivo, che al momento non funziona.",
                    "A program or service stopped this device, and it isn't working right now.",
                ),
                restart,
            ),
            Some(_) => (
                lang.pick(
                    format!("Un dispositivo non funziona correttamente: {name}"),
                    format!("A device isn't working properly: {name}"),
                ),
                lang.pick(
                    "Windows segnala un problema con questo dispositivo.",
                    "Windows reports a problem with this device.",
                ),
                restart,
            ),
        }
    };
    simple(
        headline,
        format!("{what} {}", plain_consequence(lang, d.category)),
        action,
    )
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::component::ComponentKind;
    use models::diagnostics::{CoverageStatus, DiagnosticReport, FindingKind, Severity};
    use models::drivers::{DeviceCategory as C, DeviceHealth as H, UpdateStatus};

    fn run(inv: &models::drivers::DriverInventory) -> DiagnosticReport {
        let mut inp = input(&[]);
        inp.drivers = Some(inv);
        evaluate(&inp)
    }

    fn cov(r: &DiagnosticReport) -> (CoverageStatus, String) {
        r.coverage
            .iter()
            .find(|c| c.component == ComponentKind::Drivers)
            .map(|c| (c.status, c.note.clone()))
            .expect("cov")
    }

    #[test]
    fn hardware_ids_are_not_shown_as_names_in_plain_text() {
        let mut d = device(r"ACPI\AMDI0204\2&DABA3FF&0", C::Other, H::DriverMissing);
        d.instance_id = r"ACPI\AMDI0204\2&DABA3FF&0".into();
        let r = run(&drivers(vec![d], false));
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "drivers.device.driver_missing")
            .expect("finding");
        assert!(!f.simple.headline.contains("ACPI"), "{}", f.simple.headline);
        assert!(f.simple.headline.contains("Dispositivo sconosciuto"));
        // The technical text keeps the id.
        assert!(f.title.contains("ACPI"));

        let named = run(&drivers(
            vec![device("Scheda audio", C::Audio, H::DriverMissing)],
            false,
        ));
        assert!(named.findings[0].simple.headline.ends_with("Scheda audio"));
    }

    #[test]
    fn problem_devices_are_confirmed_with_category_severity() {
        let inv = drivers(
            vec![
                device("Wifi card", C::Wifi, H::ProblemDetected),
                device("Webcam", C::Other, H::DriverMissing),
                device("GPU", C::Gpu, H::ProblemDetected),
                device("Mouse", C::Input, H::Working),
                device("Old", C::Audio, H::Disabled),
            ],
            true,
        );
        let r = run(&inv);
        let get = |id: &str| r.findings.iter().find(|f| f.rule_id == id).expect(id);
        let wifi = get("drivers.device.problem");
        assert_eq!(wifi.severity, Severity::Critical);
        assert_eq!(wifi.kind, FindingKind::Confirmed);
        assert!(wifi.evidence.iter().any(|e| e.value.contains("Code 43")));
        // The code is in the status text: no second row repeating it.
        assert!(!wifi.evidence.iter().any(|e| e.label == "Problem code"));
        let cam = get("drivers.device.driver_missing");
        assert_eq!(cam.severity, Severity::Warning);
        let gpu = get("gpu.driver.problem");
        assert_eq!(gpu.component, ComponentKind::Gpu);
        assert_eq!(gpu.severity, Severity::Critical);
        assert_eq!(
            r.findings.len(),
            3,
            "disabled/working devices are not problems"
        );
        assert_eq!(cov(&r).0, CoverageStatus::Evaluated);
    }

    #[test]
    fn updates_listed_and_not_verifiable_is_not_a_problem() {
        let mut a = device("Audio chip", C::Audio, H::Working);
        a.update = update_available();
        let mut b = device("Touchpad", C::Input, H::Working);
        b.update = UpdateStatus::NotVerifiable { reason: "x".into() };
        let r = run(&drivers(vec![a, b], true));
        assert_eq!(r.findings.len(), 1);
        let f = &r.findings[0];
        assert_eq!(f.rule_id, "drivers.updates.available");
        assert_eq!(f.severity, Severity::Info);
        assert_eq!(f.kind, FindingKind::Observation);
        assert!(f.evidence.iter().any(|e| e.value.contains("Audio chip")));
    }

    #[test]
    fn update_check_not_run_is_partial() {
        let r = run(&drivers(vec![device("x", C::Usb, H::Working)], false));
        let (status, note) = cov(&r);
        assert_eq!(status, CoverageStatus::Partial);
        assert!(note.contains("verifica aggiornamenti non eseguita"));
        assert_eq!(cov(&evaluate(&input(&[]))).0, CoverageStatus::NotEvaluated);
    }

    #[test]
    fn english_texts() {
        let inv = drivers(vec![device("Webcam", C::Other, H::ProblemDetected)], true);
        let mut inp = input(&[]);
        inp.drivers = Some(&inv);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = &r.findings[0];
        assert_eq!(f.rule_id, "drivers.device.problem");
        assert_eq!(f.title, "Device with problems: Webcam");
        assert_eq!(
            f.description,
            "Windows reports a problem with the device “Webcam” (Other)."
        );
        assert_eq!(f.evidence[0].label, "Status reported by Windows");
        assert!(
            f.simple
                .meaning
                .ends_with("Features that depend on this device might not be available.")
        );
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Drivers)
            .expect("cov");
        assert!(cov.note.starts_with("1 device present: "), "{}", cov.note);
    }
}
