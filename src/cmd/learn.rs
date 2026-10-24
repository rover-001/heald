use crate::bm25::tokenize;
use crate::xref::normalize_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String, // "Function", "Hook", "Component", "Query", "Mutation", "Route", "Struct", "Class"
    pub line: usize,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedFile {
    pub path: String,
    pub layer: String, // "Route", "Hook", "Component", "Store", "Database", "Service", "Util"
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedFeature {
    pub name: String,
    pub description: String,
    pub files: Vec<LearnedFile>,
    pub workflows: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeBase {
    pub last_scanned: String,
    pub features: Vec<LearnedFeature>,
    pub file_index: HashMap<String, String>, // file_path -> feature_name
    pub workflows: Vec<WorkflowGuide>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGuide {
    pub feature: String,
    pub files: Vec<String>,
    pub symbols: Vec<String>,
    pub guidance: String,
    pub updated_at: String,
}

/// Classify the architectural layer of a file based on path and name
pub fn classify_layer(rel_path: &str) -> &'static str {
    let lower = rel_path.to_lowercase();
    let name = Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if lower.contains("routes/")
        || lower.contains("screens/")
        || lower.contains("pages/")
        || lower.contains("app/")
    {
        "Route"
    } else if lower.contains("stores/")
        || lower.contains("context/")
        || lower.contains("state/")
        || lower.contains("redux")
    {
        "Store"
    } else if lower.contains("db/")
        || lower.contains("queries/")
        || lower.contains("convex/")
        || lower.contains("schema")
        || lower.contains("migrations/")
    {
        "Database"
    } else if lower.contains("services/") || lower.contains("api/") || lower.contains("server/") {
        "Service"
    } else if lower.contains("hooks/") || name.starts_with("use") {
        "Hook"
    } else if lower.contains("components/")
        || (rel_path.ends_with(".tsx") && name.chars().next().is_some_and(|c| c.is_uppercase()))
    {
        "Component"
    } else {
        "Util"
    }
}

/// Detect feature domain for a file
pub fn detect_file_feature(rel_path: &str, content: &str) -> &'static str {
    let lower_path = rel_path.to_lowercase();
    let lower_content = content.to_lowercase();

    if lower_path.contains("clean") || lower_path.contains("compact") {
        "Memory Consolidation & Clean"
    } else if lower_path.contains("learn") {
        "Codebase Learning & Indexing"
    } else if lower_path.contains("mcp") || lower_path.contains("serve") {
        "Model Context Protocol (MCP)"
    } else if lower_path.contains("context")
        || lower_path.contains("remember")
        || lower_path.contains("finalize")
    {
        "Context & Memory Engine"
    } else if lower_path.contains("bm25")
        || lower_path.contains("search")
        || lower_path.contains("tokenize")
    {
        "Search & Retrieval"
    } else if lower_path.starts_with("src/cmd/") || lower_path.contains("/cmd/") {
        "CLI Commands"
    } else if lower_path.contains("calendar")
        || lower_path.contains("recurring")
        || lower_path.contains("schedule")
    {
        "Calendar & Scheduling"
    } else if lower_path.contains("oauth")
        || lower_path.contains("auth")
        || lower_path.contains("login")
        || lower_path.contains("session")
    {
        "Authentication & OAuth"
    } else if lower_path.contains("social")
        || lower_path.contains("instagram")
        || lower_path.contains("youtube")
        || lower_path.contains("pinterest")
    {
        "Social Channels & Accounts"
    } else if lower_path.contains("inbox")
        || lower_path.contains("dm")
        || lower_path.contains("automation")
        || lower_path.contains("webhook")
    {
        "DM Automation & Unified Inbox"
    } else if lower_path.contains("email")
        || lower_path.contains("imap")
        || lower_path.contains("smtp")
    {
        "Email Synchronization & Polling"
    } else if lower_path.contains("mascot")
        || lower_path.contains("gemini")
        || lower_path.contains("voice")
    {
        "AI Mascot & Voice Assistant"
    } else if lower_path.contains("sync")
        || lower_path.contains("cloud")
        || lower_path.contains("turso")
    {
        "Cloud Sync & Replication"
    } else if lower_path.contains("sqlite")
        || lower_path.contains("db")
        || lower_path.contains("convex")
        || lower_path.contains("schema")
    {
        "Database & Schema"
    } else if lower_path.contains("theme")
        || lower_path.contains("styles")
        || lower_path.contains("colors")
        || lower_path.contains("layout")
        || lower_path.contains("ui")
    {
        "UI Theme & Components"
    } else if lower_path.contains("campaign")
        || lower_path.contains("crm")
        || lower_path.contains("leads")
    {
        "CRM & Campaigns"
    } else if lower_content.contains("calendar") && lower_content.contains("recurring") {
        "Calendar & Scheduling"
    } else if lower_content.contains("oauth")
        || lower_content.contains("jwt")
        || lower_content.contains("access_token")
    {
        "Authentication & OAuth"
    } else {
        "Core Application Logic"
    }
}

/// Extract symbols (functions, hooks, components, queries) from a code file
pub fn extract_symbols(content: &str, file_ext: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();

    // Regex patterns for TypeScript / JavaScript
    let re_ts_fn = regex::Regex::new(r"export\s+(?:async\s+)?function\s+([a-zA-Z0-9_]+)").unwrap();
    let re_ts_const_fn =
        regex::Regex::new(r"export\s+const\s+([a-zA-Z0-9_]+)\s*=\s*(?:async\s*)?\(").unwrap();
    let re_ts_convex = regex::Regex::new(r"export\s+const\s+([a-zA-Z0-9_]+)\s*=\s*(?:internalM|m|q|internalQ|a|internalA)(?:utation|uery|ction)\(").unwrap();
    let re_ts_component =
        regex::Regex::new(r"export\s+(?:default\s+)?function\s+([A-Z][a-zA-Z0-9_]+)").unwrap();
    let re_regular_fn =
        regex::Regex::new(r"^(?:async\s+)?function\s+([a-zA-Z0-9_]+)\s*\(").unwrap();

    // Regex for Rust
    let re_rust_fn = regex::Regex::new(r"(?:pub(?:\([^\)]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
    let re_rust_struct = regex::Regex::new(r"(?:pub(?:\([^\)]+\))?\s+)?(?:struct|enum)\s+([a-zA-Z0-9_]+)").unwrap();

    // Regex for Python
    let re_py_fn = regex::Regex::new(r"^\s*(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\(").unwrap();
    let re_py_class = regex::Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)").unwrap();

    // Regex for Go
    let re_go_fn = regex::Regex::new(r"^func\s+(?:\([^\)]+\)\s+)?([a-zA-Z0-9_]+)\s*\(").unwrap();

    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let line_trimmed = line.trim();

        if file_ext == "ts" || file_ext == "tsx" || file_ext == "js" || file_ext == "jsx" {
            if let Some(cap) = re_ts_convex.captures(line_trimmed) {
                let name = cap[1].to_string();
                let kind = if line_trimmed.contains("mutation") {
                    "Mutation"
                } else if line_trimmed.contains("query") {
                    "Query"
                } else {
                    "Action"
                };
                symbols.push(SymbolInfo {
                    name,
                    kind: kind.to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_ts_fn.captures(line_trimmed) {
                let name = cap[1].to_string();
                let kind = if name.starts_with("use") {
                    "Hook"
                } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    "Component"
                } else {
                    "Function"
                };
                symbols.push(SymbolInfo {
                    name,
                    kind: kind.to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_ts_const_fn.captures(line_trimmed) {
                let name = cap[1].to_string();
                let kind = if name.starts_with("use") {
                    "Hook"
                } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    "Component"
                } else {
                    "Function"
                };
                symbols.push(SymbolInfo {
                    name,
                    kind: kind.to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_ts_component.captures(line_trimmed) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "Component".to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_regular_fn.captures(line_trimmed) {
                let name = cap[1].to_string();
                if name.starts_with("use") {
                    symbols.push(SymbolInfo {
                        name,
                        kind: "Hook".to_string(),
                        line: line_num,
                        doc: None,
                    });
                }
            }
        } else if file_ext == "rs" {
            if let Some(cap) = re_rust_fn.captures(line_trimmed) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "Function".to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_rust_struct.captures(line_trimmed) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "Type".to_string(),
                    line: line_num,
                    doc: None,
                });
            }
        } else if file_ext == "py" {
            if let Some(cap) = re_py_fn.captures(line) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "Function".to_string(),
                    line: line_num,
                    doc: None,
                });
            } else if let Some(cap) = re_py_class.captures(line) {
                symbols.push(SymbolInfo {
                    name: cap[1].to_string(),
                    kind: "Class".to_string(),
                    line: line_num,
                    doc: None,
                });
            }
        } else if file_ext == "go"
            && let Some(cap) = re_go_fn.captures(line)
        {
            symbols.push(SymbolInfo {
                name: cap[1].to_string(),
                kind: "Function".to_string(),
                line: line_num,
                doc: None,
            });
        }
    }

    symbols
}

/// Scan project codebase and build knowledge base
pub fn scan_codebase(root: &Path) -> KnowledgeBase {
    let mut feature_groups: HashMap<String, Vec<LearnedFile>> = HashMap::new();
    let mut file_index = HashMap::new();

    let walker = ignore::WalkBuilder::new(root).hidden(false).build();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

            let path_str = path.to_string_lossy();
            // Skip common non-source directories
            if path_str.contains("/.git/")
                || path_str.contains("/.heald/")
                || path_str.contains("/node_modules/")
                || path_str.contains("/target/")
                || path_str.contains("/dist/")
                || path_str.contains("/.output/")
                || path_str.contains("/.next/")
                || path_str.contains("/android/")
                || path_str.contains("/ios/")
                || path_str.contains("/.convex/")
                || path_str.contains("/assets/")
            {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "ts" | "tsx" | "js" | "jsx" | "rs" | "py" | "go") {
                continue;
            }

            // Skip minified or bundle files
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name.ends_with(".min.js")
                || file_name.ends_with(".bundle.js")
                || file_name.ends_with(".gen.ts")
            {
                continue;
            }

            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = normalize_path(&rel.to_string_lossy());

            if let Ok(content) = fs::read_to_string(path) {
                let layer = classify_layer(&rel_str).to_string();
                let symbols = extract_symbols(&content, ext);

                if !symbols.is_empty()
                    || layer == "Route"
                    || layer == "Store"
                    || layer == "Database"
                {
                    let feature_name = detect_file_feature(&rel_str, &content).to_string();
                    let learned_file = LearnedFile {
                        path: rel_str.clone(),
                        layer,
                        symbols,
                    };
                    file_index.insert(rel_str.clone(), feature_name.clone());
                    feature_groups
                        .entry(feature_name)
                        .or_default()
                        .push(learned_file);
                }
            }
        }

    let mut features = Vec::new();
    for (name, mut files) in feature_groups {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        features.push(LearnedFeature {
            name: name.clone(),
            description: format!("Architectural subsystem covering {}", name),
            files,
            workflows: Vec::new(),
        });
    }
    features.sort_by(|a, b| a.name.cmp(&b.name));

    KnowledgeBase {
        last_scanned: chrono::Utc::now().to_rfc3339(),
        features,
        file_index,
        workflows: Vec::new(),
    }
}

