use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::okf::Document;
use crate::xref::{extract_referenced_paths, get_memory_docs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub domain: String,
    pub title: String,
    pub doc_count: usize,
    pub source_files: Vec<String>,
}

/// Known domain definitions with keyword stems for grouping memories.
const DOMAIN_RULES: &[(&str, &[&str])] = &[
    ("Calendar & Recurring Tasks", &[
        "calendar", "recurring", "schedule", "dayplan", "agenda", "template", "multiday", "multi-day", "date"
    ]),
    ("SQLite DB & Concurrency", &[
        "sqlite", "wal", "locking", "transaction", "concurrency", "retry", "database", "pragma"
    ]),
    ("Cloud Sync & Tombstones", &[
        "sync", "cloud", "tombstone", "turso", "lww", "deleted_at", "backfill", "cursor"
    ]),
    ("AI Mascot & Voice Assistant", &[
        "mascot", "floating", "zeno", "voice", "gemini", "assistant", "ai chat", "shift_task", "intent", "insights", "ai insights", "dismissal"
    ]),
    ("Social Integrations & OAuth", &[
        "oauth", "instagram", "youtube", "pinterest", "meta", "social", "connection", "accounts", "avatar", "token", "account", "accountid", "profile", "retry", "sequence"
    ]),
    ("DM Automation & Webhooks", &[
        "inbox", "dm", "unibox", "automation", "webhook", "responder", "follower", "reply"
    ]),
    ("Email Synchronization & Polling", &[
        "email", "imap", "smtp", "zoho", "outlook", "polling"
    ]),
    ("UI System & Theme", &[
        "theme", "dark", "light", "white", "color", "styling", "css", "tailwind", "layout", "bottom bar", "navbar", "modal", "slider", "card", "viewfinder",
        "header", "mobile", "drawer", "picker", "icon", "icons", "border", "dropdown", "bottom sheet", "navigation", "animation", "nav", "bar", "home page", "figma", "boxes", "transparent", "logo", "screen", "rendering", "ui", "wheel", "select", "optical", "shell"
    ]),
    ("Android Mobile & Release", &[
        "release", "keystore", "play", "signing", "bundle", "apk", "aab", "version", "proguard", "r8", "gradle", "capacitor", "intent://", "android", "adb", "sync-run", "app name"
    ]),
    ("Legal & Account Compliance", &[
        "deletion", "privacy", "terms", "legal", "compliance", "herenow", "webpage"
    ]),
    ("CRM & Campaigns", &[
        "campaign", "lead", "crm", "contact", "audience"
    ]),
    ("Content & Media Creator", &[
        "content", "post", "video", "upload", "camera", "media", "editor", "viewfinder", "reel", "story", "stories"
    ]),
    ("Backend & Deployment", &[
        "convex", "vite", "build", "compiler", "vercel", "deploy", "server", "endpoint", "api", "environment", "env", "architecture", "modular", "zustand", "store", "state management", "route", "routes", "workspace", "folder", "authentication", "auth"
    ]),
];

/// Check if a document is an ephemeral or trivial micro-record that doesn't need to be an active decision.
pub fn is_trivial_or_ephemeral(title: &str, content: &str) -> bool {
    let lower_title = title.to_lowercase();
    let lower_content = content.to_lowercase();

    // Version bump / release tag logs
    if lower_title.starts_with("bumped version")
        || lower_title.starts_with("app release version")
        || lower_title.starts_with("release keystore signing and version")
        || lower_title.contains("version bump")
    {
        return true;
    }

    // Trivial confirmations or non-actions
    if lower_title.starts_with("confirmed ") && content.len() < 300 {
        return true;
    }

    // Removed / repealed scratch features (e.g. wallpaper slider that was immediately removed)
    if (lower_title.starts_with("removed ") || lower_title.starts_with("canceled "))
        && (lower_content.contains("removed") || lower_content.contains("deleted"))
    {
        return true;
    }

    // Installed skill announcement docs
    if lower_title.starts_with("installed ") && lower_title.contains("skill") && content.len() < 300 {
        return true;
    }

    false
}

