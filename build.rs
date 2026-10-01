fn main() {
    #[cfg(feature = "web")]
    {
        // include_dir 在宏展开时读取文件；显式追踪目录才能在前端重建后重新嵌入。
        println!("cargo:rerun-if-changed=src/ui/frontend/dist/web");
        assert!(
            std::path::Path::new("src/ui/frontend/dist/web/index.html").is_file(),
            "Web 前端不存在；请先执行 make web-frontend，再用 --features web 构建。"
        );
    }
    #[cfg(feature = "gui")]
    println!("cargo:rerun-if-changed=src/ui/frontend/dist/gui");
    #[cfg(feature = "gui")]
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .config_path("src/ui/gui/tauri.conf.json")
            .capabilities_path_pattern("src/ui/gui/capabilities/*.json"),
    )
    .expect("GUI 构建失败；请先在 src/ui/frontend 执行 bun install --frozen-lockfile && bun run build:gui");
}
