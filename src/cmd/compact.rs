use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use crate::okf::Document;

pub fn run(dry_run: bool) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    if !local_base.exists() {
        eprintln!("ERROR (Heald): Workspace .heald not found. Run `heald init` first.");
        std::process::exit(1);
    }

    println!("Compacting Heald memories and session logs...");
    let memory_dir = local_base.join("memory");
    let archive_dir = memory_dir.join("archive");

    // 1. Compact session logs
    compact_logs(&memory_dir, dry_run);

    // 2. Consolidate and archive duplicate/outdated memories
    compact_memories(&local_base, &memory_dir, &archive_dir, dry_run);

    // 3. Cluster and merge related memories into unified topic docs
    crate::cmd::clean::run_clean(false, dry_run, false, Some(15));

    if !dry_run {
        crate::xref::rebuild_memory_index(&local_base);
        println!("Compaction complete. Memory index rebuilt.");
    } else {
        println!("Dry run complete. No changes written.");
    }
}

pub fn compact_logs(memory_dir: &Path, dry_run: bool) {
    let log_path = memory_dir.join("log.md");
    if !log_path.exists() {
        return;
    }

    let content = match fs::read_to_string(&log_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let (cleaned_log, entries_compacted) = process_log_content(&content);
    if entries_compacted > 0 {
        println!("  - Consolidated {} log entries (removed duplicates)", entries_compacted);
        if !dry_run {
            let _ = fs::write(&log_path, cleaned_log);
        }
    }
}

pub fn process_log_content(content: &str) -> (String, usize) {
    let mut header = String::new();
    let mut sessions: Vec<(String, String)> = Vec::new(); // (heading, body)

    let mut current_heading = String::new();
    let mut current_body = String::new();
    let mut in_session = false;

    for line in content.lines() {
        if line.starts_with("## Session ") || line.starts_with("## ") {
            if in_session {
                sessions.push((current_heading.clone(), current_body.trim().to_string()));
                current_body.clear();
            } else {
                in_session = true;
            }
            current_heading = line.to_string();
        } else if in_session {
            current_body.push_str(line);
            current_body.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    if in_session {
        sessions.push((current_heading, current_body.trim().to_string()));
    }

    if sessions.is_empty() {
        return (content.to_string(), 0);
    }

    let original_count = sessions.len();
    let mut seen_bodies = HashSet::new();
    let mut unique_sessions = Vec::new();

    for (heading, body) in sessions {
        let norm_body = body.trim().to_lowercase();
        if !norm_body.is_empty() && seen_bodies.insert(norm_body) {
            unique_sessions.push((heading, body));
        }
    }

    let removed = original_count - unique_sessions.len();
    let mut out = header.trim_end().to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    for (heading, body) in unique_sessions {
        out.push('\n');
        out.push_str(&heading);
        out.push_str("\n\n");
        out.push_str(&body);
        out.push('\n');
    }

    (out, removed)
}

pub fn compact_memories(local_base: &Path, _memory_dir: &Path, archive_dir: &Path, dry_run: bool) {
    let docs = crate::xref::get_memory_docs(local_base);
    if docs.is_empty() {
        return;
    }

    // Group docs by normalized effective title or similarity
    let mut title_groups: HashMap<String, Vec<(PathBuf, Document)>> = HashMap::new();
    for (path, doc) in docs {
        let key = doc.frontmatter.effective_title().trim().to_lowercase();
        title_groups.entry(key).or_default().push((path, doc));
    }

    let mut to_archive: Vec<(PathBuf, String)> = Vec::new();

    for (_title_key, mut group) in title_groups {
        if group.len() > 1 {
            // Sort so newest is first
            group.sort_by(|a, b| {
                let ts_a = a.1.frontmatter.timestamp.as_deref().unwrap_or("");
                let ts_b = b.1.frontmatter.timestamp.as_deref().unwrap_or("");
                let a_time = DateTime::parse_from_rfc3339(ts_a).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
                let b_time = DateTime::parse_from_rfc3339(ts_b).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
                b_time.cmp(&a_time)
            });

            // Keep the newest (index 0), archive older duplicates (index 1..)
            for (path, doc) in group.into_iter().skip(1) {
                let title = doc.frontmatter.effective_title().to_string();
                to_archive.push((path, title));
            }
        }
    }

    if to_archive.is_empty() {
        return;
    }

    if !dry_run && !archive_dir.exists() {
        let _ = fs::create_dir_all(archive_dir);
    }

    for (path, title) in &to_archive {
        let file_name = path.file_name().unwrap_or_default();
        let target = archive_dir.join(file_name);
        println!("  - Archiving superseded memory doc '{}' -> {}", title, target.display());
        if !dry_run {
            if let Err(e) = fs::rename(path, &target) {
                eprintln!("    Failed to move {}: {}", path.display(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_log_content_dedup() {
        let raw = "---\ntype: log\n---\n# Memory Log\n\n## Session 2026-08-20T01:00:00Z\n\nDid task A\n\n## Session 2026-08-20T02:00:00Z\n\nDid task A\n\n## Session 2026-08-20T03:00:00Z\n\nDid task B\n";
        let (cleaned, removed) = process_log_content(raw);
        assert_eq!(removed, 1);
        assert!(cleaned.contains("Did task A"));
        assert!(cleaned.contains("Did task B"));
        assert_eq!(cleaned.matches("Did task A").count(), 1);
    }
}