/// Detect repealed features: if doc A adds X and doc B removes X, doc A is superseded/repealed.
pub fn detect_repealed_docs(docs: &[(PathBuf, Document)]) -> HashSet<PathBuf> {
    let mut repealed = HashSet::new();
    let mut removal_keywords = Vec::new();

    for (path, doc) in docs {
        let title = doc.frontmatter.effective_title().to_lowercase();
        if title.starts_with("removed ") || title.starts_with("deleted ") || title.starts_with("reverted ") {
            let target = title
                .trim_start_matches("removed ")
                .trim_start_matches("deleted ")
                .trim_start_matches("reverted ")
                .trim();
            if target.len() > 3 {
                removal_keywords.push((path.clone(), target.to_string()));
            }
        }
    }

    for (path, doc) in docs {
        let title = doc.frontmatter.effective_title().to_lowercase();
        for (removal_path, keyword) in &removal_keywords {
            if path != removal_path && title.contains(keyword) && !title.starts_with("removed ") {
                repealed.insert(path.clone());
            }
        }
    }

    repealed
}

/// Match document to a domain cluster based on title, content, and file references.
pub fn match_domain(title: &str, content: &str, file_refs: &[String]) -> Option<&'static str> {
    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();

    let mut best_domain: Option<&'static str> = None;
    let mut best_score = 0;

    // 1. Check domain keywords in title and content
    for (domain, keywords) in DOMAIN_RULES {
        let mut score = 0;
        for kw in *keywords {
            if title_lower.contains(kw) {
                score += 3;
            }
            if content_lower.contains(kw) {
                score += 1;
            }
        }
        if score > best_score && score >= 2 {
            best_score = score;
            best_domain = Some(domain);
        }
    }

    if best_domain.is_some() {
        return best_domain;
    }

    // 2. Check file path heuristics
    for path in file_refs {
        let p = path.to_lowercase();
        if p.contains("calendar") || p.contains("recurring") {
            return Some("Calendar & Recurring Tasks");
        }
        if p.contains("sqlite") || p.contains("db/") || p.contains("queries/") {
            return Some("SQLite DB & Concurrency");
        }
        if p.contains("sync") {
            return Some("Cloud Sync & Tombstones");
        }
        if p.contains("mascot") || p.contains("gemini") {
            return Some("AI Mascot & Voice Assistant");
        }
        if p.contains("oauth") || p.contains("social") || p.contains("account") {
            return Some("Social Integrations & OAuth");
        }
        if p.contains("inbox") || p.contains("dm") || p.contains("automation") {
            return Some("DM Automation & Webhooks");
        }
        if p.contains("theme") || p.contains("styles") || p.contains("ui/") {
            return Some("UI System & Theme");
        }
        if p.contains("convex/") {
            return Some("Backend & Deployment");
        }
    }

    None
}

/// Cluster documents into logical domains.
pub fn cluster_documents(
    docs: Vec<(PathBuf, Document)>,
) -> (
    HashMap<String, Vec<(PathBuf, Document)>>, // domain -> docs
    Vec<(PathBuf, Document)>,                 // standalone docs that don't cluster
    Vec<(PathBuf, String)>,                   // docs to prune/archive directly
) {
    let repealed = detect_repealed_docs(&docs);
    let mut to_prune = Vec::new();
    let mut candidate_docs = Vec::new();

    for (path, doc) in docs {
        let title = doc.frontmatter.effective_title();
        // Check if repealed or trivial
        if repealed.contains(&path) {
            to_prune.push((path, format!("Repealed feature: {}", title)));
            continue;
        }
        if is_trivial_or_ephemeral(title, &doc.content) {
            to_prune.push((path, format!("Trivial micro-log: {}", title)));
            continue;
        }
        candidate_docs.push((path, doc));
    }

    let mut clusters: HashMap<String, Vec<(PathBuf, Document)>> = HashMap::new();
    let mut standalone = Vec::new();

    for (path, doc) in candidate_docs {
        let title = doc.frontmatter.effective_title();
        // If already consolidated, treat it as its domain anchor
        if doc.frontmatter.tags.as_ref().map_or(false, |t| t.iter().any(|x| x == "consolidated"))
            || title.starts_with("Consolidated: ")
        {
            let domain = title.trim_start_matches("Consolidated: ").to_string();
            clusters.entry(domain).or_default().push((path, doc));
            continue;
        }

        let file_refs = extract_referenced_paths(&doc.content);
        if let Some(domain) = match_domain(title, &doc.content, &file_refs) {
            clusters.entry(domain.to_string()).or_default().push((path, doc));
        } else {
            standalone.push((path, doc));
        }
    }

    (clusters, standalone, to_prune)
}

