//! Temperatures from the I/O Kit HID thermal sensors (Apple Silicon) or the SMC keys
//! `sysinfo` reads on Intel Macs. No root needed.
//!
//! Which labels count:
//! - CPU, preferred: the CPU-cluster sensors `pACC MTR Temp Sensor*` /
//!   `eACC MTR Temp Sensor*` (P/E CPU complexes), present on M1/M2-class chips.
//! - CPU, fallback: the SoC die sensors `PMU tdie*` / `PMU2 tdie*` (e.g. M5: `PMU tdie1`
//!   to `PMU tdie14`, `PMU2 tdie1` to `PMU2 tdie10`). The CPU cores sit on that die
//!   together with the GPU and NPU, and Apple does not publish which sensor covers which
//!   block, so the value is the hottest point of the SoC die (the same "hottest sensor"
//!   convention as a package/Tdie reading on a PC).
//! - CPU on Intel Macs: `PECI CPU` (die temperature over PECI). `CPU Proximity` is a
//!   heat-spreader sensor next to the package, not the die, and is not used.
//! - Never used: `PMU tdev*` (sensors near other board devices), `PMU tcal` (a fixed
//!   calibration value, not a reading), battery, NAND, ambient-light sensors.
//! - GPU: `GPU MTR Temp Sensor*` when present; there is no GPU-specific sensor on chips
//!   that only expose `PMU tdie*` (the shared die temperature is reported under CPU only,
//!   so it is not shown twice).
//!
//! Readings outside 1–150 °C are discarded (unconnected channels read e.g. −22 °C).

use sysinfo::Components;

/// Which reading a component label feeds, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorRole {
    /// A CPU-cluster sensor (Apple Silicon `pACC`/`eACC`, Intel `PECI CPU`).
    CpuCluster,
    /// An SoC die sensor (`PMU tdie*`): used for the CPU only when no cluster sensor exists.
    SocDie,
    Gpu,
}

pub fn classify(label: &str) -> Option<SensorRole> {
    let l = label.trim();
    if l.starts_with("pACC MTR Temp Sensor")
        || l.starts_with("eACC MTR Temp Sensor")
        || l == "PECI CPU"
    {
        return Some(SensorRole::CpuCluster);
    }
    if l.starts_with("GPU MTR Temp Sensor") {
        return Some(SensorRole::Gpu);
    }
    // `PMU tdie3`, `PMU2 tdie10`: "PMU" + optional digits, then "tdie" + digits.
    let (pmu, sensor) = l.split_once(' ')?;
    let pmu_ok = pmu
        .strip_prefix("PMU")
        .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()));
    let die_ok = sensor
        .strip_prefix("tdie")
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    (pmu_ok && die_ok).then_some(SensorRole::SocDie)
}

pub fn plausible(t: f32) -> Option<f64> {
    let t = f64::from(t);
    (t.is_finite() && (1.0..=150.0).contains(&t)).then_some(t)
}

fn max_of(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    values
        .into_iter()
        .fold(None, |m, v| Some(m.map_or(v, |m: f64| m.max(v))))
}

/// CPU and GPU temperatures from labelled readings, following the rules in the module
/// docs. Returns `(cpu, gpu)`.
pub fn select<'a>(
    readings: impl IntoIterator<Item = (&'a str, Option<f32>)>,
) -> (Option<f64>, Option<f64>) {
    let mut cluster = Vec::new();
    let mut die = Vec::new();
    let mut gpu = Vec::new();
    for (label, t) in readings {
        let Some(t) = t.and_then(plausible) else {
            continue;
        };
        match classify(label) {
            Some(SensorRole::CpuCluster) => cluster.push(t),
            Some(SensorRole::SocDie) => die.push(t),
            Some(SensorRole::Gpu) => gpu.push(t),
            None => {}
        }
    }
    let cpu = if cluster.is_empty() {
        max_of(die)
    } else {
        max_of(cluster)
    };
    (cpu, max_of(gpu))
}

/// Holds the sensor list and refreshes only the sensors that feed a reading.
pub struct Sensors {
    components: Option<Components>,
    /// Indices into `components` of sensors with a role.
    used: Vec<usize>,
}

