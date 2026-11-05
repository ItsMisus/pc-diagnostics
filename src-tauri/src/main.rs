// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The GPU benchmark runs in a child process (this same executable started with a
    // worker flag), so a GPU driver crash or TDR can never take down the app.
    if let Some(exit_code) = bench::gpu_worker::run_if_requested() {
        std::process::exit(exit_code);
    }
    pc_diagnostics_lib::run();
}