/// Save knowledge base to .heald/knowledge/
pub fn save_knowledge(local_base: &Path, kb: &KnowledgeBase) {
    let knowledge_dir = local_base.join("knowledge");
    let _ = fs::create_dir_all(&knowledge_dir);

    // Save JSON
    let json_path = knowledge_dir.join("features.json");
    if let Ok(serialized) = serde_json::to_string_pretty(kb) {
        let _ = fs::write(&json_path, serialized);
    }

    // Save markdown map for fast agent & human inspection
    let md_path = knowledge_dir.join("feature-map.md");
    let md_content = generate_markdown_feature_map(kb);
    let _ = fs::write(&md_path, md_content);
}

/// Load knowledge base from .heald/knowledge/features.json
pub fn load_knowledge(local_base: &Path) -> Option<KnowledgeBase> {
    let json_path = local_base.join("knowledge").join("features.json");
    if json_path.exists()
        && let Ok(raw) = fs::read_to_string(json_path)
        && let Ok(mut kb) = serde_json::from_str::<KnowledgeBase>(&raw)
    {
        // Load additional workflows if present
        let wf_path = local_base.join("knowledge").join("workflows.json");
        if wf_path.exists()
            && let Ok(wf_raw) = fs::read_to_string(wf_path)
            && let Ok(wfs) = serde_json::from_str::<Vec<WorkflowGuide>>(&wf_raw)
        {
            kb.workflows = wfs;
        }
        return Some(kb);
    }
    None
}

