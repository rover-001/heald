use std::process::Command;
use std::fs;
use std::path::PathBuf;

fn get_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_heald"))
}

#[test]
fn test_blame_one_file() {
    // Before testing, let's create a fake memory document
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();
    fs::write(mem_dir.join("test-doc-1.md"), "---\ntitle: Test Doc 1\ntimestamp: 2026-08-09T00:00:00Z\ntype: decision\n---\nWe decided to use src/auth.rs for auth.\n").unwrap();
    
    let output = Command::new(get_bin())
        .args(["blame", "src/auth.rs"])
        .output()
        .expect("failed to execute");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("src/auth.rs"));
    assert!(stdout.contains("test-doc-1.md"));
    assert!(stdout.contains("We decided to use src/auth.rs for auth."));
}

#[test]
fn test_blame_multiple_docs_ordering() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();
    // Older
    fs::write(mem_dir.join("test-doc-old.md"), "---\ntitle: Old Doc\ntimestamp: 2025-01-01T00:00:00Z\ntype: decision\n---\nRef src/auth.rs older.\n").unwrap();
    // Newer
    fs::write(mem_dir.join("test-doc-new.md"), "---\ntitle: New Doc\ntimestamp: 2026-08-10T00:00:00Z\ntype: decision\n---\nRef src/auth.rs newer.\n").unwrap();

    let output = Command::new(get_bin())
        .args(["blame", "src/auth.rs"])
        .output()
        .expect("failed to execute");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    
    // Check ordering: new should appear before old
    let idx_new = stdout.find("test-doc-new.md").unwrap();
    let idx_old = stdout.find("test-doc-old.md").unwrap();
    assert!(idx_new < idx_old);
}

#[test]
fn test_blame_zero_refs() {
    let output = Command::new(get_bin())
        .args(["blame", "src/nonexistent.rs"])
        .output()
        .expect("failed to execute");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("No decisions on record for src/nonexistent.rs"));
}

#[test]
fn test_blame_directory() {
    // Create another file under src/cmd
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();
    fs::write(mem_dir.join("test-doc-cmd.md"), "---\ntimestamp: 2026-08-10T00:00:00Z\ntype: decision\n---\nWe touch src/cmd/map.rs here.\n").unwrap();

    let output = Command::new(get_bin())
        .args(["blame", "src/cmd"])
        .output()
        .expect("failed to execute");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(stdout.contains("src/cmd/map.rs"));
    assert!(stdout.contains("test-doc-cmd.md"));
}

#[test]
fn test_blame_json() {
    let output = Command::new(get_bin())
        .args(["blame", "src/auth.rs", "--json"])
        .output()
        .expect("failed to execute");
    
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    // Should be parseable JSON
    let val: serde_json::Value = serde_json::from_str(&stdout).expect("Valid JSON");
    assert!(val.is_object());
    let obj = val.as_object().unwrap();
    assert!(obj.contains_key("src/auth.rs"));
    assert!(obj["src/auth.rs"].as_array().unwrap().len() >= 2);
}

#[test]
fn test_map_against_repo() {
    // Run map
    let output = Command::new(get_bin())
        .args(["map"])
        .output()
        .expect("failed to execute");
    
    assert!(output.status.success());

    // Read the map file
    let map_file = PathBuf::from(".heald/map.md");
    assert!(map_file.exists());
    let content = fs::read_to_string(map_file).unwrap();

    // Check token cap: 8000 chars should be safely under ~2000 tokens. 
    // Wait, the code caps around 6000 chars and then truncates.
    assert!(content.len() <= 6500, "Map is too large: {} bytes", content.len());

    // Must include at least one cross-referenced memory doc.
    assert!(content.contains("see "), "Must contain cross reference");
}

