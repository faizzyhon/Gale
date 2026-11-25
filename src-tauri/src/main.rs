// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Run by the MSI on uninstall: remove our "start with Windows" entry, then exit without starting the app.
    if std::env::args().any(|a| a == "--uninstall-cleanup") {
        gale_app::uninstall_cleanup();
        return;
    }
    // Moving from the old per-user install: run by the MSI right after installing, and again on every start
    // (before the single-instance check, so a still-running old copy cannot swallow this launch).
    if !gale_app::startup::is_packaged() && !gale_app::startup::is_portable() {
        gale_app::startup::remove_per_user_install();
    }
    // Store package: the package path changes with every update, so the browser keys are refreshed on each start.
    gale_app::startup::register_native_host();
    if std::env::args().any(|a| a == "--migrate") {
        return;
    }
    gale_app::run()
}