/// Generate human/agent-readable markdown feature map
pub fn generate_markdown_feature_map(kb: &KnowledgeBase) -> String {
    let mut out = String::new();
    out.push_str("# Heald Learned Codebase & Feature Navigator\n\n");
    out.push_str(
        "Generated by `heald learn`. Fast index of features, primary files, and functions.\n\n",
    );

    for feat in &kb.features {
        out.push_str(&format!("## Feature: {}\n\n", feat.name));
        for file in &feat.files {
            out.push_str(&format!("### `{}` ({})\n", file.path, file.layer));
            if file.symbols.is_empty() {
                out.push_str("  *(Primary file for this feature)*\n");
            } else {
                for sym in &file.symbols {
                    out.push_str(&format!(
                        "  - `{}` [{}] (line {})\n",
                        sym.name, sym.kind, sym.line
                    ));
                }
            }
            out.push('\n');
        }

        if !feat.workflows.is_empty() {
            out.push_str("#### Learned Workflows & Guidelines:\n");
            for wf in &feat.workflows {
                out.push_str(&format!("- {}\n", wf));
            }
            out.push('\n');
        }
    }

    if !kb.workflows.is_empty() {
        out.push_str("## Active Task & Flow Guides\n\n");
        for wf in &kb.workflows {
            out.push_str(&format!("### {}\n", wf.feature));
            out.push_str(&format!("- **Files**: {}\n", wf.files.join(", ")));
            if !wf.symbols.is_empty() {
                out.push_str(&format!("- **Functions**: {}\n", wf.symbols.join(", ")));
            }
            out.push_str(&format!("- **Guidance**: {}\n\n", wf.guidance));
        }
    }

    out
}