#[test]
fn test_context_and_memory_index() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();
    fs::write(
        mem_dir.join("test-auto-index.md"),
        "---\ntitle: Auto Index Test\ntimestamp: 2026-08-15T00:00:00Z\ntype: decision\ntags: [pinned]\n---\nAuto generated index test content.\n",
    ).unwrap();

    let output = Command::new(get_bin())
        .args(["context", "agents"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Project Memory Manifest"));
    assert!(stdout.contains("Auto Index Test"));
    assert!(stdout.contains("test-auto-index.md"));

    let index_file = mem_dir.join("index.md");
    assert!(index_file.exists());
    let index_content = fs::read_to_string(index_file).unwrap();
    assert!(index_content.contains("Project Memory Manifest"));
    assert!(index_content.contains("Auto Index Test"));
}

#[test]
fn test_skill_commands_lifecycle() {
    let output_list = Command::new(get_bin())
        .args(["skill", "list"])
        .output()
        .expect("failed to execute");
    assert!(output_list.status.success());

    // Test install a skill locally via file
    let temp_skill_file = PathBuf::from("temp_test_skill.md");
    let test_skill_content = "---\ntype: skill\ntitle: test-search-skill\ndescription: A test skill for search\ntriggers: [searching, testing-search]\n---\nTest skill body content for testing.\n";
    fs::write(&temp_skill_file, test_skill_content).unwrap();

    let output_install = Command::new(get_bin())
        .args(["skill", "install", temp_skill_file.to_str().unwrap(), "--name", "test-search-skill"])
        .output()
        .expect("failed to execute");
    let _ = fs::remove_file(&temp_skill_file);
    assert!(output_install.status.success());

    // Test search
    let output_search = Command::new(get_bin())
        .args(["skill", "search", "testing-search"])
        .output()
        .expect("failed to execute");
    assert!(output_search.status.success());
    let search_stdout = String::from_utf8(output_search.stdout).unwrap();
    assert!(search_stdout.contains("test-search-skill"));

    // Test info
    let output_info = Command::new(get_bin())
        .args(["skill", "info", "test-search-skill"])
        .output()
        .expect("failed to execute");
    assert!(output_info.status.success());
    let info_stdout = String::from_utf8(output_info.stdout).unwrap();
    assert!(info_stdout.contains("Test skill body content for testing."));
}

#[test]
fn test_mcp_server_stdio_interaction() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(get_bin())
        .args(["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to start mcp server");

    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });

    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });

    let remember_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "heald_remember",
            "arguments": {
                "type": "decision",
                "title": "MCP Stdio Test Memory",
                "body": "Memory written via MCP test suite"
            }
        }
    });

    let recall_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "heald_recall",
            "arguments": {
                "query": "MCP Stdio Test",
                "compact": true
            }
        }
    });

    let finalize_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "heald_finalize",
            "arguments": {
                "summary": "Completed session via MCP tool call."
            }
        }
    });

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
        writeln!(stdin, "{}", serde_json::to_string(&list_req).unwrap()).unwrap();
        writeln!(stdin, "{}", serde_json::to_string(&remember_req).unwrap()).unwrap();
        writeln!(stdin, "{}", serde_json::to_string(&recall_req).unwrap()).unwrap();
        writeln!(stdin, "{}", serde_json::to_string(&finalize_req).unwrap()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read output");
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("\"protocolVersion\":\"2024-11-05\""));
    assert!(stdout.contains("heald_recall"));
    assert!(stdout.contains("heald_remember"));
    assert!(stdout.contains("heald_forget"));
    assert!(stdout.contains("heald_map"));
    assert!(stdout.contains("heald_blame"));
    assert!(stdout.contains("heald_doctor"));
    assert!(stdout.contains("heald_finalize"));
    assert!(stdout.contains("Saved memory to"));
    assert!(stdout.contains("MCP Stdio Test Memory"));
    assert!(stdout.contains("Session finalized and logged successfully"));
}

