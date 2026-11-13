//! The render loop of dynamic effects: one background thread, started only while a dynamic
//! effect is on at least one real device, stopped (and joined) as soon as it is not. At most
//! 30 frames per second; a device gets a frame only when it differs from the last one sent.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use models::control::lighting::LightingEffect;
use platform::control::lighting::LightingFrame;

use super::LightingBackend;
use super::effect::{color_at, dim, level_at};

/// Frame interval: 30 fps at most.
pub const FRAME: Duration = Duration::from_millis(34);

/// One device in a program.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: String,
    pub color_capable: bool,
    pub lamp_count: u32,
    /// Lamp positions in the program's layout (per device, or across devices when synced).
    pub positions: Vec<f64>,
    /// When this device's timeline starts.
    pub t0: Instant,
}

/// What the devices show.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub effect: LightingEffect,
    pub brightness: u8,
    pub targets: Vec<Target>,
}

/// The frame of `target` at `now`.
pub fn render(program: &Program, target: &Target, now: Instant) -> LightingFrame {
    let t = now.saturating_duration_since(target.t0).as_millis() as u64;
    if target.color_capable {
        let n = target.lamp_count.max(1);
        LightingFrame::Colors(
            (0..n)
                .map(|i| {
                    let pos = target.positions.get(i as usize).copied();
                    dim(color_at(&program.effect, t, i, n, pos), program.brightness)
                })
                .collect(),
        )
    } else {
        // Quantized to 1/200 so tiny steps do not cost a write each.
        let level = level_at(&program.effect, t) * f64::from(program.brightness) / 100.0;
        LightingFrame::Level((level * 200.0).round() / 200.0)
    }
}

pub struct Engine {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Engine {
    /// Starts rendering `program` (shared, so a new program replaces it without a restart).
    pub fn start(backend: Arc<dyn LightingBackend>, program: Arc<Mutex<Option<Program>>>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::Builder::new()
            .name("lighting-engine".into())
            .spawn(move || run(&*backend, &program, &flag))
            .map_err(|e| tracing::warn!(error = %e, "lighting engine did not start"))
            .ok();
        Self { stop, handle }
    }

    pub fn running(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// Stops the thread and waits for it (at most one frame).
    pub fn stop(mut self) {
        self.stop_inner();
    }

    fn stop_inner(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            h.thread().unpark();
            let _ = h.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop_inner();
    }
}

fn run(backend: &dyn LightingBackend, program: &Mutex<Option<Program>>, stop: &AtomicBool) {
    let mut last: Vec<(String, LightingFrame)> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        let started = Instant::now();
        let Some(p) = program.lock().ok().and_then(|g| g.clone()) else {
            break;
        };
        if !p.effect.is_dynamic() {
            break;
        }
        for target in &p.targets {
            if failed.contains(&target.id) {
                continue;
            }
            let frame = render(&p, target, started);
            if last.iter().any(|(id, f)| id == &target.id && f == &frame) {
                continue;
            }
            match backend.write(&target.id, &frame, false) {
                Ok(()) => {
                    last.retain(|(id, _)| id != &target.id);
                    last.push((target.id.clone(), frame));
                }
                Err(e) => {
                    // Unplugged or taken by another app: stop writing to it, keep the rest.
                    tracing::warn!(id = target.id, error = %e.message, "lighting frame failed");
                    failed.push(target.id.clone());
                }
            }
        }
        if failed.len() >= p.targets.len() {
            break;
        }
        let spent = started.elapsed();
        if let Some(rest) = FRAME.checked_sub(spent) {
            std::thread::park_timeout(rest);
        }
    }
}
