// Build script. `tauri_build` generates the code Tauri needs (window config,
// permission tables, the Windows icon resource).
//
// We list our own commands here so Tauri generates one permission per command
// (`allow-get-state`, `allow-add-timer`, ...). Once commands are listed, a
// window can only call the ones its capability file grants it. Without this
// list, every window could call every command.
const COMMANDS: &[&str] = &[
    "get_state",
    "report_shown",
    "add_timer",
    "remove_timer",
    "expire_all",
    "show_main",
    "quit_app",
    "get_settings",
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