/// Generate a consolidated document body from a cluster of related docs.
pub fn synthesize_cluster(
    domain: &str,
    mut docs: Vec<(PathBuf, Document)>,
) -> (String, String, Vec<PathBuf>) {
    // Sort docs by timestamp descending (newest first)
    docs.sort_by(|a, b| {
        let ts_a = a.1.frontmatter.timestamp.as_deref().unwrap_or("");
        let ts_b = b.1.frontmatter.timestamp.as_deref().unwrap_or("");
        let a_time = DateTime::parse_from_rfc3339(ts_a).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        let b_time = DateTime::parse_from_rfc3339(ts_b).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        b_time.cmp(&a_time)
    });

    let latest_timestamp = docs
        .first()
        .and_then(|(_, d)| d.frontmatter.timestamp.clone())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let slug = format!(
        "consolidated-{}",
        domain
            .to_lowercase()
            .replace('&', "and")
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
    );

    let title = format!("Consolidated: {}", domain);

    let mut all_files = BTreeSet::new();
    let mut decision_points: Vec<String> = Vec::new();
    let mut history_entries: Vec<String> = Vec::new();
    let mut source_paths = Vec::new();

    for (path, doc) in &docs {
        source_paths.push(path.clone());
        let d_title = doc.frontmatter.effective_title();
        let ts = doc.frontmatter.timestamp.as_deref().unwrap_or("");
        let date_str = if ts.len() >= 10 { &ts[..10] } else { ts };

        let is_consolidated = doc.frontmatter.tags.as_ref().map_or(false, |t| t.iter().any(|x| x == "consolidated"))
            || d_title.starts_with("Consolidated: ");

        if is_consolidated {
            // Parse existing consolidated content, stripping duplicate headers and categorizing bullets
            for line in doc.content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty()
                    || trimmed.starts_with('#')
                    || trimmed.starts_with("Consolidated architectural")
                    || trimmed.starts_with("*(No detailed")
                    || trimmed.starts_with("*(Files referenced")
                {
                    continue;
                }
                if trimmed.starts_with("- `") && trimmed.ends_with('`') {
                    let file_str = trimmed.trim_start_matches("- `").trim_end_matches('`');
                    if !file_str.is_empty() {
                        all_files.insert(file_str.to_string());
                    }
                } else if trimmed.starts_with("- ") && (trimmed.contains(" — ") || trimmed.starts_with("- 20")) {
                    if !history_entries.contains(&trimmed.to_string()) {
                        history_entries.push(trimmed.to_string());
                    }
                } else if trimmed.starts_with("- ") {
                    if !decision_points.contains(&trimmed.to_string()) {
                        decision_points.push(trimmed.to_string());
                    }
                }
            }
            continue;
        }

        // Collect referenced files from non-consolidated doc
        for fpath in extract_referenced_paths(&doc.content) {
            all_files.insert(fpath);
        }

        let first_line = doc
            .content
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .unwrap_or("")
            .trim();

        if !first_line.is_empty() {
            let bullet = format!("- **{}**: {}", d_title, first_line);
            if !decision_points.contains(&bullet) {
                decision_points.push(bullet);
            }
        }

        let history_bullet = format!(
            "- {} — {} (`{}`)",
            date_str,
            d_title,
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        if !history_entries.contains(&history_bullet) {
            history_entries.push(history_bullet);
        }
    }

    let mut body = String::new();
    body.push_str(&format!(
        "---\ntype: decision\ntitle: \"{}\"\ntimestamp: {}\ntags: [consolidated, \"{}\"]\n---\n\n",
        title, latest_timestamp, domain.to_lowercase().replace(' ', "-")
    ));

    body.push_str(&format!("# {}\n\n", title));
    body.push_str("Consolidated architectural decisions, system invariants, and implementation patterns.\n\n");

    body.push_str("## Core System Decisions & Invariants\n\n");
    if decision_points.is_empty() {
        body.push_str("*(No detailed decision notes)*\n\n");
    } else {
        for p in &decision_points {
            body.push_str(p);
            body.push('\n');
        }
        body.push('\n');
    }

    body.push_str("## Impacted Files & Subsystems\n\n");
    if all_files.is_empty() {
        body.push_str("*(Files referenced in historical session logs)*\n\n");
    } else {
        for f in &all_files {
            body.push_str(&format!("- `{}`\n", f));
        }
        body.push('\n');
    }

    body.push_str(&format!(
        "## Consolidated Evolution History ({} items archived)\n\n",
        history_entries.len()
    ));
    for h in &history_entries {
        body.push_str(h);
        body.push('\n');
    }

    (slug, body, source_paths)
}

