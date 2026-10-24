use crate::okf::Document;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}


#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub fn run() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&err_resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let id = req.id.unwrap_or(Value::Null);

        match req.method.as_str() {
            "initialize" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "heald",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    })),
                    error: None,
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
            }
            "notifications/initialized" | "initialized" => {
                // Initialized notification, no response required if id is null, but if id is present, acknowledge
                if !id.is_null() {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({})),
                        error: None,
                    };
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                    let _ = stdout.flush();
                }
            }
            "tools/list" => {
                let tools = get_tool_definitions();
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "tools": tools
                    })),
                    error: None,
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                let call_result = handle_tool_call(tool_name, &arguments);
                match call_result {
                    Ok(res_val) => {
                        let resp = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": res_val
                                    }
                                ]
                            })),
                            error: None,
                        };
                        let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                        let _ = stdout.flush();
                    }
                    Err(err_msg) => {
                        let resp = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(json!({
                                "content": [
                                    {
                                        "type": "text",
                                        "text": format!("Error: {}", err_msg)
                                    }
                                ],
                                "isError": true
                            })),
                            error: None,
                        };
                        let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                        let _ = stdout.flush();
                    }
                }
            }
            "ping" => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({})),
                    error: None,
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
            }
            _ => {
                if !id.is_null() {
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {}", req.method),
                            data: None,
                        }),
                    };
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                    let _ = stdout.flush();
                }
            }
        }
    }
}

fn get_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "heald_recall",
            "description": "Retrieve project memories, decisions, and manifest assembled within a token/char budget.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Optional keyword or query to filter memories."
                    },
                    "task": {
                        "type": "string",
                        "description": "Current goal or task description for intent-aware context slicing."
                    },
                    "intent": {
                        "type": "string",
                        "description": "Alternative alias for task/goal intent."
                    },
                    "budget": {
                        "type": "integer",
                        "description": "Maximum character budget (defaults to ~32000 chars / 8000 tokens)."
                    },
                    "compact": {
                        "type": "boolean",
                        "description": "Return ultra-condensed token-efficient summaries instead of full document bodies."
                    },
                    "strict": {
                        "type": "boolean",
                        "description": "Strictly slice context to only matching decisions and drop unrelated documents."
                    }
                }
            }
        }),
        json!({
            "name": "heald_remember",
            "description": "Record an architectural decision, rule, learning, or project context document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "description": "Type of memory: decision, rule, learning, pattern, etc. (default: decision)"
                    },
                    "title": {
                        "type": "string",
                        "description": "Short, descriptive title for the memory."
                    },
                    "body": {
                        "type": "string",
                        "description": "Markdown body detailing what was decided, rationale, or instructions."
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional tags, e.g. ['pinned', 'auth', 'database']."
                    }
                },
                "required": ["title", "body"]
            }
        }),
        json!({
            "name": "heald_forget",
            "description": "Remove or forget an outdated memory document by slug or title match.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Slug or substring match of the memory document title to remove."
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "heald_map",
            "description": "Generate and return a condensed repository structure map with file cross-references to past decisions.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "heald_blame",
            "description": "Find all memory documents and architectural decisions that referenced a specific file or directory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File or directory path to inspect."
                    }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "heald_doctor",
            "description": "Run integrity checks across rules, skills, and memory bundles.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "heald_drift",
            "description": "Scan memory documents for drifted, moved, or deleted code symbols and functions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fix": {
                        "type": "boolean",
                        "description": "Automatically repair moved symbol references in memory docs."
                    }
                }
            }
        }),
        json!({
            "name": "heald_learn",
            "description": "Query or record codebase features, files, functions, and workflow navigation guidance.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Feature, keyword, or function to search." },
                    "scan": { "type": "boolean", "description": "Force rescan of codebase." },
                    "record": { "type": "boolean", "description": "Record a new learning." },
                    "feature": { "type": "string", "description": "Feature name when recording." },
                    "notes": { "type": "string", "description": "Guidance or instructions when recording." }
                }
            }
        }),
        json!({
            "name": "heald_clean",
            "description": "Consolidate related memory documents, merge topic clusters, and prune trivial notes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dry_run": { "type": "boolean", "description": "Preview changes without modifying files." },
                    "threshold": { "type": "integer", "description": "Maximum allowed active memory documents before consolidation." }
                }
            }
        }),
        json!({
            "name": "heald_finalize",
            "description": "Finalize an agent session: append summary to memory log, run auto-learn on modified files, and trigger auto-clean if needed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summary": {
                        "type": "string",
                        "description": "Session summary describing what was accomplished."
                    }
                },
                "required": ["summary"]
            }
        })
    ]
}

