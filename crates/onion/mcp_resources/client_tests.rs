//! `client` 模块单元测试（经 `#[path]` 挂载为兄弟测试模块）。

use super::{McpServerConfig, discover_servers, load_servers};
use crate::credentials::CredentialStore;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ava_mcp_discovery_{}_{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// 在临时根写入凭据文件并返回存储（模板解析测试用）。
fn store_with(root: &std::path::Path, refs: &[(&str, &str)]) -> CredentialStore {
    std::fs::create_dir_all(root).expect("create credential root");
    let mut body = String::from("{\"version\":1,\"refs\":{");
    for (idx, (name, value)) in refs.iter().enumerate() {
        if idx > 0 {
            body.push(',');
        }
        body.push_str(&format!("\"{name}\": \"{value}\""));
    }
    body.push_str("}}");
    std::fs::write(crate::paths::credentials_file(root), body).expect("write credentials");
    CredentialStore::new(root)
}

fn write_config(path: &std::path::Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create config dir");
    }
    std::fs::write(path, body).expect("write config");
}

fn server_names(servers: &[McpServerConfig]) -> Vec<String> {
    servers.iter().map(|s| s.server_name().to_string()).collect()
}

#[test]
fn missing_files_are_silently_skipped() {
    let root = temp_dir("missing");
    let (servers, notes) = discover_servers(&root, &root, None);
    assert!(servers.is_empty());
    assert!(notes.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn global_then_workspace_then_explicit_merge() {
    let root = temp_dir("merge");
    let global = root.join("global");
    let workspace = root.join("workspace");
    write_config(
        &global.join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"cmd-a"},{"transport":"stdio","serverName":"b","command":"cmd-b"}]}"#,
    );
    write_config(
        &workspace.join(".onion").join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"c","command":"cmd-c"}]}"#,
    );
    let (servers, notes) = discover_servers(&workspace, &global, None);
    assert_eq!(server_names(&servers), vec!["a", "b", "c"]);
    assert!(notes.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn later_file_overrides_same_server_name() {
    let root = temp_dir("override");
    let global = root.join("global");
    let workspace = root.join("workspace");
    write_config(
        &global.join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"global-a"},{"transport":"stdio","serverName":"b","command":"global-b"}]}"#,
    );
    write_config(
        &workspace.join(".onion").join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"workspace-a"}]}"#,
    );
    let (servers, notes) = discover_servers(&workspace, &global, None);
    assert_eq!(server_names(&servers), vec!["a", "b"]);
    let McpServerConfig::Stdio { command, .. } = &servers[0] else {
        panic!("expected stdio server");
    };
    assert_eq!(command, "workspace-a");
    assert_eq!(notes.len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn explicit_path_highest_priority() {
    let root = temp_dir("explicit");
    let global = root.join("global");
    write_config(
        &global.join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"global-a"}]}"#,
    );
    let explicit = root.join("custom.json");
    write_config(
        &explicit,
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"explicit-a"}]}"#,
    );
    let (servers, _) = discover_servers(&root, &global, Some(&explicit));
    let McpServerConfig::Stdio { command, .. } = &servers[0] else {
        panic!("expected stdio server");
    };
    assert_eq!(command, "explicit-a");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn malformed_file_is_collected_not_fatal() {
    let root = temp_dir("malformed");
    let global = root.join("global");
    write_config(
        &global.join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"global-a"}]}"#,
    );
    let workspace = root.join("workspace");
    write_config(&workspace.join(".onion").join("mcp.json"), "not json");
    let (servers, notes) = discover_servers(&workspace, &global, None);
    // 坏文件不影响好文件：a 仍加载成功。
    assert_eq!(server_names(&servers), vec!["a"]);
    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("MCP 配置文件加载失败"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn duplicate_paths_load_once() {
    let root = temp_dir("duplicate");
    let global = root.join("global");
    write_config(
        &global.join("mcp.json"),
        r#"{"servers":[{"transport":"stdio","serverName":"a","command":"global-a"}]}"#,
    );
    // 显式路径与全局重合：同一文件只加载一次（a 只注册一份）。
    let (servers, notes) = discover_servers(&root, &global, Some(&global.join("mcp.json")));
    assert_eq!(server_names(&servers), vec!["a"]);
    assert!(notes.is_empty());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn load_servers_requires_servers_array() {
    let root = temp_dir("array");
    let path = root.join("mcp.json");
    write_config(&path, r#"{"servers":[]}"#);
    assert!(load_servers(&path).unwrap().is_empty());
    write_config(&path, r#"{}"#);
    assert!(load_servers(&path).is_err());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn parse_server_stdio_and_http() {
    let stdio = serde_json::json!({
        "transport": "stdio",
        "serverName": "mem",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-memory"],
        "env": {"KEY": "v"},
        "cwd": "/tmp",
        "toolCallTimeoutMs": 1234
    });
    let McpServerConfig::Stdio { server_name, command, args, env, cwd, tool_call_timeout_ms } =
        super::parse_server(&stdio).unwrap()
    else {
        panic!("expected stdio server");
    };
    assert_eq!(server_name, "mem");
    assert_eq!(command, "npx");
    assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-memory"]);
    assert_eq!(env.get("KEY").map(String::as_str), Some("v"));
    assert_eq!(cwd, Some(std::path::PathBuf::from("/tmp")));
    assert_eq!(tool_call_timeout_ms, 1234);

    let http = serde_json::json!({
        "transport": "streamable-http",
        "serverName": "remote",
        "url": "https://example.com/mcp",
        "headers": {"Authorization": "Bearer x"},
        "toolCallTimeoutMs": 4321
    });
    let McpServerConfig::StreamableHttp { server_name, url, headers, tool_call_timeout_ms } =
        super::parse_server(&http).unwrap()
    else {
        panic!("expected http server");
    };
    assert_eq!(server_name, "remote");
    assert_eq!(url, "https://example.com/mcp");
    assert_eq!(headers.get("Authorization").map(String::as_str), Some("Bearer x"));
    assert_eq!(tool_call_timeout_ms, 4321);
}

#[test]
fn resolve_credentials_expands_stdio_and_http_fields() {
    let root = temp_dir("template");
    let store = store_with(&root, &[("MEM_KEY", "sk-secret"), ("TOKEN", "tok-1234567890")]);

    let stdio = McpServerConfig::Stdio {
        server_name: "mem".to_string(),
        command: "npx".to_string(),
        args: vec!["-y".to_string(), "--key=${credential:MEM_KEY}".to_string()],
        env: std::iter::once(("API_KEY".to_string(), "${credential:MEM_KEY}".to_string())).collect(),
        cwd: Some(std::path::PathBuf::from("${credential:TOKEN}-dir")),
        tool_call_timeout_ms: 60_000,
    };
    let McpServerConfig::Stdio { command, args, env, cwd, .. } =
        stdio.resolve_credentials(&store).unwrap()
    else {
        panic!("expected stdio server");
    };
    assert_eq!(command, "npx");
    assert_eq!(args, vec!["-y", "--key=sk-secret"]);
    assert_eq!(env.get("API_KEY").map(String::as_str), Some("sk-secret"));
    assert_eq!(cwd, Some(std::path::PathBuf::from("tok-1234567890-dir")));

    let http = McpServerConfig::StreamableHttp {
        server_name: "remote".to_string(),
        url: "https://${credential:MEM_KEY}@example.com/mcp".to_string(),
        headers: std::iter::once((
            "Authorization".to_string(),
            "Bearer ${credential:TOKEN}".to_string(),
        ))
        .collect(),
        tool_call_timeout_ms: 60_000,
    };
    let McpServerConfig::StreamableHttp { url, headers, .. } =
        http.resolve_credentials(&store).unwrap()
    else {
        panic!("expected http server");
    };
    assert_eq!(url, "https://sk-secret@example.com/mcp");
    assert_eq!(headers.get("Authorization").map(String::as_str), Some("Bearer tok-1234567890"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn resolve_credentials_missing_reference_names_the_credential() {
    let root = temp_dir("missing_ref");
    let store = store_with(&root, &[]);
    let stdio = McpServerConfig::Stdio {
        server_name: "mem".to_string(),
        command: "npx".to_string(),
        args: vec![],
        env: std::iter::once(("API_KEY".to_string(), "${credential:MEM_KEY}".to_string())).collect(),
        cwd: None,
        tool_call_timeout_ms: 60_000,
    };
    let err = stdio.resolve_credentials(&store).unwrap_err();
    assert!(matches!(err, crate::credentials::TemplateError::Missing { ref name } if name == "MEM_KEY"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn resolve_credentials_rejects_malformed_syntax() {
    let root = temp_dir("bad_syntax");
    let store = store_with(&root, &[("MEM_KEY", "sk-secret")]);
    let stdio = McpServerConfig::Stdio {
        server_name: "mem".to_string(),
        command: "echo ${HOME}".to_string(),
        args: vec![],
        env: Default::default(),
        cwd: None,
        tool_call_timeout_ms: 60_000,
    };
    let err = stdio.resolve_credentials(&store).unwrap_err();
    assert!(matches!(err, crate::credentials::TemplateError::Syntax(_)));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn resolve_credentials_passthrough_without_templates() {
    let root = temp_dir("plain");
    let store = store_with(&root, &[]);
    let stdio = McpServerConfig::Stdio {
        server_name: "mem".to_string(),
        command: "npx".to_string(),
        args: vec!["-y".to_string()],
        env: std::iter::once(("KEY".to_string(), "v".to_string())).collect(),
        cwd: None,
        tool_call_timeout_ms: 60_000,
    };
    let McpServerConfig::Stdio { command, args, env, .. } =
        stdio.resolve_credentials(&store).unwrap()
    else {
        panic!("expected stdio server");
    };
    assert_eq!(command, "npx");
    assert_eq!(args, vec!["-y"]);
    assert_eq!(env.get("KEY").map(String::as_str), Some("v"));
    std::fs::remove_dir_all(&root).ok();
}
