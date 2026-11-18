use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atelier-mcp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_command_lines_of_the_known_ais_are_recognized() {
    assert_eq!(detect("claude"), Some(Agent::Claude));
    assert_eq!(detect("  claude --resume  "), Some(Agent::Claude));
    assert_eq!(
        detect(r"C:\Users\moi\AppData\Roaming\npm\claude.cmd -p hi"),
        Some(Agent::Claude)
    );
    assert_eq!(detect("gemini"), Some(Agent::Gemini));
    assert_eq!(detect("codex exec fix"), Some(Agent::Codex));
    assert_eq!(detect("claudette"), None);
    assert_eq!(detect("echo claude"), None);
    assert_eq!(detect(""), None);
}

#[test]
fn claude_gets_the_server_in_mcp_json_and_keeps_what_was_there() {
    let root = scratch("claude");
    fs::write(
        root.join(".mcp.json"),
        r#"{"mcpServers":{"autre":{"command":"x"}}}"#,
    )
    .unwrap();
    assert!(!is_connected(Agent::Claude, &root, None));
    let written = connect(Agent::Claude, &root, None).unwrap();
    assert_eq!(written.len(), 2);
    assert!(is_connected(Agent::Claude, &root, None));
    let file: Value =
        serde_json::from_str(&fs::read_to_string(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(file["mcpServers"]["autre"]["command"], "x");
    assert_eq!(
        file["mcpServers"][SERVER]["url"], "${ATELIER_MCP_URL}",
        "no address or secret in the file"
    );
    assert_eq!(
        file["mcpServers"][SERVER]["headers"]["Authorization"],
        "Bearer ${ATELIER_MCP_TOKEN}"
    );
    let settings: Value = serde_json::from_str(
        &fs::read_to_string(root.join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["enabledMcpjsonServers"], json!([SERVER]));
    // Twice changes nothing more.
    connect(Agent::Claude, &root, None).unwrap();
    let settings: Value = serde_json::from_str(
        &fs::read_to_string(root.join(".claude/settings.local.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(settings["enabledMcpjsonServers"], json!([SERVER]));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn codex_gets_a_toml_entry_with_the_current_address_and_keeps_the_rest() {
    let root = scratch("codex");
    fs::create_dir_all(root.join(".codex")).unwrap();
    fs::write(
        root.join(".codex/config.toml"),
        "# mon réglage\nmodel = \"o3\"\n\n[mcp_servers.autre]\ncommand = \"npx\"\n",
    )
    .unwrap();
    let first = "http://127.0.0.1:4000/mcp";
    assert!(!is_connected(Agent::Codex, &root, Some(first)));
    connect(Agent::Codex, &root, Some(first)).unwrap();
    assert!(is_connected(Agent::Codex, &root, Some(first)));
    let text = fs::read_to_string(root.join(".codex/config.toml")).unwrap();
    assert!(
        text.contains("# mon réglage")
            && text.contains("model = \"o3\"")
            && text.contains("command = \"npx\""),
        "{text}"
    );
    assert!(
        text.contains("bearer_token_env_var = \"ATELIER_MCP_TOKEN\""),
        "the secret is not written: {text}"
    );
    // The port changed: the entry is stale until it is written again.
    let second = "http://127.0.0.1:5000/mcp";
    assert!(!is_connected(Agent::Codex, &root, Some(second)));
    connect(Agent::Codex, &root, Some(second)).unwrap();
    assert!(is_connected(Agent::Codex, &root, Some(second)));
    assert_eq!(
        fs::read_to_string(root.join(".codex/config.toml"))
            .unwrap()
            .matches("[mcp_servers.atelier]")
            .count(),
        1
    );
    assert!(
        connect(Agent::Codex, &root, None).is_err(),
        "no server, no address"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn gemini_gets_its_own_settings_file_and_a_broken_file_is_left_alone() {
    let root = scratch("gemini");
    connect(Agent::Gemini, &root, None).unwrap();
    let file: Value =
        serde_json::from_str(&fs::read_to_string(root.join(".gemini/settings.json")).unwrap())
            .unwrap();
    assert_eq!(file["mcpServers"][SERVER]["httpUrl"], "${ATELIER_MCP_URL}");
    let broken = scratch("broken");
    fs::write(broken.join(".mcp.json"), "{ pas du json").unwrap();
    assert!(connect(Agent::Claude, &broken, None).is_err());
    assert_eq!(
        fs::read_to_string(broken.join(".mcp.json")).unwrap(),
        "{ pas du json",
        "untouched"
    );
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&broken);
}

/// For every project: the server goes in the file of the user, which keeps all it held, and
/// no project gets a file.
#[test]
fn the_server_is_declared_once_for_every_project() {
    let home = scratch("home");
    let project = scratch("home-project");
    fs::write(
        home.join(".claude.json"),
        r#"{"numStartups":7,"mcpServers":{"autre":{"command":"x"}},"projects":{"a":{}}}"#,
    )
    .unwrap();
    assert!(!is_connected_user(Agent::Claude, &home, None));
    let written = connect_user(Agent::Claude, &home, None).unwrap();
    assert_eq!(written, home.join(".claude.json"));
    assert!(is_connected_user(Agent::Claude, &home, None));
    let file: Value = serde_json::from_str(&fs::read_to_string(&written).unwrap()).unwrap();
    assert_eq!(file["numStartups"], 7);
    assert_eq!(file["projects"]["a"], json!({}));
    assert_eq!(file["mcpServers"]["autre"]["command"], "x");
    assert_eq!(file["mcpServers"][SERVER]["url"], "${ATELIER_MCP_URL}");
    assert_eq!(
        file["mcpServers"][SERVER]["headers"]["Authorization"], "Bearer ${ATELIER_MCP_TOKEN}",
        "no secret in the file"
    );
    assert!(!home.join(".claude.json.atelier-tmp").exists());

    connect_user(Agent::Gemini, &home, None).unwrap();
    assert!(is_connected_user(Agent::Gemini, &home, None));
    assert!(home.join(".gemini/settings.json").is_file());

    assert!(
        connect_user(Agent::Codex, &home, None).is_err(),
        "no address yet"
    );
    connect_user(Agent::Codex, &home, Some("http://127.0.0.1:1/mcp")).unwrap();
    assert!(is_connected_user(
        Agent::Codex,
        &home,
        Some("http://127.0.0.1:1/mcp")
    ));

    // Nothing in the project, which does not declare the server itself.
    assert_eq!(fs::read_dir(&project).unwrap().count(), 0);
    assert!(!is_connected(Agent::Claude, &project, None));
    for dir in [home, project] {
        let _ = fs::remove_dir_all(dir);
    }
}

/// A file of the user that is not valid JSON is left as it is.
#[test]
fn a_broken_file_of_the_user_is_not_replaced() {
    let home = scratch("home-broken");
    fs::write(home.join(".claude.json"), "{ not json").unwrap();
    assert!(connect_user(Agent::Claude, &home, None).is_err());
    assert_eq!(
        fs::read_to_string(home.join(".claude.json")).unwrap(),
        "{ not json"
    );
    let _ = fs::remove_dir_all(home);
}
