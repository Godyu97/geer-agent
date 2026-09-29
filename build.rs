fn main() {
    #[cfg(feature = "gui")]
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .config_path("src/ui/gui/tauri.conf.json")
            .capabilities_path_pattern("src/ui/gui/capabilities/*.json"),
    )
    .expect("GUI 构建失败；请先在 src/ui/gui/frontend 执行 npm ci && npm run build");
}