pub fn handle_tool_call(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "heald_recall" => {
            let query = args.get("query").and_then(|v| v.as_str());
            let task = args.get("task").and_then(|v| v.as_str());
            let intent = args.get("intent").and_then(|v| v.as_str());
            let budget = args.get("budget").and_then(|v| v.as_u64()).map(|b| b as usize);
            let compact = args.get("compact").and_then(|v| v.as_bool()).unwrap_or(false);
            let strict = args.get("strict").and_then(|v| v.as_bool()).unwrap_or(false);
            let effective_query = task.or(intent).or(query);
            let is_intent = strict || task.is_some() || intent.is_some();
            recall_memory(effective_query, budget, compact, is_intent)
        }
        "heald_remember" => {
            let doc_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("decision");
            let title = args.get("title").and_then(|v| v.as_str()).ok_or("Missing required 'title'")?;
            let body = args.get("body").and_then(|v| v.as_str()).ok_or("Missing required 'body'")?;
            let tags: Option<Vec<String>> = args.get("tags").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect()
            });

            remember_memory(doc_type, title, body, tags.as_deref())
        }
        "heald_forget" => {
            let query = args.get("query").and_then(|v| v.as_str()).ok_or("Missing required 'query'")?;
            forget_memory(query)
        }
        "heald_map" => {
            get_repo_map()
        }
        "heald_blame" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing required 'path'")?;
            blame_path(path)
        }
        "heald_doctor" => {
            run_doctor_checks()
        }
        "heald_learn" => {
            let query = args.get("query").and_then(|v| v.as_str());
            let scan = args.get("scan").and_then(|v| v.as_bool()).unwrap_or(false);
            let record = args.get("record").and_then(|v| v.as_bool()).unwrap_or(false);
            let feature = args.get("feature").and_then(|v| v.as_str());
            let notes = args.get("notes").and_then(|v| v.as_str());
            mcp_learn(query, scan, record, feature, notes)
        }
        "heald_clean" => {
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
            let threshold = args.get("threshold").and_then(|v| v.as_u64()).map(|t| t as usize);
            mcp_clean(dry_run, threshold)
        }
        "heald_finalize" => {
            let summary = args.get("summary").and_then(|v| v.as_str()).ok_or("Missing required 'summary'")?;
            finalize_session(summary)
        }
        "heald_drift" => {
            let fix = args.get("fix").and_then(|v| v.as_bool()).unwrap_or(false);
            mcp_drift(fix)
        }
        other => Err(format!("Unknown tool: {}", other)),
    }
}

