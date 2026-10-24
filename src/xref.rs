use std::path::{Path, PathBuf};
use crate::okf::Document;
use chrono::{DateTime, Utc};

pub struct Reference {
    pub title: String,
    pub timestamp: String,
    pub snippet: String,
    pub doc_filename: String,
}

pub fn get_memory_docs(local_base: &Path) -> Vec<(PathBuf, Document)> {
    let memory_dir = local_base.join("memory");
    let mut docs = Vec::new();
    if memory_dir.exists() {
        let walker = walkdir::WalkDir::new(&memory_dir).into_iter().filter_map(|e| e.ok());
        for entry in walker {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                if path.components().any(|c| c.as_os_str() == "archive") {
                    continue;
                }
                if let Ok(doc) = Document::from_file(path)
                    && path.file_name().and_then(|n| n.to_str()) != Some("index.md") &&
                       path.file_name().and_then(|n| n.to_str()) != Some("log.md") {
                        docs.push((path.to_path_buf(), doc));
                    }
            }
        }

    }
    // Sort by timestamp descending
    docs.sort_by(|a, b| {
        let ts_a = a.1.frontmatter.timestamp.as_deref().unwrap_or("");
        let ts_b = b.1.frontmatter.timestamp.as_deref().unwrap_or("");
        let a_time = DateTime::parse_from_rfc3339(ts_a).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        let b_time = DateTime::parse_from_rfc3339(ts_b).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        b_time.cmp(&a_time)
    });
    docs
}

pub fn normalize_path(path: &str) -> String {
    let path = path.replace("\\", "/");
    
    path.strip_prefix("./").unwrap_or(&path).to_string()
}

pub fn extract_snippet(content: &str, path: &str) -> Option<String> {
    let path_norm = normalize_path(path);
    for line in content.lines() {
        if line.contains(&path_norm) {
            let mut snippet = line.trim().to_string();
            if snippet.len() > 100 {
                snippet.truncate(97);
                snippet.push_str("...");
            }
            return Some(snippet);
        }
    }
    None
}

/// Extract candidate source code file paths mentioned in a document.
pub fn extract_referenced_paths(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let re = regex::Regex::new(r"(?:^|[\s`'\(])([~a-zA-Z0-9_\-\./\\]+\.[a-zA-Z0-9_\-]+)").unwrap();
    let code_extensions = [
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "rb", "c", "cpp", "h", "hpp",
        "toml", "json", "yaml", "yml", "sql", "sh", "css", "scss", "html", "vue", "svelte", "proto",
    ];
    for cap in re.captures_iter(content) {
        let matched = &cap[1];
        // Exclude paths starting with ~ or absolute paths starting with /
        if matched.starts_with('~') || matched.starts_with('/') {
            continue;
        }
        let norm = normalize_path(matched);
        // Exclude web URLs, anchors, markdown syntax fragments, or pure extensions
        if norm.starts_with("http://") || norm.starts_with("https://") || norm.starts_with("www.") {
            continue;
        }
        // Exclude common memory doc references
        if norm.ends_with(".md") {
            continue;
        }
        let ext = Path::new(&norm).extension().and_then(|s| s.to_str()).unwrap_or("");
        if code_extensions.contains(&ext) && !paths.contains(&norm) {
            paths.push(norm);
        }
    }
    paths
}


pub fn find_references(docs: &[(PathBuf, Document)], file_path: &str) -> Vec<Reference> {
    let mut refs = Vec::new();
    let norm = normalize_path(file_path);
    for (doc_path, doc) in docs {
        if let Some(snippet) = extract_snippet(&doc.content, &norm) {
            let doc_filename = doc_path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let title = doc.frontmatter.title.clone().unwrap_or_else(|| doc_filename.clone());
            let timestamp = doc.frontmatter.timestamp.clone().unwrap_or_else(|| "".to_string());
            // simplify timestamp to just date for display
            let ts = if timestamp.len() >= 10 { timestamp[..10].to_string() } else { timestamp };
            refs.push(Reference {
                title,
                timestamp: ts,
                snippet,
                doc_filename,
            });
        }
    }
    refs
}

pub fn rebuild_memory_index(local_base: &Path) {
    let memory_dir = local_base.join("memory");
    if !memory_dir.exists() {
        return;
    }
    let docs = get_memory_docs(local_base);
    let index_path = memory_dir.join("index.md");

    let mut body = String::new();
    if docs.is_empty() {
        body.push_str("*(No active memory documents or architectural decisions recorded yet.)*\n");
    } else {
        body.push_str("## Project Memory Manifest\n\n");
        body.push_str("| Title | Type | Date | Tags | Summary |\n");
        body.push_str("| :--- | :--- | :--- | :--- | :--- |\n");

        let total_docs = docs.len();
        let display_limit = 25;

        for (path, doc) in docs.iter().take(display_limit) {
            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let title = doc.frontmatter.effective_title();
            let doc_type = &doc.frontmatter.r#type;
            let date = if let Some(ts) = &doc.frontmatter.timestamp {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                    dt.format("%Y-%m-%d").to_string()
                } else if ts.len() >= 10 {
                    ts[..10].to_string()
                } else {
                    ts.clone()
                }
            } else {
                "-".to_string()
            };
            let tags = doc.frontmatter.tags.as_ref()
                .map(|t| t.join(", "))
                .unwrap_or_else(|| "-".to_string());

            let first_line = doc.content
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim();
            let mut snippet = first_line.to_string();
            if snippet.len() > 60 {
                snippet.truncate(57);
                snippet.push_str("...");
            }
            let snippet = snippet.replace('|', "\\|");
            let title_escaped = title.replace('|', "\\|");

            body.push_str(&format!(
                "| **{}** (`{}.md`) | `{}` | {} | {} | {} |\n",
                title_escaped, file_stem, doc_type, date, tags, snippet
            ));
        }

        if total_docs > display_limit {
            body.push_str(&format!(
                "\n*(Showing {} of {} active memory documents. Run `heald clean` to consolidate related decisions into domain files.)*\n",
                display_limit, total_docs
            ));
        }
    }

    let full_content = format!(
        "---\ntype: summary\ntitle: \"Memory Index\"\n---\n{}",
        body
    );

    if let Err(e) = std::fs::write(&index_path, full_content) {
        eprintln!("Failed to write index.md: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_paths() {
        let text = "Added blackbox-execution rule to ~/.heald/rules and updated ~/.gemini/config/AGENTS.md to enforce. Modified src/auth.rs and non_existent.rs.";
        let paths = extract_referenced_paths(text);
        assert_eq!(paths, vec!["src/auth.rs", "non_existent.rs"]);
    }
}