/// Run memory cleanup, clustering, and consolidation.
pub fn run_clean(auto: bool, dry_run: bool, _force: bool, threshold: Option<usize>) -> usize {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    if !local_base.exists() {
        if !auto {
            eprintln!("ERROR (Heald): No .heald directory found in current directory.");
        }
        return 0;
    }

    let memory_dir = local_base.join("memory");
    let archive_dir = memory_dir.join("archive");

    let docs = get_memory_docs(&local_base);
    let total_initial = docs.len();

    let limit = threshold.unwrap_or(20);
    // If auto mode and total docs is already below threshold, nothing to do
    if auto && total_initial <= limit {
        return 0;
    }

    println!(
        "Heald Memory Clean: Scanning {} active memory documents (threshold: {})...",
        total_initial, limit
    );

    let (clusters, standalone, to_prune) = cluster_documents(docs);

    if !dry_run && !archive_dir.exists() {
        let _ = fs::create_dir_all(&archive_dir);
    }

    let mut archived_count = 0;

    // 1. Prune trivial or repealed docs
    for (path, reason) in to_prune {
        let file_name = path.file_name().unwrap_or_default();
        let target = archive_dir.join(file_name);
        println!("  - Pruning/Archiving: {} ({})", file_name.to_string_lossy(), reason);
        if !dry_run {
            let _ = fs::rename(&path, &target);
        }
        archived_count += 1;
    }

    // 2. Consolidate clusters that have 2 or more documents (or existing consolidated docs that need normalization)
    let mut consolidated_created = 0;
    for (domain, group) in clusters {
        let is_existing_consolidated = group.iter().any(|(_, d)| {
            d.frontmatter.tags.as_ref().map_or(false, |t| t.iter().any(|x| x == "consolidated"))
                || d.frontmatter.effective_title().starts_with("Consolidated: ")
        });
        if group.len() >= 2 || is_existing_consolidated {
            // If it's a single consolidated doc, only rewrite if it needs normalization (duplicate headers)
            if group.len() == 1 {
                let doc_content = &group[0].1.content;
                let count = doc_content.matches("# Consolidated:").count();
                if count <= 1 {
                    continue;
                }
            }

            let (slug, content, source_paths) = synthesize_cluster(&domain, group);
            let consolidated_file = memory_dir.join(format!("{}.md", slug));

            println!(
                "  - Merging {} docs -> '{}' ({})",
                source_paths.len(),
                domain,
                consolidated_file.file_name().unwrap_or_default().to_string_lossy()
            );

            if !dry_run {
                // Write the consolidated file
                let _ = fs::write(&consolidated_file, content);

                // Move individual source files to archive (unless it's the consolidated file itself)
                for src in source_paths {
                    if src != consolidated_file {
                        let fname = src.file_name().unwrap_or_default();
                        let target = archive_dir.join(fname);
                        let _ = fs::rename(&src, &target);
                        archived_count += 1;
                    }
                }
            } else {
                archived_count += source_paths.len();
            }
            consolidated_created += 1;
        } else if group.len() == 1 {
            // Keep the single doc in standalone
        }
    }

    if !dry_run {
        crate::xref::rebuild_memory_index(&local_base);
        let remaining = get_memory_docs(&local_base).len();
        println!(
            "Clean complete: Archived {} documents, created {} consolidated domains. Active docs: {} -> {}.",
            archived_count, consolidated_created, total_initial, remaining
        );
    } else {
        println!(
            "Dry run complete: Would archive {} documents into {} consolidated domains. Standalone: {}.",
            archived_count, consolidated_created, standalone.len()
        );
    }

    archived_count
}

