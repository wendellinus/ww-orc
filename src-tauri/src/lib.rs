mod application;
mod bootstrap;
mod features;
mod infrastructure;
mod ipc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    bootstrap::run();
}