pub type QueryMatchResult<'a> = (
    &'a LearnedFeature,
    Vec<(&'a LearnedFile, Vec<&'a SymbolInfo>)>,
    f64,
);

/// Search knowledge base for relevant feature, files, and functions
pub fn query_knowledge<'a>(
    kb: &'a KnowledgeBase,
    query: &str,
) -> Vec<QueryMatchResult<'a>> {
    let q_tokens = tokenize(query);
    let mut scored_results = Vec::new();

    for feat in &kb.features {
        let mut score: f64 = 0.0;
        let feat_name_lower = feat.name.to_lowercase();
        for t in &q_tokens {
            if feat_name_lower.contains(t) {
                score += 5.0;
            }
        }

        let mut matched_files = Vec::new();

        for file in &feat.files {
            let mut file_score: f64 = 0.0;
            let path_lower = file.path.to_lowercase();
            for t in &q_tokens {
                if path_lower.contains(t) {
                    file_score += 3.0;
                }
            }

            let mut matched_symbols = Vec::new();
            for sym in &file.symbols {
                let sym_lower = sym.name.to_lowercase();
                for t in &q_tokens {
                    if sym_lower.contains(t) {
                        file_score += 2.0;
                        matched_symbols.push(sym);
                    }
                }
            }

            if file_score > 0.0 || score > 0.0 {
                matched_files.push((file, matched_symbols));
                score += file_score;
            }
        }

        // Also check workflows
        for wf in &kb.workflows {
            if wf.feature == feat.name {
                for t in &q_tokens {
                    if wf.guidance.to_lowercase().contains(t) {
                        score += 3.0;
                    }
                }
            }
        }

        if score > 0.0 {
            scored_results.push((feat, matched_files, score));
        }
    }

    scored_results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    scored_results
}