impl Sensors {
    pub fn new() -> Self {
        let mut s = Self {
            components: None,
            used: Vec::new(),
        };
        s.enumerate();
        s
    }

    fn enumerate(&mut self) {
        match Components::new_with_refreshed_list() {
            Ok(c) => {
                self.used = c
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| classify(x.label()).is_some())
                    .map(|(i, _)| i)
                    .collect();
                self.components = Some(c);
            }
            Err(e) => {
                tracing::warn!(error = %e, "thermal sensor enumeration failed");
                self.components = None;
                self.used.clear();
            }
        }
    }

    /// Whether a sensor that can feed the CPU reading was found at enumeration.
    pub fn has_cpu_sensors(&self) -> bool {
        let Some(c) = self.components.as_ref() else {
            return false;
        };
        self.used.iter().filter_map(|&i| c.get(i)).any(|x| {
            matches!(
                classify(x.label()),
                Some(SensorRole::CpuCluster | SensorRole::SocDie)
            )
        })
    }

    /// Refreshes the used sensors and returns `(cpu, gpu)` in °C.
    pub fn read(&mut self) -> (Option<f64>, Option<f64>) {
        let Some(c) = self.components.as_mut() else {
            return (None, None);
        };
        for &i in &self.used {
            if let Some(x) = c.get_mut(i) {
                x.refresh();
            }
        }
        select(
            self.used
                .iter()
                .filter_map(|&i| c.get(i))
                .map(|x| (x.label(), x.temperature())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_classified_exactly() {
        assert_eq!(classify("PMU tdie1"), Some(SensorRole::SocDie));
        assert_eq!(classify("PMU2 tdie10"), Some(SensorRole::SocDie));
        assert_eq!(classify("PMU tdev1"), None);
        assert_eq!(classify("PMU tcal"), None);
        assert_eq!(classify("PMU2 tcal"), None);
        assert_eq!(classify("PMU tdie"), None);
        assert_eq!(classify("PMUx tdie1"), None);
        assert_eq!(classify("gas gauge battery"), None);
        assert_eq!(classify("NAND CH0 temp"), None);
        assert_eq!(
            classify("pACC MTR Temp Sensor2"),
            Some(SensorRole::CpuCluster)
        );
        assert_eq!(
            classify("eACC MTR Temp Sensor0"),
            Some(SensorRole::CpuCluster)
        );
        assert_eq!(classify("GPU MTR Temp Sensor1"), Some(SensorRole::Gpu));
        assert_eq!(classify("PECI CPU"), Some(SensorRole::CpuCluster));
        assert_eq!(classify("CPU Proximity"), None);
    }

    #[test]
    fn cluster_sensors_win_over_die_and_bad_readings_are_dropped() {
        let (cpu, gpu) = select([
            ("PMU tdie1", Some(70.0)),
            ("pACC MTR Temp Sensor2", Some(55.0)),
            ("eACC MTR Temp Sensor0", Some(48.0)),
            ("GPU MTR Temp Sensor1", Some(44.0)),
            ("PMU tcal", Some(99.0)),
        ]);
        assert_eq!(cpu, Some(55.0));
        assert_eq!(gpu, Some(44.0));

        let (cpu, gpu) = select([
            ("PMU tdie1", Some(43.3)),
            ("PMU2 tdie2", Some(40.8)),
            ("PMU tdev1", Some(-22.2)),
            ("PMU tcal", Some(51.8)),
            ("PMU tdie9", None),
        ]);
        assert_eq!(cpu, Some(f64::from(43.3_f32)));
        assert_eq!(gpu, None);

        assert_eq!(select([("PMU tdie1", Some(-3.0))]), (None, None));
        assert_eq!(select([("PMU tdie1", Some(f32::NAN))]), (None, None));
    }

    #[test]
    fn host_cpu_temperature_is_plausible_if_present() {
        let mut s = Sensors::new();
        let (cpu, gpu) = s.read();
        if crate::macos::physical_apple_silicon() {
            assert!(s.has_cpu_sensors(), "Apple Silicon exposes PMU die sensors");
            let t = cpu.expect("CPU/SoC die temperature");
            assert!((10.0..=110.0).contains(&t), "{t} °C");
        }
        if let Some(g) = gpu {
            assert!((10.0..=110.0).contains(&g));
        }
    }
}