fn mcp_learn(query: Option<&str>, scan: bool, record: bool, feature: Option<&str>, notes: Option<&str>) -> Result<String, String> {
    let root = std::env::current_dir().unwrap_or_default();
    let local_base = root.join(".heald");
    if !local_base.exists() {
        crate::cmd::init::run(false);
    }
    if record {
        crate::cmd::learn::record_learning_workflow(&local_base, feature, &[], None, notes.unwrap_or("Learned pattern"));
        return Ok(format!("Recorded learning for feature '{}'", feature.unwrap_or("Auto")));
    }
    let kb = if scan || crate::cmd::learn::load_knowledge(&local_base).is_none() {
        let new_kb = crate::cmd::learn::scan_codebase(&root);
        crate::cmd::learn::save_knowledge(&local_base, &new_kb);
        new_kb
    } else {
        crate::cmd::learn::load_knowledge(&local_base).unwrap()
    };
    if let Some(q) = query {
        let matches = crate::cmd::learn::query_knowledge(&kb, q);
        if matches.is_empty() {
            return Ok(format!("No learned features matching query '{}'. Available: {}", q, kb.features.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")));
        }
        let mut out = String::new();
        for (feat, matched_files, _) in matches.into_iter().take(3) {
            out.push_str(&format!("🎯 Feature: {}\nFiles to inspect:\n", feat.name));
            for (file, syms) in matched_files {
                out.push_str(&format!("  • `{}` ({})\n", file.path, file.layer));
                let display_syms = if !syms.is_empty() { syms } else { file.symbols.iter().collect() };
                for s in display_syms.into_iter().take(5) {
                    out.push_str(&format!("    - `{}` [{}] (L{})\n", s.name, s.kind, s.line));
                }
            }
            let wfs: Vec<&crate::cmd::learn::WorkflowGuide> = kb.workflows.iter().filter(|w| w.feature == feat.name).collect();
            if !wfs.is_empty() {
                out.push_str("Learned Workflows:\n");
                for w in wfs {
                    out.push_str(&format!("  - {}\n", w.guidance));
                }
            }
            out.push('\n');
        }
        Ok(out)
    } else {
        let mut out = String::from("Heald Learned Feature Index:\n");
        for feat in &kb.features {
            let files: Vec<String> = feat.files.iter().take(3).map(|f| f.path.clone()).collect();
            out.push_str(&format!("• {}: {}\n", feat.name, files.join(", ")));
        }
        Ok(out)
    }
}

fn mcp_clean(dry_run: bool, threshold: Option<usize>) -> Result<String, String> {
    let archived = crate::cmd::clean::run_clean(false, dry_run, false, threshold);
    Ok(format!("Clean complete. Archived {} documents.", archived))
}

fn recall_memory(
    query_opt: Option<&str>,
    budget_opt: Option<usize>,
    compact: bool,
    strict_intent: bool,
) -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    if !local_base.exists() {
        crate::cmd::init::run(false);
    } else {
        crate::xref::rebuild_memory_index(&local_base);
    }

    let memory_dir = local_base.join("memory");
    let mut docs = Vec::new();
    let mut index_doc = None;

    if memory_dir.exists() {
        let walker = walkdir::WalkDir::new(&memory_dir).into_iter().filter_map(|e| e.ok());
        for entry in walker {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md")
                && let Ok(doc) = Document::from_file(path) {
                    if path.file_name().and_then(|n| n.to_str()) == Some("index.md") {
                        index_doc = Some(doc);
                    } else if path.file_name().and_then(|n| n.to_str()) != Some("log.md") {
                        docs.push(doc);
                    }
                }
        }
    }

    if let Some(query) = query_opt {
        let q = query.to_lowercase();
        if strict_intent {
            docs.retain(|d| {
                d.frontmatter.effective_title().to_lowercase().contains(&q)
                    || d.content.to_lowercase().contains(&q)
                    || d.frontmatter.tags.as_ref().is_some_and(|tags| tags.iter().any(|t| t.to_lowercase().contains(&q) || t.eq_ignore_ascii_case("pinned")))
            });
        } else {
            docs.retain(|d| {
                d.frontmatter.effective_title().to_lowercase().contains(&q)
                    || d.content.to_lowercase().contains(&q)
                    || d.frontmatter.tags.as_ref().is_some_and(|tags| tags.iter().any(|t| t.to_lowercase().contains(&q)))
            });
        }
    }

    docs.sort_by_key(|d| -score_doc(d));

    let budget_chars = budget_opt.unwrap_or(8000) * 4;
    let mut current_chars = 0;
    let mut included = Vec::new();

    if let Some(idx) = index_doc {
        if strict_intent && query_opt.is_some() {
            let active_titles: Vec<String> = docs
                .iter()
                .map(|d| d.frontmatter.effective_title().to_string())
                .collect();
            let sliced_idx = crate::cmd::context::slice_manifest_index(&idx, &active_titles);
            let len = sliced_idx.content.len();
            current_chars += len;
            included.push(sliced_idx);
        } else {
            let len = idx.content.len();
            current_chars += len;
            included.push(idx);
        }
    }

    let total_available = docs.len() + if included.is_empty() { 0 } else { 1 };

    for doc in docs {
        let len = doc.content.len();
        if current_chars + len <= budget_chars {
            current_chars += len;
            included.push(doc);
        }
    }

    let intent_tag = if strict_intent { " [Intent-Sliced]" } else { "" };
    let mut out = format!(
        "<!-- Heald Context: Included {}/{} memory documents (Budget: {} chars{}){} -->\n",
        included.len(),
        total_available,
        budget_chars,
        query_opt.map(|q| format!(", Query: \"{}\"", q)).unwrap_or_default(),
        intent_tag
    );

    for doc in included {
        if compact {
            out.push_str(&format!("\n{}\n", crate::cmd::context::format_compact_doc(&doc)));
        } else if let Some(title) = &doc.frontmatter.title {
            out.push_str(&format!("\n# {}\n\n{}\n", title, doc.content));
        } else {
            out.push_str(&format!("\n# {}\n\n{}\n", doc.frontmatter.r#type, doc.content));
        }
    }

    Ok(out)
}

fn mcp_drift(fix: bool) -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let project_root = std::env::current_dir().unwrap_or_default();
    if !local_base.exists() {
        return Err(".heald directory not found. Run `heald init` first.".to_string());
    }
    let items = crate::cmd::drift::detect_drift(&local_base, &project_root);
    if items.is_empty() {
        return Ok("✓ Zero symbol drift detected across all memory documents and codebase symbols.".to_string());
    }
    if fix {
        crate::cmd::drift::run(true, false);
        return Ok(format!("Repaired {} drifted symbol reference(s).", items.len()));
    }
    let res = serde_json::to_string_pretty(&items).map_err(|e| e.to_string())?;
    Ok(res)
}

fn finalize_session(summary: &str) -> Result<String, String> {
    if summary.trim().is_empty() {
        return Err("Summary cannot be empty".to_string());
    }
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    if !local_base.exists() {
        crate::cmd::init::run(false);
    }
    let memory_dir = local_base.join("memory");
    let log_path = memory_dir.join("log.md");
    let timestamp = Utc::now().to_rfc3339();
    let entry = format!("\n## Session {}\n\n{}\n", timestamp, summary.trim());

    if log_path.exists() {
        let mut content = std::fs::read_to_string(&log_path).unwrap_or_default();
        content.push_str(&entry);
        std::fs::write(&log_path, content).map_err(|e| e.to_string())?;
    } else {
        std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
        std::fs::write(&log_path, format!("---\ntype: log\n---\n# Memory Log\n{}", entry)).map_err(|e| e.to_string())?;
    }

    crate::cmd::learn::auto_learn_from_git(summary);
    crate::cmd::clean::run_clean(true, false, false, Some(20));

    Ok("Session finalized and logged successfully.".to_string())
}

fn score_doc(doc: &Document) -> i32 {
    let mut s = 0;
    if let Some(tags) = &doc.frontmatter.tags
        && tags.iter().any(|t| t == "pinned") {
            s += 1000;
        }
    if let Some(ts) = &doc.frontmatter.timestamp
        && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
            let days = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days();
            s += (100 - days).max(0) as i32;
        }
    s
}

fn remember_memory(doc_type: &str, title: &str, body: &str, tags_opt: Option<&[String]>) -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    if !local_base.exists() {
        crate::cmd::init::run(false);
    }

    let memory_dir = local_base.join("memory");
    if !memory_dir.exists() {
        std::fs::create_dir_all(&memory_dir).map_err(|e| e.to_string())?;
    }

    let timestamp = Utc::now().to_rfc3339();
    let slug = title.to_lowercase().replace(' ', "-").chars().filter(|c| c.is_alphanumeric() || *c == '-').collect::<String>();

    let tags_yaml = if let Some(tags) = tags_opt {
        if !tags.is_empty() {
            format!("tags: [{}]\n", tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(", "))
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let content = format!(
        "---\ntype: {}\ntitle: \"{}\"\ntimestamp: {}\n{}---\n{}\n",
        doc_type, title, timestamp, tags_yaml, body
    );

    let path = memory_dir.join(format!("{}.md", slug));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    crate::xref::rebuild_memory_index(&local_base);

    Ok(format!("Saved memory to {}", path.display()))
}

fn forget_memory(query: &str) -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let memory_dir = local_base.join("memory");

    if !memory_dir.exists() {
        return Err("No memory directory found.".to_string());
    }

    if query == "index" {
        return Err("Refusing to delete index.md.".to_string());
    }

    let mut exact_match: Option<(PathBuf, String)> = None;
    let mut fuzzy_matches = Vec::new();

    let walker = walkdir::WalkDir::new(&memory_dir).into_iter().filter_map(|e| e.ok());
    for entry in walker {
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            let file_stem = entry.path().file_stem().unwrap().to_str().unwrap();
            if file_stem == "log" || file_stem == "index" {
                continue;
            }

            if let Ok(doc) = Document::from_file(entry.path()) {
                let title = doc.frontmatter.effective_title().to_string();
                if file_stem == query {
                    exact_match = Some((entry.path().to_path_buf(), title));
                    break;
                }
                if title.to_lowercase().contains(&query.to_lowercase()) {
                    fuzzy_matches.push((entry.path().to_path_buf(), file_stem.to_string(), title));
                }
            }
        }
    }

    let (target_path, target_title) = if let Some(exact) = exact_match {
        exact
    } else if fuzzy_matches.len() == 1 {
        let m = fuzzy_matches.pop().unwrap();
        (m.0, m.2)
    } else if fuzzy_matches.len() > 1 {
        let list = fuzzy_matches.iter().map(|m| format!("- {} (slug: {})", m.2, m.1)).collect::<Vec<_>>().join("\n");
        return Err(format!("Multiple matches found for \"{}\":\n{}", query, list));
    } else {
        return Err(format!("No memory document found matching \"{}\".", query));
    };

    std::fs::remove_file(&target_path).map_err(|e| e.to_string())?;

    let log_path = memory_dir.join("log.md");
    let timestamp = Utc::now().to_rfc3339();
    let log_entry = format!("Forgot: \"{}\" ({})\n", target_title, timestamp);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = file.write_all(log_entry.as_bytes());
    }

    crate::xref::rebuild_memory_index(&local_base);

    Ok(format!("Forgot \"{}\" (deleted {})", target_title, target_path.file_name().unwrap().to_str().unwrap()))
}