#[test]
fn test_context_compact_mode() {
    let output = Command::new(get_bin())
        .args(["context", "agents", "--compact"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("### Memory Index") || stdout.contains("###"));
    assert!(stdout.contains("<!-- Heald Context: Included"));
}

#[test]
fn test_context_relevant_query_bm25() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();

    fs::write(
        mem_dir.join("jwt-auth-decision.md"),
        "---\ntitle: JWT Auth Decision\ntimestamp: 2026-08-10T00:00:00Z\ntype: decision\n---\nWe implemented JWT authentication and token refresh policies.\n",
    ).unwrap();

    fs::write(
        mem_dir.join("database-migration-decision.md"),
        "---\ntitle: Database Migration Decision\ntimestamp: 2026-08-11T00:00:00Z\ntype: decision\n---\nPostgreSQL migrations and schema index configuration.\n",
    ).unwrap();

    // Query for auth should rank jwt-auth-decision above database-migration-decision
    let output = Command::new(get_bin())
        .args(["context", "agents", "--relevant", "authentication token refresh"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("# JWT Auth Decision"));
    assert!(stdout.contains("# Database Migration Decision"));
    let idx_auth = stdout.find("# JWT Auth Decision").unwrap();
    let idx_db = stdout.find("# Database Migration Decision").unwrap();
    assert!(idx_auth < idx_db, "Relevant document heading must appear before unrelated document heading");

}

#[test]
fn test_compact_command_dedup_and_archive() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();

    // Add duplicate memory files with same effective title but different timestamps
    fs::write(
        mem_dir.join("duplicate-test-v1.md"),
        "---\ntitle: Unique Feature Test\ntimestamp: 2025-01-01T00:00:00Z\ntype: decision\n---\nOlder version.\n",
    ).unwrap();

    fs::write(
        mem_dir.join("duplicate-test-v2.md"),
        "---\ntitle: Unique Feature Test\ntimestamp: 2026-08-20T00:00:00Z\ntype: decision\n---\nNewer version.\n",
    ).unwrap();

    // Add duplicate log sessions to log.md
    let log_path = mem_dir.join("log.md");
    fs::write(
        &log_path,
        "---\ntype: log\n---\n# Memory Log\n\n## Session 2026-08-20T10:00:00Z\n\nWorked on CLI testing\n\n## Session 2026-08-20T11:00:00Z\n\nWorked on CLI testing\n\n## Session 2026-08-20T12:00:00Z\n\nCompleted CLI test suite\n",
    ).unwrap();

    let output = Command::new(get_bin())
        .args(["compact"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Compacting Heald memories and session logs"));
    assert!(stdout.contains("Consolidated 1 log entries") || stdout.contains("duplicates"));

    // Check that archive folder exists and contains duplicate-test-v1.md
    let archive_dir = mem_dir.join("archive");
    assert!(archive_dir.exists());
    assert!(archive_dir.join("duplicate-test-v1.md").exists());
    assert!(mem_dir.join("duplicate-test-v2.md").exists());

    // Check deduplicated log.md
    let log_content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(log_content.matches("Worked on CLI testing").count(), 1);
    assert!(log_content.contains("Completed CLI test suite"));
}

#[test]
fn test_doctor_validations() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();

    // Write a memory doc with an orphan file reference
    fs::write(
        mem_dir.join("orphan-ref-doc.md"),
        "---\ntitle: Orphan Ref Doc\ntype: decision\n---\nWe modified non_existent_file_xyz.rs in this task.\n",
    ).unwrap();

    let output = Command::new(get_bin())
        .args(["doctor"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Validating memory documents & cross-references"));
    assert!(stdout.contains("WARNING (Orphan Reference)") || stdout.contains("warning(s)"));
}

#[test]
fn test_clean_clustering_and_pruning() {
    let local_base = PathBuf::from(".heald");
    let mem_dir = local_base.join("memory");
    fs::create_dir_all(&mem_dir).unwrap();

    // Add related memories about OAuth
    fs::write(
        mem_dir.join("instagram-oauth-flow.md"),
        "---\ntitle: Instagram OAuth Flow\ntimestamp: 2026-08-20T00:00:00Z\ntype: decision\n---\nConfigured Instagram OAuth token exchange in src/auth.rs.\n",
    ).unwrap();

    fs::write(
        mem_dir.join("youtube-oauth-refresh.md"),
        "---\ntitle: YouTube OAuth Token Refresh\ntimestamp: 2026-08-21T00:00:00Z\ntype: decision\n---\nAdded YouTube token refresh and connection status.\n",
    ).unwrap();

    // Add a trivial version bump
    fs::write(
        mem_dir.join("bump-app-version-103.md"),
        "---\ntitle: App Release Version 1.0.3\ntimestamp: 2026-08-22T00:00:00Z\ntype: decision\n---\nBumped version in package.json.\n",
    ).unwrap();

    let output = Command::new(get_bin())
        .args(["clean", "--threshold", "2"])
        .output()
        .expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Merging") || stdout.contains("Archived"));

    // Check that consolidated document was created
    let consolidated = mem_dir.join("consolidated-social-integrations-and-oauth.md");
    assert!(consolidated.exists(), "Consolidated doc should exist");

    let content = fs::read_to_string(&consolidated).unwrap();
    assert!(content.contains("Consolidated: Social Integrations & OAuth"));
    assert!(content.contains("Instagram OAuth Flow"));
    assert!(content.contains("YouTube OAuth Token Refresh"));

    // Check archive contains original docs
    let archive_dir = mem_dir.join("archive");
    assert!(archive_dir.join("instagram-oauth-flow.md").exists());
    assert!(archive_dir.join("youtube-oauth-refresh.md").exists());
    assert!(archive_dir.join("bump-app-version-103.md").exists());
}

#[test]
fn test_learn_scan_and_query() {
    // Run learn --scan on current repo
    let output_scan = Command::new(get_bin())
        .args(["learn", "--scan"])
        .output()
        .expect("failed to execute");

    assert!(output_scan.status.success());
    let stdout_scan = String::from_utf8(output_scan.stdout).unwrap();
    assert!(stdout_scan.contains("Learned"));

    // Knowledge files should exist
    assert!(PathBuf::from(".heald/knowledge/features.json").exists());
    assert!(PathBuf::from(".heald/knowledge/feature-map.md").exists());

    // Run learn query
    let output_query = Command::new(get_bin())
        .args(["learn", "clean"])
        .output()
        .expect("failed to execute");

    assert!(output_query.status.success());
    let stdout_query = String::from_utf8(output_query.stdout).unwrap();
    assert!(stdout_query.contains("clean.rs") || stdout_query.contains("run_clean") || stdout_query.contains("Feature"));

    // Test record workflow
    let output_record = Command::new(get_bin())
        .args(["learn", "--record", "--feature", "Testing", "--notes", "Run cargo test before pushing"])
        .output()
        .expect("failed to execute");

    assert!(output_record.status.success());
}



