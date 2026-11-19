// Build script. `tauri_build` generates the code Tauri needs (window config,
// permission tables, the Windows icon resource).
//
// We list our own commands here so Tauri generates one permission per command
// (`allow-get-state`, `allow-add-patient`, ...). Once commands are listed, a
// window can only call the ones its capability file grants it. Without this
// list, every window could call every command.
const COMMANDS: &[&str] = &[
    "get_state",
    "add_patient",
    "mark_drop",
    "remove_patient",
    "show_main",
    "quit_app",
    "get_settings",
    "set_interval_minutes",
    "set_wait_minutes",
    "set_autostart",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
