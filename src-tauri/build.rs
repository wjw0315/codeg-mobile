fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new()
                .commands(&["get_active_token", "log_js_error"])
            ),
    )
    .expect("failed to run tauri-build");
}