/// Auto-learn: detect modified files from git diff and record workflow guidance
pub fn auto_learn_from_git(summary: &str) {
    let root = std::env::current_dir().unwrap_or_default();
    let local_base = root.join(".heald");
    if !local_base.exists() {
        return;
    }

    // Attempt to run git diff --name-only HEAD~1 or git status --porcelain
    let git_out = std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD~1"])
        .output()
        .or_else(|_| {
            std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .output()
        });

    let touched_files: Vec<String> = match git_out {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines()
                .map(|l| {
                    l.trim_start_matches(|c: char| {
                        c.is_whitespace() || c == 'M' || c == 'A' || c == '?'
                    })
                    .trim()
                    .to_string()
                })
                .filter(|f| {
                    !f.is_empty()
                        && (f.ends_with(".ts")
                            || f.ends_with(".tsx")
                            || f.ends_with(".rs")
                            || f.ends_with(".py")
                            || f.ends_with(".go"))
                })
                .collect()
        }
        _ => Vec::new(),
    };

    if touched_files.is_empty() {
        return;
    }

    record_learning_workflow(&local_base, None, &touched_files, None, summary);
}

/// Record a learning workflow entry
pub fn record_learning_workflow(
    local_base: &Path,
    feature_opt: Option<&str>,
    files: &[String],
    symbols_opt: Option<&[String]>,
    guidance: &str,
) {
    let knowledge_dir = local_base.join("knowledge");
    let _ = fs::create_dir_all(&knowledge_dir);
    let wf_path = knowledge_dir.join("workflows.json");

    let mut workflows: Vec<WorkflowGuide> = if wf_path.exists() {
        fs::read_to_string(&wf_path)
            .ok()
            .and_then(|r| serde_json::from_str(&r).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let feature_name = feature_opt.map(|s| s.to_string()).unwrap_or_else(|| {
        if let Some(first_file) = files.first() {
            detect_file_feature(first_file, "").to_string()
        } else {
            "General".to_string()
        }
    });

    let symbols = symbols_opt.map(|s| s.to_vec()).unwrap_or_default();

    workflows.push(WorkflowGuide {
        feature: feature_name,
        files: files.to_vec(),
        symbols,
        guidance: guidance.to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    if let Ok(serialized) = serde_json::to_string_pretty(&workflows) {
        let _ = fs::write(&wf_path, serialized);
    }
}

/// CLI Runner for `heald learn`
pub fn run(
    query: Option<&str>,
    scan: bool,
    record: bool,
    feature: Option<&str>,
    files: Option<&[String]>,
    notes: Option<&str>,
    json: bool,
) {
    let root = std::env::current_dir().unwrap_or_default();
    let local_base = root.join(".heald");

    if !local_base.exists() {
        crate::cmd::init::run(false);
    }

    // Handle record mode
    if record {
        let notes_str = notes.unwrap_or("Learned pattern");
        let file_list = files.unwrap_or(&[]);
        record_learning_workflow(&local_base, feature, file_list, None, notes_str);
        println!(
            "Recorded workflow learning for feature '{}'.",
            feature.unwrap_or("Auto")
        );
        return;
    }

    // Load or scan
    let kb = if scan || load_knowledge(&local_base).is_none() {
        println!("Scanning codebase symbols and features...");
        let new_kb = scan_codebase(&root);
        save_knowledge(&local_base, &new_kb);
        println!(
            "Learned {} features across {} indexed files.",
            new_kb.features.len(),
            new_kb.file_index.len()
        );
        new_kb
    } else {
        load_knowledge(&local_base).unwrap()
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&kb).unwrap());
        return;
    }

    if let Some(q) = query {
        let matches = query_knowledge(&kb, q);
        if matches.is_empty() {
            println!("No learned features found matching query '{}'.", q);
            println!("Available features:");
            for f in &kb.features {
                println!("  - {} ({} files)", f.name, f.files.len());
            }
            return;
        }

        println!(
            "=== Heald Code Navigation & Learned Guidance for '{}' ===\n",
            q
        );
        for (feat, matched_files, _score) in matches.into_iter().take(3) {
            println!("🎯 Feature: {}", feat.name);
            println!("📁 Files to inspect:");
            for (file, matched_symbols) in matched_files {
                println!("  • `{}` ({})", file.path, file.layer);
                let display_syms = if !matched_symbols.is_empty() {
                    matched_symbols
                } else {
                    file.symbols.iter().collect()
                };

                for sym in display_syms.into_iter().take(5) {
                    println!("    - `{}` [{}] (line {})", sym.name, sym.kind, sym.line);
                }
            }

            // Check if any workflows exist for this feature
            let feature_wfs: Vec<&WorkflowGuide> = kb
                .workflows
                .iter()
                .filter(|w| w.feature == feat.name)
                .collect();
            if !feature_wfs.is_empty() {
                println!("\n💡 Learned Workflows & What to do:");
                for wf in feature_wfs {
                    println!("  - {}", wf.guidance);
                }
            }
            println!();
        }
    } else {
        println!("=== Heald Learned Feature Index ===");
        println!(
            "Run `heald learn <feature-or-query>` to see exact files and functions to modify.\n"
        );
        for feat in &kb.features {
            let top_files: Vec<String> =
                feat.files.iter().take(3).map(|f| f.path.clone()).collect();
            println!(
                "• **{}** ({} files): {}",
                feat.name,
                feat.files.len(),
                top_files.join(", ")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_layer() {
        assert_eq!(classify_layer("src/routes/calendar.tsx"), "Route");
        assert_eq!(classify_layer("src/hooks/useCalendarSchedule.ts"), "Hook");
        assert_eq!(classify_layer("src/components/Button.tsx"), "Component");
        assert_eq!(classify_layer("src/stores/useInboxStore.ts"), "Store");
        assert_eq!(classify_layer("src/db/queries/recurring.ts"), "Database");
    }

    #[test]
    fn test_extract_symbols_ts() {
        let code = r#"
            export function parseRecurringTemplate(raw: string): ScheduleRule { return {}; }
            export const useCalendarSchedule = () => {};
            export const removeRecurringTask = mutation({});
        "#;
        let symbols = extract_symbols(code, "ts");
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "parseRecurringTemplate");
        assert_eq!(symbols[0].kind, "Function");
        assert_eq!(symbols[1].name, "useCalendarSchedule");
        assert_eq!(symbols[1].kind, "Hook");
        assert_eq!(symbols[2].name, "removeRecurringTask");
        assert_eq!(symbols[2].kind, "Mutation");
    }

    #[test]
    fn test_extract_symbols_rust() {
        let code = r#"
            pub fn run_clean(dry_run: bool) -> usize { 0 }
            pub struct KnowledgeBase { pub name: String }
        "#;
        let symbols = extract_symbols(code, "rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "run_clean");
        assert_eq!(symbols[1].name, "KnowledgeBase");
    }
}
