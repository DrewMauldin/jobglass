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
        .run(tauri::generate_context!())
        .expect("JobGlass desktop runtime failed");
}
