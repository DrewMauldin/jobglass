pub mod adapters;
pub mod app;
pub mod diagnostics;
pub mod export;
pub mod input;
pub mod model;
pub mod process;

#[cfg(test)]
mod boundary_tests;
#[cfg(test)]
mod model_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app::scan_jobs, app::render_export])
        .run(tauri::generate_context!())
        .expect("JobGlass desktop runtime failed");
}