fn get_repo_map() -> Result<String, String> {
    crate::cmd::map::run();
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let map_file = local_base.join("map.md");
    if map_file.exists() {
        std::fs::read_to_string(&map_file).map_err(|e| e.to_string())
    } else {
        Err("Failed to generate repository map.".to_string())
    }
}

fn blame_path(path: &str) -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let docs = crate::xref::get_memory_docs(&local_base);

    let target_path = Path::new(path);
    let mut files_to_check = Vec::new();

    if target_path.is_dir() {
        let walker = ignore::WalkBuilder::new(target_path).build();
        for result in walker {
            if let Ok(entry) = result
                && entry.path().is_file() {
                    files_to_check.push(entry.path().to_path_buf());
                }
        }
    } else {
        files_to_check.push(target_path.to_path_buf());
    }

    let mut all_refs = std::collections::BTreeMap::new();
    let mut total_refs = 0;

    for file in files_to_check {
        let file_str = file.to_string_lossy().to_string();
        let norm = crate::xref::normalize_path(&file_str);
        let refs = crate::xref::find_references(&docs, &norm);
        if !refs.is_empty() {
            total_refs += refs.len();
            all_refs.insert(norm, refs);
        }
    }

    if total_refs == 0 {
        return Ok(format!("No decisions on record for {}", path));
    }

    let mut out = String::new();
    for (file, refs) in all_refs {
        out.push_str(&format!("{}\n", file));
        for r in refs {
            out.push_str(&format!("  {:<20} {:<10} \"{}\"\n", r.doc_filename, r.timestamp, r.snippet));
        }
    }

    Ok(out)
}

fn run_doctor_checks() -> Result<String, String> {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let global_base = dirs::home_dir().unwrap_or_default().join(".heald");

    let mut issues = Vec::new();
    for base in &[local_base, global_base] {
        if base.exists() {
            check_doctor_dir(&base.join("rules"), &mut issues);
            check_doctor_dir(&base.join("skills"), &mut issues);
            check_doctor_dir(&base.join("memory"), &mut issues);
        }
    }

    if issues.is_empty() {
        Ok("Bundle is healthy. All documents and rules parsed cleanly.".to_string())
    } else {
        Ok(format!("Doctor found issues:\n{}", issues.join("\n")))
    }
}

fn check_doctor_dir(dir: &Path, issues: &mut Vec<String>) {
    if !dir.exists() {
        return;
    }
    let walker = walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok());
    for entry in walker {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md")
            && let Err(e) = Document::from_file(path) {
                issues.push(format!("ERROR in {}: {}", path.display(), e));
            }
    }
}
