use std::{
    cell::Cell,
    fs,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{BUILTINS, ExecutionMode, ToolKind, Tools, validate_registry};

#[test]
fn registry_rejects_duplicates_and_resolves_all_definitions() {
    assert!(validate_registry(&[ToolKind::Read, ToolKind::Read]).is_err());
    assert!(validate_registry(BUILTINS).is_ok());
    let definitions = Tools::definitions();
    let serialized: Vec<_> = definitions
        .iter()
        .map(|spec| {
            serde_json::json!({
                "name": spec.name, "description": spec.description, "parameters": spec.parameters,
            })
        })
        .collect();
    let bytes = serde_json::to_vec(&serialized)
        .expect("序列化工具定义")
        .len();
    eprintln!("tool definition bytes: {bytes}");
    assert!(bytes < 8_000, "工具定义过长：{bytes} 字节");
    assert_eq!(definitions.len(), 11);
    for spec in definitions {
        assert!(
            spec.description.chars().count() <= 160,
            "{} 描述过长",
            spec.name
        );
        let kind = ToolKind::find(spec.name).expect("声明的工具必须能分发");
        assert_eq!(kind.mode(), Tools::execution_mode(spec.name));
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[tokio::test]
async fn search_options_preserve_workspace_paths_original_lines_and_read_context() {
    use serde_json::json;
    let dir = query_dir();
    fs::create_dir(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/a.rs"),
        "before\n    Foo.bar\nfooXbar\nafter\n",
    )
    .unwrap();
    fs::write(dir.join("other.txt"), "Foo.bar\n").unwrap();
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    tools.allow_all_for_test();
    for (args, expected) in [
        (
            json!({"pattern":"Foo.bar", "path":"src"}),
            "src/a.rs:2:     Foo.bar\n",
        ),
        (
            json!({"pattern":"foo.bar", "path":"src", "fixed_strings":true}),
            "",
        ),
        (
            json!({"pattern":"foo.bar", "path":"src", "ignore_case":true}),
            "src/a.rs:2:     Foo.bar\nsrc/a.rs:3: fooXbar\n",
        ),
        (
            json!({"pattern":"foo.bar", "path":"src", "ignore_case":true, "fixed_strings":true}),
            "src/a.rs:2:     Foo.bar\n",
        ),
        (
            json!({"pattern":"Foo", "glob":"**/*.rs", "output":"files"}),
            "src/a.rs\n",
        ),
    ] {
        let result = tools
            .execute_recorded_in(&dir, "search", &args.to_string())
            .await;
        assert!(result.success, "{}", result.text);
        assert_eq!(body(&result), expected, "{}", result.text);
        assert_eq!(
            metadata(&result)["path_base"],
            dir.canonicalize().unwrap().to_string_lossy().as_ref()
        );
    }
    let read = tools
        .execute_recorded_in(&dir, "read", r#"{"path":"src/a.rs","offset":1,"limit":4}"#)
        .await;
    assert!(read.success);
    assert_eq!(body(&read), "before\n    Foo.bar\nfooXbar\nafter\n");
    let invalid = tools
        .execute_recorded_in(&dir, "search", r#"{"pattern":"["}"#)
        .await;
    assert!(!invalid.success);
    assert_eq!(metadata(&invalid)["code"], "command_failed");
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn search_rejects_escapes_and_uses_the_current_workspace() {
    use serde_json::json;
    let outer = query_dir();
    let dir = outer.join("repo");
    let sibling = outer.join("repo-extra");
    fs::create_dir(&dir).unwrap();
    fs::create_dir(&sibling).unwrap();
    fs::write(dir.join("marker"), "inside\n").unwrap();
    fs::write(sibling.join("marker"), "outside\n").unwrap();
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    let prompts = Rc::new(Cell::new(0));
    let seen = Rc::clone(&prompts);
    tools.set_confirm(move |prompt| {
        assert!(prompt.contains("workspace"));
        seen.set(seen.get() + 1);
        Ok(true)
    });
    for path in [
        "../repo-extra".to_owned(),
        sibling.to_string_lossy().into_owned(),
    ] {
        let result = tools
            .execute_recorded_in(
                &dir,
                "search",
                &json!({"pattern":"outside", "path":path}).to_string(),
            )
            .await;
        assert!(!result.success);
        assert_eq!(metadata(&result)["code"], "outside_workspace");
    }
    assert_eq!(prompts.get(), 0, "越界参数在授权前拒绝");
    let first = tools
        .execute_recorded_in(&dir, "search", r#"{"pattern":"inside"}"#)
        .await;
    assert_eq!(body(&first), "marker:1: inside\n");
    let absolute_inside = tools
        .execute_recorded_in(
            &dir,
            "search",
            &json!({"pattern":"inside", "path":dir.join("marker")}).to_string(),
        )
        .await;
    assert!(absolute_inside.success);
    tools.reset();
    let second = tools
        .execute_recorded_in(&sibling, "search", r#"{"pattern":"outside"}"#)
        .await;
    assert_eq!(body(&second), "marker:1: outside\n");
    assert_eq!(prompts.get(), 2);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&sibling, dir.join("escape")).unwrap();
        std::os::unix::fs::symlink(sibling.join("marker"), dir.join("escape-file")).unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("loop")).unwrap();
        use std::os::unix::ffi::OsStringExt;
        let byte_path = dir.join(std::ffi::OsString::from_vec(b"byte-\xff".to_vec()));
        fs::write(&byte_path, "inside\n").unwrap();
        std::os::unix::fs::symlink(&byte_path, dir.join("byte-alias")).unwrap();
        let invalid_path = tools
            .execute_recorded_in(
                &dir,
                "search",
                r#"{"pattern":"inside","path":"byte-alias"}"#,
            )
            .await;
        assert_eq!(metadata(&invalid_path)["code"], "invalid_path_encoding");
        for path in ["escape", "escape-file"] {
            let result = tools
                .execute_recorded_in(
                    &dir,
                    "search",
                    &json!({"pattern":"outside", "path":path}).to_string(),
                )
                .await;
            assert_eq!(metadata(&result)["code"], "outside_workspace");
        }
        let result = tools
            .execute_recorded_in(&dir, "search", r#"{"pattern":"outside"}"#)
            .await;
        assert!(result.success, "{}", result.text);
        assert_eq!(metadata(&result)["matches"], 0);
        assert!(body(&result).is_empty());
    }
    fs::remove_dir_all(outer).unwrap();
}

#[tokio::test]
async fn search_respects_ignore_rules_binary_size_limits_and_complete_result_budget() {
    use serde_json::json;
    let dir = query_dir();
    fs::write(dir.join(".ignore"), "ignored.txt\n").unwrap();
    fs::write(dir.join("ignored.txt"), "needle\n").unwrap();
    fs::write(dir.join("binary"), b"needle\n\0needle\n").unwrap();
    let mut binary_invalid = b"needle\xff\n".to_vec();
    binary_invalid.extend(vec![b'x'; 200_000]);
    binary_invalid.push(0);
    fs::write(dir.join("binary-invalid"), binary_invalid).unwrap();
    fs::write(
        dir.join("binary-late"),
        format!("{}{}\0", "needle\n".repeat(60), "x".repeat(200_000)),
    )
    .unwrap();
    fs::write(
        dir.join("large"),
        format!("needle\n{}", "x".repeat(1024 * 1024)),
    )
    .unwrap();
    let special = "space ' $ file.txt";
    fs::write(
        dir.join(special),
        "  $(touch should-not-exist); needle [x]\n",
    )
    .unwrap();
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).unwrap();
    tools.allow_all_for_test();
    let result = tools
        .execute_recorded_in(&dir, "search", r#"{"pattern":"needle"}"#)
        .await;
    assert!(result.success, "{}", result.text);
    assert_eq!(
        body(&result),
        format!("{special}:1:   $(touch should-not-exist); needle [x]\n")
    );
    let literal = tools
        .execute_recorded_in(
            &dir,
            "search",
            &json!({"pattern":"$(touch should-not-exist);", "fixed_strings":true,"path":special})
                .to_string(),
        )
        .await;
    assert!(literal.success);
    assert!(!dir.join("should-not-exist").exists());
    for args in [
        json!({"pattern":"needle", "path":"ignored.txt"}),
        json!({"pattern":"needle", "glob":"ignored.txt"}),
    ] {
        let result = tools
            .execute_recorded_in(&dir, "search", &args.to_string())
            .await;
        assert_eq!(body(&result), "ignored.txt:1: needle\n");
    }
    fs::write(dir.join("many"), "needle\n".repeat(80)).unwrap();
    let result = tools
        .execute_recorded_in(&dir, "search", r#"{"pattern":"needle","path":"many"}"#)
        .await;
    assert!(result.success);
    assert!(result.text.chars().count() <= 2000);
    assert_eq!(metadata(&result)["matches"], 50);
    assert_eq!(metadata(&result)["truncated"], true);
    assert_eq!(
        body(&result)
            .lines()
            .filter(|line| line.starts_with("many:"))
            .count(),
        50
    );
    fs::write(dir.join("long"), format!("needle {}\n", "中".repeat(3000))).unwrap();
    let result = tools
        .execute_recorded_in(&dir, "search", r#"{"pattern":"needle","path":"long"}"#)
        .await;
    assert_eq!(metadata(&result)["truncated"], true);
    assert!(!body(&result).contains("long:"), "不得返回半个命中行");
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn disabled_and_invalid_arguments() {
    assert_eq!(Tools::execution_mode("read"), ExecutionMode::Parallel);
    assert_eq!(Tools::execution_mode("write"), ExecutionMode::Sequential);
    assert_eq!(Tools::execution_mode("bash"), ExecutionMode::Sequential);
    assert_eq!(Tools::execution_mode("search"), ExecutionMode::Parallel);
    assert_eq!(
        Tools::execution_mode("web_search"),
        ExecutionMode::Sequential
    );
    assert_eq!(
        Tools::execution_mode("web_fetch"),
        ExecutionMode::Sequential
    );
    let mut tools = Tools::new(false, crate::config::default_bash_bin()).expect("工作目录存在");
    assert!(tools.specs().is_empty());
    assert_eq!(
        Tools::new(true, crate::config::default_bash_bin())
            .expect("工作目录存在")
            .specs()
            .len(),
        11
    );
    assert_eq!(
        tools.execute("get_current_time", "{}").await,
        "工具已关闭。"
    );
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
    assert!(
        tools
            .execute("get_current_time", "{")
            .await
            .contains("JSON")
    );
    assert!(
        tools
            .execute("get_current_time", "[]")
            .await
            .contains("对象")
    );
    assert!(
        tools
            .execute("get_current_time", "{\"x\":1}")
            .await
            .contains("不接受")
    );
    assert!(tools.execute("unknown", "{}").await.contains("未知"));
    assert!(!tools.execute_recorded("unknown", "{}").await.success);
    assert!(
        tools
            .execute_recorded("get_current_time", "{}")
            .await
            .success
    );
}

#[tokio::test]
async fn grants_are_per_tool_and_reset_revokes_them() {
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
    let prompts = Rc::new(Cell::new(0));
    let seen = Rc::clone(&prompts);
    tools.confirm = Box::new(move |prompt| {
        assert!(prompt.contains("本次会话"));
        seen.set(seen.get() + 1);
        Ok(true)
    });
    for _ in 0..2 {
        let result = tools.execute("bash", r#"{"command":"printf ok"}"#).await;
        assert!(result.contains("退出状态：0"), "{result}");
    }
    assert_eq!(prompts.get(), 1);
    tools.reset();
    tools.execute("bash", r#"{"command":"printf ok"}"#).await;
    assert_eq!(prompts.get(), 2);

    tools.reset();
    tools.confirm = Box::new(|_| Ok(false));
    let denied = tools
        .execute_recorded("bash", r#"{"command":"printf denied"}"#)
        .await;
    assert!(denied.text.contains("未执行"));
    assert!(!denied.success);
    assert!(!tools.grants.contains("bash"));

    tools.confirm = Box::new(|_| Ok(true));
    let failed = tools
        .execute_recorded("bash", r#"{"command":"exit 7"}"#)
        .await;
    assert!(!failed.success);
    assert!(failed.text.contains("退出状态：7"));
}

#[tokio::test]
async fn file_tools_use_independent_session_grants_and_absolute_paths() {
    let dir = std::env::temp_dir().join(format!(
        "geer-agent-auth-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("创建目录");
    let path = dir.join("file.txt");
    fs::write(&path, "hello").expect("准备文件");
    let path_json = serde_json::to_string(&path.to_string_lossy().to_string()).expect("编码路径");
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
    let prompts = Rc::new(Cell::new(0));
    let seen = Rc::clone(&prompts);
    tools.confirm = Box::new(move |prompt| {
        assert!(prompt.contains("路径："));
        seen.set(seen.get() + 1);
        Ok(true)
    });
    let read = format!("{{\"path\":{path_json}}}");
    let first = tools.execute_recorded("read", &read).await;
    assert_eq!(first.text.split_once("\n\n").expect("正文").1, "hello");
    assert!(first.success);
    assert!(!first.changed);
    assert!(tools.execute("read", &read).await.ends_with("\n\nhello"));
    assert_eq!(prompts.get(), 1);
    let write = format!("{{\"path\":{path_json},\"content\":\"world\"}}");
    assert!(tools.execute("write", &write).await.contains("文件已改变"));
    assert_eq!(prompts.get(), 2);
    let edit = format!(
        "{{\"path\":{path_json},\"edits\":[{{\"oldText\":\"world\",\"newText\":\"done\"}}]}}"
    );
    assert!(tools.execute("edit", &edit).await.contains("文件已改变"));
    let missing = tools
        .execute_recorded("read", r#"{"path":"missing-file-for-test"}"#)
        .await;
    assert!(!missing.success);
    assert_eq!(prompts.get(), 3);
    tools.reset();
    assert!(tools.execute("read", &read).await.ends_with("\n\ndone"));
    assert_eq!(prompts.get(), 4);
    fs::remove_dir_all(dir).expect("清理目录");
}

#[tokio::test]
async fn batch_preserves_order_across_parallel_reads_and_write() {
    let dir = std::env::temp_dir().join(format!(
        "geer-agent-batch-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("创建目录");
    let first = dir.join("first.txt");
    let second = dir.join("second.txt");
    fs::write(&first, "before").expect("准备文件");
    fs::write(&second, "second").expect("准备文件");
    let path1 = serde_json::to_string(&first.to_string_lossy()).expect("编码路径");
    let path2 = serde_json::to_string(&second.to_string_lossy()).expect("编码路径");
    let read1 = format!("{{\"path\":{path1}}}");
    let read2 = format!("{{\"path\":{path2}}}");
    let write = format!("{{\"path\":{path1},\"content\":\"after\"}}");
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("工作目录存在");
    tools.confirm = Box::new(|_| Ok(true));
    let results = tools
        .execute_batch(&[
            ("read", &read1),
            ("read", &read2),
            ("write", &write),
            ("read", &read1),
        ])
        .await;
    let bodies: Vec<_> = results
        .iter()
        .filter_map(|result| result.output.text.split_once("\n\n").map(|(_, body)| body))
        .collect();
    assert_eq!(bodies, ["before", "second", "after"]);
    assert!(results.iter().all(|result| result.output.success));
    assert!(results[2].output.changed);
    fs::remove_dir_all(dir).expect("清理目录");
}

fn metadata(output: &super::ToolOutput) -> serde_json::Value {
    serde_json::from_str(
        output
            .text
            .split_once("\n\n")
            .map_or(output.text.as_str(), |(meta, _)| meta),
    )
    .expect("结果元信息")
}

fn body(output: &super::ToolOutput) -> &str {
    output.text.split_once("\n\n").expect("元信息与正文分隔").1
}

fn query_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("geer-query-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("创建测试目录");
    dir
}

#[tokio::test]
async fn invalid_parameters_fail_before_authorization_with_field_and_hint() {
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.confirm = Box::new(|_| panic!("非法参数不得请求授权"));
    for (name, args, field) in [
        ("read", r#"{"path":"a","offset":0}"#, "offset"),
        ("read", r#"{"path":"a","offset":1.5}"#, "offset"),
        ("read", r#"{"path":"a","limit":null}"#, "limit"),
        ("read", r#"{"path":"a","limit":"2"}"#, "limit"),
        ("read", r#"{"path":"a","limit":-1}"#, "limit"),
        ("read", r#"{"path":"a","extra":true}"#, "extra"),
        ("write", r#"{"path":"a","content":null}"#, "content"),
        ("edit", r#"{"path":"a","edits":[]}"#, "edits"),
        (
            "edit",
            r#"{"path":"a","edits":[{"oldText":"","newText":"b"}]}"#,
            "edits[0].oldText",
        ),
        (
            "edit",
            r#"{"path":"a","edits":[{"oldText":"a","newText":null}]}"#,
            "edits[0].newText",
        ),
        (
            "edit",
            r#"{"path":"a","edits":[{"oldText":"a","newText":"b","extra":1}]}"#,
            "edits[0].extra",
        ),
        ("edit", r#"{"path":"a","edits":[null]}"#, "edits[0]."),
        ("ls", r#"{"path":"  "}"#, "path"),
        ("ls", r#"{"path":null}"#, "path"),
        ("ls", r#"{"path":4}"#, "path"),
        ("ls", r#"{"limit":2}"#, "limit"),
        ("glob", r#"{"pattern":""}"#, "pattern"),
        ("glob", r#"{"pattern":null}"#, "pattern"),
        ("glob", r#"{}"#, "pattern"),
        ("rg", r#"{"pattern":"a","glob":null}"#, "glob"),
        ("rg", r#"{"pattern":"a","output":null}"#, "output"),
        ("rg", r#"{"pattern":"a","output":"paths"}"#, "output"),
        (
            "rg",
            r#"{"pattern":"a","fixed_strings":1}"#,
            "fixed_strings",
        ),
        ("rg", r#"{"pattern":"a","args":"--hidden"}"#, "args"),
        ("rg", r#"{"pattern":"\u0000"}"#, "pattern"),
        ("read", r#"{"path":"a\u0000b"}"#, "path"),
    ] {
        let result = tools.execute_recorded(name, args).await;
        assert!(!result.success, "{name} {args}");
        assert!(!result.changed);
        let meta = metadata(&result);
        assert_eq!(meta["code"], "invalid_argument", "{name} {args}");
        assert_eq!(meta["field"], field, "{name} {args}");
        assert!(!meta["hint"].as_str().expect("修复建议").is_empty());
        assert_eq!(result.text, tools.execute_recorded(name, args).await.text);
    }
    tools.confirm = Box::new(|_| Ok(false));
    let spaces = tools.execute_recorded("rg", r#"{"pattern":"  "}"#).await;
    assert_eq!(metadata(&spaces)["code"], "authorization_denied");
}

#[tokio::test]
async fn real_queries_then_read_edit_and_verify_in_isolated_directory() {
    use serde_json::json;
    let dir = query_dir();
    let nested = dir.join("dir ' \" $(touch INJECTED) `touch ALSO_INJECTED`");
    fs::create_dir_all(&nested).expect("特殊路径");
    let path = nested.join("--sample.txt");
    let original =
        "alpha same\nbeta same\nneedle 42\n--flag $(touch INJECTED) `touch ALSO_INJECTED` ' \"\n";
    fs::write(&path, original).expect("准备文件");
    fs::write(nested.join("other.rs"), "needle 99\n").expect("非文本扩展名");
    fs::write(nested.join(".hidden"), "hidden").expect("隐藏文件");
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.cwd = dir.clone();
    tools.allow_all_for_test();
    let listing = tools.execute_recorded("ls", "{}").await;
    assert!(listing.success, "{}", listing.text);
    assert!(body(&listing).contains("/\n"));
    let listing = tools
        .execute_recorded("ls", &json!({"path":nested}).to_string())
        .await;
    assert!(body(&listing).contains(".hidden"));
    assert_eq!(metadata(&listing)["path"], json!(nested));
    let glob = tools
        .execute_recorded(
            "glob",
            &json!({"path":nested,"pattern":"--*.txt"}).to_string(),
        )
        .await;
    assert!(glob.success, "{}", glob.text);
    assert_eq!(body(&glob), "./--sample.txt\n");
    assert_eq!(metadata(&glob)["path_base"], json!(nested));
    let args = json!({"path":nested,"pattern":"^needle [0-9]+$","glob":"*.txt"});
    let search = tools.execute_recorded("rg", &args.to_string()).await;
    assert!(search.success, "{}", search.text);
    assert!(body(&search).contains("--sample.txt:3:needle 42"));
    assert!(!body(&search).contains("other.rs:1:needle 99"));
    let files = tools
        .execute_recorded(
            "rg",
            &json!({"path":nested,"pattern":"same","glob":"*.txt","output":"files"}).to_string(),
        )
        .await;
    assert!(files.success, "{}", files.text);
    assert_eq!(
        body(&files),
        format!("{}\n", crate::config::bash_arg(path.to_str().unwrap()))
    );
    let literal = tools.execute_recorded("rg", &json!({"path":path,"pattern":"--flag $(touch INJECTED) `touch ALSO_INJECTED` ' \"","fixed_strings":true}).to_string()).await;
    assert!(literal.success, "{}", literal.text);
    assert!(body(&literal).contains(":4:--flag $(touch INJECTED)"));
    assert!(!dir.join("INJECTED").exists());
    assert!(!dir.join("ALSO_INJECTED").exists());
    let read_args = json!({"path":path});
    let read = tools.execute_recorded("read", &read_args.to_string()).await;
    assert_eq!(body(&read), original);
    let ambiguous = tools
        .execute_recorded(
            "edit",
            &json!({"path":path,"edits":[{"oldText":"same","newText":"changed"}]}).to_string(),
        )
        .await;
    assert_eq!(metadata(&ambiguous)["code"], "ambiguous_old_text");
    assert!(
        metadata(&ambiguous)["hint"]
            .as_str()
            .expect("建议")
            .contains("前后文")
    );
    let edited = tools
        .execute_recorded(
            "edit",
            &json!({"path":path,"edits":[{"oldText":"beta same","newText":"beta changed"}]})
                .to_string(),
        )
        .await;
    assert!(edited.success && edited.changed);
    let after = tools.execute_recorded("read", &read_args.to_string()).await;
    assert_eq!(body(&after), original.replace("beta same", "beta changed"));
    assert!(!tools.grants.contains("bash"));
    fs::remove_dir_all(dir).expect("清理");
}

#[tokio::test]
async fn query_arguments_reach_rg_without_shell_rewriting() {
    use serde_json::json;
    let dir = query_dir();
    fs::write(dir.join("routes.cfg"), "GET /api/v1/users\n").expect("路由文件");
    fs::write(dir.join("b.txt"), "x\n").expect("文本文件");
    fs::write(dir.join("a.txt"), "x\n").expect("文本文件");
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.cwd = dir.clone();
    tools.allow_all_for_test();
    let slash = tools
        .execute_recorded(
            "rg",
            &json!({"pattern":"/api/v1","fixed_strings":true}).to_string(),
        )
        .await;
    assert!(slash.success, "{}", slash.text);
    assert!(
        body(&slash).contains(":1:GET /api/v1/users"),
        "{}",
        slash.text
    );
    let glob = tools
        .execute_recorded("glob", r#"{"pattern":"*.txt"}"#)
        .await;
    assert!(glob.success, "{}", glob.text);
    assert_eq!(body(&glob), "./a.txt\n./b.txt\n");
    fs::remove_dir_all(dir).expect("清理");
}

#[cfg(windows)]
#[test]
fn windows_paths_accept_git_bash_and_verbatim_forms() {
    use super::file::resolve_path;
    let cwd = std::path::Path::new(r"F:\repo");
    let resolved = |raw| resolve_path(cwd, raw).expect("路径有效");
    assert_eq!(
        resolved("/f/repo/src/a.rs"),
        std::path::Path::new(r"F:\repo\src\a.rs")
    );
    assert_eq!(
        resolved("src/a.rs"),
        std::path::Path::new(r"F:\repo\src\a.rs")
    );
    assert_eq!(
        resolved("F:/other/b.rs"),
        std::path::Path::new(r"F:\other\b.rs")
    );
    assert_eq!(
        resolved(r"\\?\F:\x\c.rs"),
        std::path::Path::new(r"F:\x\c.rs")
    );
}

#[tokio::test]
async fn query_empty_errors_ignore_rules_and_truncation_are_distinct() {
    use serde_json::json;
    let dir = query_dir();
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.cwd = dir.clone();
    tools.allow_all_for_test();
    let empty = tools.execute_recorded("ls", "{}").await;
    assert!(empty.success);
    assert!(body(&empty).is_empty());
    for name in ["glob", "rg"] {
        let empty = tools
            .execute_recorded(name, r#"{"pattern":"NOT_FOUND"}"#)
            .await;
        assert!(empty.success, "{}", empty.text);
        assert!(body(&empty).is_empty());
        assert_eq!(metadata(&empty)["matches"], 0);
        assert_eq!(metadata(&empty)["exit_code"], 1);
        assert_eq!(metadata(&empty)["truncated"], false);
    }
    fs::write(dir.join("file.txt"), "file\n").expect("文件");
    for name in ["ls", "glob"] {
        for root in ["file.txt", "missing"] {
            let mut args = json!({"path":root});
            if name == "glob" {
                args["pattern"] = json!("*");
            }
            let bad = tools.execute_recorded(name, &args.to_string()).await;
            assert!(!bad.success, "{}", bad.text);
            assert_eq!(metadata(&bad)["code"], "invalid_search_root");
            assert_eq!(metadata(&bad)["exit_code"], 125);
        }
    }
    for name in ["glob", "rg"] {
        let bad = tools.execute_recorded(name, r#"{"pattern":"["}"#).await;
        assert!(!bad.success, "{}", bad.text);
        assert_eq!(metadata(&bad)["exit_code"], 2);
        assert!(body(&bad).contains("error"));
    }
    fs::write(dir.join(".ignore"), "ignored.txt\n").expect("忽略规则");
    fs::write(dir.join("ignored.txt"), "needle\n").expect("被忽略文件");
    let ignored = tools
        .execute_recorded("rg", r#"{"pattern":"needle"}"#)
        .await;
    assert_eq!(metadata(&ignored)["matches"], 0);
    let explicit = tools
        .execute_recorded("glob", r#"{"pattern":"ignored.txt"}"#)
        .await;
    assert!(body(&explicit).contains("ignored.txt"));
    for index in 0..160 {
        fs::write(
            dir.join(format!("long-file-name-{index:04}.txt")),
            "needle\n",
        )
        .expect("大量文件");
    }
    for (name, args) in [
        ("ls", "{}"),
        ("glob", r#"{"pattern":"*.txt"}"#),
        ("rg", r#"{"pattern":"needle"}"#),
    ] {
        let result = tools.execute_recorded(name, args).await;
        assert!(result.success, "{}", result.text);
        assert!(result.text.chars().count() <= 2000);
        assert_eq!(
            metadata(&result)["truncated"],
            true,
            "{name}: {}",
            result.text
        );
        assert!(result.text.contains("缩小"));
        assert!(metadata(&result)["next_offset"].is_null());
    }
    fs::remove_dir_all(dir).expect("清理");
}

#[tokio::test]
async fn queries_have_separate_grants_and_batch_respects_mutation_barriers() {
    use serde_json::json;
    let dir = query_dir();
    fs::write(dir.join("a.txt"), "before").expect("文件");
    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.cwd = dir.clone();
    let prompts = Rc::new(Cell::new(0));
    let seen = Rc::clone(&prompts);
    tools.confirm = Box::new(move |prompt| {
        assert!(!prompt.contains("任意 Bash 命令"));
        seen.set(seen.get() + 1);
        Ok(true)
    });
    let result = tools
        .execute_batch(&[
            ("ls", "{}"),
            ("glob", r#"{"pattern":"*.txt"}"#),
            ("rg", r#"{"pattern":"before"}"#),
            ("read", r#"{"path":"a.txt"}"#),
            ("write", r#"{"path":"a.txt","content":"after"}"#),
            ("rg", r#"{"pattern":"after"}"#),
            ("read", r#"{"path":"a.txt"}"#),
            (
                "edit",
                r#"{"path":"a.txt","edits":[{"oldText":"after","newText":"final"}]}"#,
            ),
            ("read", r#"{"path":"a.txt"}"#),
        ])
        .await;
    assert!(result.iter().all(|r| r.output.success), "{result:?}");
    let names: Vec<_> = result
        .iter()
        .map(|r| {
            metadata(&r.output)["tool"]
                .as_str()
                .expect("工具名")
                .to_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "ls", "glob", "rg", "read", "write", "rg", "read", "edit", "read"
        ]
    );
    assert_eq!(body(&result[3].output), "before");
    assert!(body(&result[5].output).contains(":1:after"));
    assert_eq!(body(&result[6].output), "after");
    assert_eq!(body(&result[8].output), "final");
    assert_eq!(prompts.get(), 6);
    assert_eq!(tools.grants.len(), 6);
    assert!(!tools.grants.contains("bash"));
    tools.reset();
    for (name, args) in [
        ("ls", "{}"),
        ("glob", r#"{"pattern":"*.txt"}"#),
        ("rg", r#"{"pattern":"final"}"#),
    ] {
        assert!(tools.execute_recorded(name, args).await.success);
    }
    assert_eq!(prompts.get(), 9);
    tools.confirm = Box::new(|_| Ok(false));
    assert!(
        !tools
            .execute_recorded("bash", r#"{"command":"touch should-not-exist"}"#)
            .await
            .success
    );
    assert!(!dir.join("should-not-exist").exists());
    tools.reset();
    let denied = tools.execute_recorded("rg", r#"{"pattern":"final"}"#).await;
    assert_eq!(metadata(&denied)["code"], "authorization_denied");
    assert!(tools.grants.is_empty());
    tools.enabled = false;
    assert!(tools.specs().is_empty());
    for name in ["ls", "glob", "rg", "read", "write", "edit"] {
        let denied = tools.execute_recorded(name, &json!({}).to_string()).await;
        assert!(!denied.success);
        assert_eq!(denied.text, "工具已关闭。");
    }
    fs::remove_dir_all(dir).expect("清理");
}

#[tokio::test]
async fn explicit_workspace_drives_bash_file_and_default_query_paths() {
    let root = query_dir();
    let workspace_a = root.join("workspace A");
    let workspace_b = root.join("工作区 B");
    fs::create_dir_all(&workspace_a).expect("创建 A");
    fs::create_dir_all(&workspace_b).expect("创建 B");
    fs::write(workspace_a.join("marker.txt"), "alpha-only").expect("写入 A");
    fs::write(workspace_b.join("marker.txt"), "beta-only").expect("写入 B");
    fs::write(workspace_a.join("only-a.txt"), "A").expect("写入 A 列表文件");
    fs::write(workspace_b.join("only-b.txt"), "B").expect("写入 B 列表文件");

    let mut tools = Tools::new(true, crate::config::default_bash_bin()).expect("初始化");
    tools.allow_all_for_test();
    let read_a = tools
        .execute_recorded_in(&workspace_a, "read", r#"{"path":"marker.txt"}"#)
        .await;
    let read_b = tools
        .execute_recorded_in(&workspace_b, "read", r#"{"path":"marker.txt"}"#)
        .await;
    assert_eq!(body(&read_a), "alpha-only");
    assert_eq!(body(&read_b), "beta-only");

    let absolute = tools
        .execute_recorded_in(
            &workspace_b,
            "read",
            &serde_json::json!({"path":workspace_a.join("marker.txt")}).to_string(),
        )
        .await;
    assert_eq!(body(&absolute), "alpha-only");
    let quoted = tools
        .execute_recorded_in(
            &workspace_b,
            "read",
            &serde_json::json!({"path":"\"marker.txt\""}).to_string(),
        )
        .await;
    assert_eq!(body(&quoted), "beta-only");
    let missing = tools
        .execute_recorded_in(&workspace_b, "read", r#"{"path":"only-a.txt"}"#)
        .await;
    assert!(!missing.success);
    assert_eq!(metadata(&missing)["code"], "not_found");
    assert_eq!(
        metadata(&missing)["path"],
        serde_json::json!(workspace_b.join("only-a.txt"))
    );
    assert!(
        metadata(&missing)["message"]
            .as_str()
            .unwrap()
            .contains("未获得文件正文")
    );
    assert!(metadata(&missing)["empty"].is_null());

    let bash = tools
        .execute_recorded_in(&workspace_b, "bash", r#"{"command":"pwd; cat marker.txt"}"#)
        .await;
    assert!(bash.success);
    // Git Bash 会把 Windows 临时目录挂载成 /tmp，只比较最后两级目录名。
    let tail = format!("{}/工作区 B", root.file_name().unwrap().to_string_lossy());
    assert!(bash.text.contains(&tail), "{}", bash.text);
    assert!(bash.text.contains("beta-only"));

    let listing = tools.execute_recorded_in(&workspace_b, "ls", "{}").await;
    assert!(listing.text.contains("only-b.txt"));
    assert!(!listing.text.contains("only-a.txt"));
    let search = tools
        .execute_recorded_in(&workspace_a, "rg", r#"{"pattern":"alpha-only"}"#)
        .await;
    assert!(search.success);
    assert!(search.text.contains("marker.txt:1:alpha-only"));
    assert!(!search.text.contains("beta-only"));
    fs::remove_dir_all(root).expect("清理");
}

#[cfg(unix)]
#[tokio::test]
async fn query_dependencies_are_resolved_by_configured_bash_only() {
    use crate::test_support;
    use std::{os::unix::fs::PermissionsExt, time::Duration};
    let dir = query_dir();
    let mut bash = test_support::command("bash");
    bash.args(["--noprofile", "--norc", "-c", "command -v bash"]);
    let bash = test_support::run(&mut bash, &[]).expect("Bash 可用");
    assert!(bash.status.success(), "Bash 路径探针应成功");
    let bash_path = String::from_utf8(bash.stdout).expect("路径编码");
    assert!(std::path::Path::new(bash_path.trim()).is_absolute());
    let wrapper = dir.join(format!("bash-{}", uuid::Uuid::new_v4()));
    let empty_path = dir.join("empty-path");
    fs::create_dir(&empty_path).expect("创建空 PATH 目录");
    // Fedora 的缺失命令钩子可能递归 fork；空 PATH 必须配合禁用启动文件与远程检测。
    // 仅删除启动变量，保留工具通过环境传入的脚本与参数。
    let quoted = format!("'{}'", bash_path.trim().replace('\'', "'\\''"));
    let quoted_path = format!(
        "'{}'",
        empty_path.display().to_string().replace('\'', "'\\''")
    );
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nunset BASH_ENV ENV SSH_CLIENT SSH_CONNECTION SSH_TTY\nPATH={quoted_path}\nexport PATH\nexec {quoted} --noprofile --norc \"$@\"\n"
        ),
    )
    .expect("隔离环境");
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).expect("执行权限");
    let startup = dir.join("startup-hook");
    fs::write(&startup, "printf 'startup-hook-ran' >&2; exit 88\n").expect("启动钩子夹具");
    fs::write(dir.join(".bashrc"), "printf 'bashrc-ran' >&2; exit 89\n").expect("远程启动夹具");
    let mut probe = test_support::command(&wrapper);
    probe
        .env("HOME", &dir)
        .env("BASH_ENV", &startup)
        .env("ENV", &startup)
        .env("SSH_CLIENT", "127.0.0.1 1234 22")
        .env("SSH_CONNECTION", "127.0.0.1 1234 127.0.0.1 22")
        .env("SSH_TTY", "/dev/pts/test")
        .env("SHLVL", "0")
        .env("GEER_AGENT_SCRIPT", "internal script")
        .env("GEER_AGENT_ARG_1", "literal argument")
        .args([
            "-c",
            r#"printf '%s\n' "$GEER_AGENT_SCRIPT" "$GEER_AGENT_ARG_1"; if read -r input; then exit 90; fi; exec ls -1Ap -- ."#,
        ]);
    let probe = test_support::run_with_timeout(&mut probe, &[], Duration::from_secs(2))
        .expect("隔离夹具应快速结束");
    assert_eq!(probe.status.code(), Some(127), "缺少命令应返回 127");
    assert_eq!(probe.stdout, b"internal script\nliteral argument\n");
    assert!(!String::from_utf8_lossy(&probe.stderr).contains("-ran"));
    let mut tools = Tools::new(true, wrapper).expect("初始化");
    tools.cwd = dir.clone();
    tools.allow_all_for_test();
    for (name, args) in [
        ("ls", "{}"),
        ("glob", r#"{"pattern":"*"}"#),
        ("rg", r#"{"pattern":"x"}"#),
        ("search", r#"{"pattern":"x"}"#),
    ] {
        let result = tools.execute_recorded(name, args).await;
        assert!(!result.success);
        assert_eq!(metadata(&result)["code"], "command_unavailable");
        assert_eq!(metadata(&result)["exit_code"], 127);
    }
    fs::write(dir.join("file.txt"), "still works").expect("准备文件");
    assert!(
        tools
            .execute_recorded("read", r#"{"path":"file.txt"}"#)
            .await
            .success
    );
    tools.bash_bin = dir.join("missing-bash");
    let result = tools.execute_recorded("ls", "{}").await;
    assert_eq!(metadata(&result)["code"], "command_unavailable");
    assert!(metadata(&result)["exit_code"].is_null());
    fs::remove_dir_all(dir).expect("清理");
}
