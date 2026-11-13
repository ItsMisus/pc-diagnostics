//! Prints the real device inventory of this machine with timings. Read-only.
//!
//! `cargo run -p drivers --example driver_inventory [--release] [-- --updates]`
//! `--updates` also runs the Windows Update driver search (network, may take minutes).

use std::collections::BTreeMap;
use std::time::Instant;

use models::drivers::UpdateStatus;

fn main() {
    if !drivers::inventory_supported() {
        eprintln!("{}", drivers::unsupported_error());
        return;
    }
    let t = Instant::now();
    let raw = platform::devices::enumerate_present_devices().expect("enumerate");
    let setupapi = t.elapsed();
    let t = Instant::now();
    let signed = platform::devices::signed_drivers(drivers::SIGNED_DRIVERS_TIMEOUT);
    let wmi = t.elapsed();
    println!(
        "SetupAPI/CfgMgr32: {} devices in {setupapi:?}; WMI Win32_PnPSignedDriver: {} in {wmi:?}",
        raw.len(),
        signed
            .as_ref()
            .map_or_else(|e| format!("error {e}"), |r| format!("{} rows", r.len())),
    );
    let t = Instant::now();
    let inv = drivers::collect_inventory().expect("collect_inventory");
    println!(
        "collect_inventory (concurrent): {:?}\nsummary: {:?}",
        t.elapsed(),
        inv.summary
    );

    let mut by_cat: BTreeMap<String, usize> = BTreeMap::new();
    let mut unsigned = 0;
    let mut sig_unavailable = 0;
    let mut inbox = 0;
    let mut no_driver = 0;
    for d in &inv.devices {
        *by_cat.entry(format!("{:?}", d.category)).or_default() += 1;
        match &d.driver {
            None => no_driver += 1,
            Some(drv) => {
                inbox += usize::from(drv.is_inbox);
                match drv.signed.value() {
                    Some(false) => unsigned += 1,
                    Some(true) => {}
                    None => sig_unavailable += 1,
                }
            }
        }
    }
    println!("by category: {by_cat:?}");
    println!(
        "driver: inbox {inbox}, no driver {no_driver}, unsigned {unsigned}, signature unavailable {sig_unavailable}"
    );
    for d in inv
        .devices
        .iter()
        .filter(|d| d.problem_code.is_some() || d.problem_description.is_some())
    {
        println!(
            "PROBLEM {:?} [{:?}] {} ({}): {}",
            d.health,
            d.category,
            d.name,
            d.class_name.as_deref().unwrap_or("-"),
            d.problem_description.as_deref().unwrap_or("-")
        );
    }

    if std::env::args().any(|a| a == "--list") {
        for d in &inv.devices {
            let drv = d.driver.as_ref();
            println!(
                "{:?}\t{:?}\t{}\t{}\t{}\t{}\t{}",
                d.health,
                d.category,
                d.class_name.as_deref().unwrap_or("-"),
                d.name,
                drv.and_then(|x| x.provider.as_deref()).unwrap_or("-"),
                drv.and_then(|x| x.version.as_deref()).unwrap_or("-"),
                drv.and_then(|x| x.date.as_deref()).unwrap_or("-"),
            );
        }
    }

    if std::env::args().any(|a| a == "--offers") {
        // Raw offers (search only), to audit matching by hand.
        match platform::devices::search_driver_updates(drivers::UPDATE_SEARCH_TIMEOUT) {
            Ok(offers) => {
                for o in &offers {
                    let holders: Vec<&str> = inv
                        .devices
                        .iter()
                        .filter(|d| {
                            d.hardware_ids.iter().any(|h| {
                                h.to_ascii_lowercase()
                                    .contains(&o.hardware_id.to_ascii_lowercase())
                            })
                        })
                        .map(|d| d.name.as_str())
                        .collect();
                    println!(
                        "OFFER {:?} hwid={} class={:?} provider={:?} date={:?}; devices containing this id: {holders:?}",
                        o.title, o.hardware_id, o.class, o.provider, o.date
                    );
                }
            }
            Err(e) => println!("search failed: {e:?}"),
        }
    }

    if std::env::args().any(|a| a == "--updates") {
        let t = Instant::now();
        match drivers::check_updates(&inv, models::settings::Language::English) {
            Ok(checked) => {
                let uc = checked.update_check.as_ref().expect("update_check");
                println!(
                    "WU search: {:?} (duration_ms {}), offered {}, summary {:?}",
                    t.elapsed(),
                    uc.duration_ms,
                    uc.offered,
                    checked.summary
                );
                for d in &checked.devices {
                    if let UpdateStatus::UpdateAvailable {
                        title,
                        version,
                        date,
                        hardware_id,
                        ..
                    } = &d.update
                    {
                        println!(
                            "  UPDATE {} <- {title} (version {version:?}, date {date:?}) via {hardware_id}",
                            d.name
                        );
                    }
                }
            }
            Err(e) => println!("WU search failed after {:?}: {e:?}", t.elapsed()),
        }
    }
}
