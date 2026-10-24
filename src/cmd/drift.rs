use crate::xref::{get_memory_docs, normalize_path};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DriftIssueType {
    SymbolMoved,
    SymbolMissing,
    OrphanPath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    pub doc_file: String,
    pub doc_title: String,
    pub symbol: String,
    pub issue_type: DriftIssueType,
    pub message: String,
    pub old_target: Option<String>,
    pub suggested_fix: Option<String>,
    pub line_in_doc: Option<usize>,
}

pub struct SymbolLocation {
    pub file: String,
    pub line: usize,
    pub kind: String,
}

fn pick_best_location<'a>(
    symbol: &str,
    locs: &'a [SymbolLocation],
) -> &'a SymbolLocation {
    let mut scored: Vec<(i32, &'a SymbolLocation)> = locs
        .iter()
        .map(|loc| {
            let mut score = 0;
            if !loc.file.contains("test") {
                score += 10;
            }
            let file_lower = loc.file.to_lowercase();
            for part in symbol.split('_') {
                if part.len() >= 3 && file_lower.contains(&part.to_lowercase()) {
                    score += 5;
                }
            }
            (score, loc)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored[0].1
}

const IGNORED_IDENTIFIERS: &[&str] = &[
    "true", "false", "bool", "usize", "isize", "u8", "u16", "u32", "u64", "u128", "i8", "i16",
    "i32", "i64", "i128", "f32", "f64", "str", "String", "Vec", "Option", "Result", "None",
    "Some", "Ok", "Err", "Box", "Arc", "Rc", "Self", "self", "let", "mut", "pub", "fn", "struct",
    "enum", "impl", "trait", "async", "await", "return", "type", "decision", "rule", "learning",
    "heald", "cargo", "test", "todo", "fixme", "pinned", "json", "yaml", "toml", "md", "mdc",
    "sh", "bash", "zsh", "fish", "path", "file", "index", "log", "git", "cli", "mcp", "api",
    "auth", "db", "ui", "ux", "get", "set", "run", "all", "yes", "no", "main", "init", "sync",
    "clean", "compact", "forget", "blame", "map", "doctor", "learn", "skill", "context",
];

pub fn detect_drift(local_base: &Path, project_root: &Path) -> Vec<DriftItem> {
    let mut items = Vec::new();

    // 1. Scan codebase symbols using learn engine
    let kb = crate::cmd::learn::scan_codebase(project_root);

    let mut symbols_by_name: HashMap<String, Vec<SymbolLocation>> = HashMap::new();
    let mut symbols_by_file: HashMap<String, HashSet<String>> = HashMap::new();
    let mut existing_code_files: HashSet<String> = HashSet::new();

    for feat in &kb.features {
        for file in &feat.files {
            let norm_path = normalize_path(&file.path);
            existing_code_files.insert(norm_path.clone());
            let file_syms = symbols_by_file.entry(norm_path.clone()).or_default();

            for sym in &file.symbols {
                file_syms.insert(sym.name.clone());
                symbols_by_name
                    .entry(sym.name.clone())
                    .or_default()
                    .push(SymbolLocation {
                        file: norm_path.clone(),
                        line: sym.line,
                        kind: sym.kind.clone(),
                    });
            }
        }
    }

    // Also collect all files on disk under project_root
    let mut all_files_on_disk: HashSet<String> = HashSet::new();
    let walker = ignore::WalkBuilder::new(project_root).hidden(false).build();
    for entry in walker.flatten() {
        if entry.path().is_file() {
            if let Ok(rel) = entry.path().strip_prefix(project_root) {
                all_files_on_disk.insert(normalize_path(&rel.to_string_lossy()));
            }
        }
    }

    // 2. Read all memory documents
    let docs = get_memory_docs(local_base);

    let scoped_regex =
        regex::Regex::new(r"([a-zA-Z0-9_\-\./\\]+\.[a-zA-Z0-9_\-]+)::([a-zA-Z0-9_]+)").unwrap();
    let backtick_regex = regex::Regex::new(r"`([a-zA-Z0-9_]+)(?:\(\))?`").unwrap();

    let ignored_set: HashSet<&str> = IGNORED_IDENTIFIERS.iter().copied().collect();

    for (doc_path, doc) in docs {
        let doc_filename = doc_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let doc_title = doc.frontmatter.effective_title().to_string();

        let mut seen_doc_issues = HashSet::new();

        // Check line by line to track line numbers
        for (line_idx, line) in doc.content.lines().enumerate() {
            let line_num = line_idx + 1;

            // A. Check scoped symbol references: `path/file.rs::symbol_name`
            for cap in scoped_regex.captures_iter(line) {
                let file_ref = &cap[1];
                let symbol_name = &cap[2];

                if ignored_set.contains(symbol_name) {
                    continue;
                }

                let norm_file = normalize_path(file_ref);

                if !all_files_on_disk.contains(&norm_file) {
                    // Handled as orphan or moved
                    if let Some(locs) = symbols_by_name.get(symbol_name) {
                        let best = pick_best_location(symbol_name, locs);
                        let key = format!("moved:{}:{}", doc_filename, symbol_name);
                        if seen_doc_issues.insert(key) {
                            items.push(DriftItem {
                                doc_file: doc_filename.clone(),
                                doc_title: doc_title.clone(),
                                symbol: symbol_name.to_string(),
                                issue_type: DriftIssueType::SymbolMoved,
                                message: format!(
                                    "File `{}` missing; symbol `{}` now exists in `{}:{}`",
                                    norm_file, symbol_name, best.file, best.line
                                ),
                                old_target: Some(format!("{}::{}", norm_file, symbol_name)),
                                suggested_fix: Some(format!("{}::{}", best.file, symbol_name)),
                                line_in_doc: Some(line_num),
                            });
                        }
                    } else {
                        let key = format!("missing:{}:{}", doc_filename, symbol_name);
                        if seen_doc_issues.insert(key) {
                            items.push(DriftItem {
                                doc_file: doc_filename.clone(),
                                doc_title: doc_title.clone(),
                                symbol: symbol_name.to_string(),
                                issue_type: DriftIssueType::OrphanPath,
                                message: format!(
                                    "Referenced path `{}` does not exist on disk",
                                    norm_file
                                ),
                                old_target: Some(norm_file.clone()),
                                suggested_fix: None,
                                line_in_doc: Some(line_num),
                            });
                        }
                    }
                    continue;
                }

                // File exists on disk. Check if symbol exists in this file
                let file_syms = symbols_by_file.get(&norm_file);
                let has_symbol = file_syms.is_some_and(|s| s.contains(symbol_name));

                if !has_symbol {
                    if let Some(locs) = symbols_by_name.get(symbol_name) {
                        let best = pick_best_location(symbol_name, locs);
                        let key = format!("moved:{}:{}", doc_filename, symbol_name);
                        if seen_doc_issues.insert(key) {
                            items.push(DriftItem {
                                doc_file: doc_filename.clone(),
                                doc_title: doc_title.clone(),
                                symbol: symbol_name.to_string(),
                                issue_type: DriftIssueType::SymbolMoved,
                                message: format!(
                                    "Symbol `{}` not found in `{}`; moved to `{}:{}`",
                                    symbol_name, norm_file, best.file, best.line
                                ),
                                old_target: Some(format!("{}::{}", norm_file, symbol_name)),
                                suggested_fix: Some(format!("{}::{}", best.file, symbol_name)),
                                line_in_doc: Some(line_num),
                            });
                        }
                    } else {
                        let key = format!("missing_sym:{}:{}", doc_filename, symbol_name);
                        if seen_doc_issues.insert(key) {
                            items.push(DriftItem {
                                doc_file: doc_filename.clone(),
                                doc_title: doc_title.clone(),
                                symbol: symbol_name.to_string(),
                                issue_type: DriftIssueType::SymbolMissing,
                                message: format!(
                                    "Symbol `{}` in `{}` no longer exists in codebase",
                                    symbol_name, norm_file
                                ),
                                old_target: Some(format!("{}::{}", norm_file, symbol_name)),
                                suggested_fix: None,
                                line_in_doc: Some(line_num),
                            });
                        }
                    }
                }
            }

            // B. Check backtick references: `symbol_name` or `symbol_name()`
            for cap in backtick_regex.captures_iter(line) {
                let symbol_name = &cap[1];

                if ignored_set.contains(symbol_name) {
                    continue;
                }

                // Only consider symbols that look like code symbols (>= 4 chars, contains '_' or mixed-case)
                let is_camel_or_pascal = symbol_name.chars().any(|c| c.is_uppercase())
                    && symbol_name.chars().any(|c| c.is_lowercase());
                let is_snake = symbol_name.contains('_');
                let has_parens = line.contains(&format!("`{}()`", symbol_name));

                if !is_snake && !is_camel_or_pascal && !has_parens {
                    continue;
                }

                // Check if the doc mentions a specific file on this or nearby line, or in the whole doc
                let referenced_paths = crate::xref::extract_referenced_paths(line);
                for ref_path in referenced_paths {
                    let norm = normalize_path(&ref_path);
                    if all_files_on_disk.contains(&norm) {
                        let file_syms = symbols_by_file.get(&norm);
                        if let Some(s) = file_syms {
                            if !s.contains(symbol_name) {
                                // Symbol does not exist in the referenced file
                                if let Some(locs) = symbols_by_name.get(symbol_name) {
                                    let best = pick_best_location(symbol_name, locs);
                                    let key = format!("moved:{}:{}", doc_filename, symbol_name);
                                    if seen_doc_issues.insert(key) {
                                        items.push(DriftItem {
                                            doc_file: doc_filename.clone(),
                                            doc_title: doc_title.clone(),
                                            symbol: symbol_name.to_string(),
                                            issue_type: DriftIssueType::SymbolMoved,
                                            message: format!(
                                                "Symbol `{}` referenced with `{}` not found in that file; found in `{}:{}`",
                                                symbol_name, norm, best.file, best.line
                                            ),
                                            old_target: Some(norm),
                                            suggested_fix: Some(best.file.clone()),
                                            line_in_doc: Some(line_num),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    items
}

pub fn run(fix: bool, json_output: bool) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let project_root = std::env::current_dir().unwrap_or_default();

    if !local_base.exists() {
        eprintln!("Error: .heald directory not found. Run `heald init` first.");
        std::process::exit(1);
    }

    let drift_items = detect_drift(&local_base, &project_root);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&drift_items).unwrap());
        return;
    }

    println!("=== Heald Memory Symbol Drift Report ===\n");

    if drift_items.is_empty() {
        println!("✓ Zero symbol drift detected across all memory documents and codebase symbols.");
        return;
    }

    println!("Found {} symbol drift issue(s):\n", drift_items.len());

    let mut fixed_count = 0;

    for item in &drift_items {
        let tag = match item.issue_type {
            DriftIssueType::SymbolMoved => "[MOVED]",
            DriftIssueType::SymbolMissing => "[MISSING]",
            DriftIssueType::OrphanPath => "[ORPHAN]",
        };

        let loc_str = item
            .line_in_doc
            .map(|l| format!(":{}", l))
            .unwrap_or_default();
        println!("• {} In '{}{}' (Title: \"{}\")", tag, item.doc_file, loc_str, item.doc_title);
        println!("  {}", item.message);

        if let Some(fix_target) = &item.suggested_fix {
            if let Some(old) = &item.old_target {
                println!("  Suggested fix: replace `{}` -> `{}`", old, fix_target);
            }
        }

        if fix {
            if let (Some(old), Some(new_val)) = (&item.old_target, &item.suggested_fix) {
                let doc_path = local_base.join("memory").join(&item.doc_file);
                if doc_path.exists() {
                    if let Ok(content) = fs::read_to_string(&doc_path) {
                        if content.contains(old) {
                            let updated = content.replace(old, new_val);
                            if fs::write(&doc_path, updated).is_ok() {
                                println!("  ✓ Auto-patched reference in {}", item.doc_file);
                                fixed_count += 1;
                            }
                        }
                    }
                }
            }
        }
        println!();
    }

    if fix {
        println!("Drift repair complete: fixed {} reference(s).", fixed_count);
    } else {
        let fixable_count = drift_items
            .iter()
            .filter(|i| i.suggested_fix.is_some() && i.old_target.is_some())
            .count();
        if fixable_count > 0 {
            println!(
                "Run `heald drift --fix` to automatically repair {} moved symbol reference(s).",
                fixable_count
            );
        }
    }
}
