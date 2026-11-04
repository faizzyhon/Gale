// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Run by the MSI on uninstall: remove our "start with Windows" entry, then exit without starting the app.
    if std::env::args().any(|a| a == "--uninstall-cleanup") {
        gale_app::uninstall_cleanup();
        return;
    }
    gale_app::run()
}
