#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Before anything else: the app's single-instance check would otherwise take over.
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        std::process::exit(stash_lib::run_mcp());
    }
    stash_lib::run()
}
