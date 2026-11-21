//! Build script for Tauri.

fn main() {
    // `tauri::generate_context!` fails to compile when `frontendDist`
    // (../dist, see tauri.conf.json) is missing. The directory is Trunk build
    // output and not tracked in git, so make sure it exists on fresh clones.
    let _ = std::fs::create_dir_all("../dist");
    tauri_build::build();
}