pub fn run(auto: bool, dry_run: bool, force: bool, threshold: Option<usize>) {
    run_clean(auto, dry_run, force, threshold);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_trivial_or_ephemeral() {
        assert!(is_trivial_or_ephemeral("Bumped version to 1.0.3", "Updated package.json"));
        assert!(is_trivial_or_ephemeral("App Release Version 1.0.3", "Bumped Android code"));
        assert!(is_trivial_or_ephemeral("Removed wallpaper slider plugin", "Removed all wallpaper code"));
        assert!(!is_trivial_or_ephemeral("SQLite WAL Concurrency and Retry Strategy", "Detailed architecture with exponential backoff..."));
    }

    #[test]
    fn test_match_domain() {
        let domain = match_domain("Fixed recurring task calendar pre-population", "useCalendarSchedule", &[]);
        assert_eq!(domain, Some("Calendar & Recurring Tasks"));

        let domain_oauth = match_domain("Instagram OAuth Callback and Tokens", "token exchange", &[]);
        assert_eq!(domain_oauth, Some("Social Integrations & OAuth"));

        let domain_ui = match_domain("Increased capture logo size to 64x64px in mobile header", "", &[]);
        assert_eq!(domain_ui, Some("UI System & Theme"));
    }

    #[test]
    fn test_synthesize_cluster_with_existing_consolidated() {
        let existing_content = r#"---
title: "Consolidated: Social Integrations & OAuth"
timestamp: 2026-08-21T00:00:00Z
tags: [consolidated, "social-integrations-and-oauth"]
---

# Consolidated: Social Integrations & OAuth

Consolidated architectural decisions, system invariants, and implementation patterns.

## Core System Decisions & Invariants

# Consolidated: Social Integrations & OAuth

## Core System Decisions & Invariants

- **YouTube OAuth Token Refresh**: Added YouTube token refresh.

## Impacted Files & Subsystems

- `src/auth.rs`

## Consolidated Evolution History (1 items archived)

- 2026-08-21 — YouTube OAuth Token Refresh (`youtube-oauth-refresh.md`)
"#;
        let existing_doc = Document::parse(existing_content).unwrap();

        let new_content = r#"---
title: "Instagram OAuth Flow"
timestamp: 2026-08-22T00:00:00Z
---
Added Instagram OAuth login handler.
"#;
        let new_doc = Document::parse(new_content).unwrap();

        let (slug, body, paths) = synthesize_cluster(
            "Social Integrations & OAuth",
            vec![
                (PathBuf::from("consolidated-social-integrations-and-oauth.md"), existing_doc),
                (PathBuf::from("instagram-oauth-flow.md"), new_doc),
            ],
        );

        assert_eq!(slug, "consolidated-social-integrations-and-oauth");
        assert_eq!(paths.len(), 2);

        // Crucial: Verify NO duplicate "# Consolidated:" headers exist in synthesized body!
        assert_eq!(body.matches("# Consolidated:").count(), 1);
        assert_eq!(body.matches("## Core System Decisions & Invariants").count(), 1);
        assert_eq!(body.matches("## Impacted Files & Subsystems").count(), 1);
        assert_eq!(body.matches("## Consolidated Evolution History").count(), 1);

        // Verify both decisions and history entries are present
        assert!(body.contains("YouTube OAuth Token Refresh"));
        assert!(body.contains("Instagram OAuth Flow"));
        assert!(body.contains("youtube-oauth-refresh.md"));
        assert!(body.contains("instagram-oauth-flow.md"));
    }
}
