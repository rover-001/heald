use crate::okf::Document;
use crate::xref::{extract_referenced_paths, get_memory_docs};
use std::collections::HashMap;
use std::path::Path;

pub fn run() {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let global_base = dirs::home_dir().unwrap_or_default().join(".heald");
    let project_root = std::env::current_dir().unwrap_or_default();

    let mut has_errors = false;
    for base in &[&local_base, &global_base] {
        if base.exists() {
            println!("Checking bundle at {}...", base.display());
            has_errors |= check_dir(&base.join("rules"));
            has_errors |= check_dir(&base.join("skills"));
            has_errors |= check_dir(&base.join("memory"));
        }
    }

    if local_base.exists() {
        println!("\nValidating memory documents & cross-references...");
        let (errs, warns) = validate_memory_integrity(&local_base, &project_root);
        has_errors |= errs;
        if warns > 0 {
            println!("Doctor found {} warning(s).", warns);
        }
    }

    if has_errors {
        println!("\nDoctor found issues.");
    } else {
        println!("\nBundle is healthy.");
    }
}

fn check_dir(dir: &Path) -> bool {
    let mut has_errors = false;
    if !dir.exists() {
        return false;
    }

    // Check all markdown files recursively
    let walker = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok());
    for entry in walker {
        let path = entry.path();
        if path.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("md")
            && let Err(e) = Document::from_file(path)
        {
            println!("ERROR in {}: {}", path.display(), e);
            has_errors = true;
        }
    }
    has_errors
}

/// Validates memory docs for:
/// 1. Broken xref links (links to markdown files or memory docs that don't exist)
/// 2. Orphan references (paths to source files in the project that no longer exist)
/// 3. Conflicting / overlapping decisions (multiple decisions touching the same target file)
pub fn validate_memory_integrity(local_base: &Path, project_root: &Path) -> (bool, usize) {
    let docs = get_memory_docs(local_base);
    let mut has_errors = false;
    let mut warnings_count = 0;

    // Track file references for conflict detection: normalized_rel_path -> Vec<(doc_filename, title)>
    let mut file_touch_map: HashMap<String, Vec<(String, String)>> = HashMap::new();

    let memory_dir = local_base.join("memory");
    let link_regex = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();

    for (doc_path, doc) in &docs {
        let doc_filename = doc_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let doc_title = doc.frontmatter.effective_title().to_string();

        // 1. Check markdown link targets [text](target)
        for cap in link_regex.captures_iter(&doc.content) {
            let target = &cap[2];
            // Skip http/https and anchors
            if target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with('#')
            {
                continue;
            }

            // Check if target exists relative to doc or relative to project root or memory dir
            let rel_to_doc = doc_path.parent().unwrap_or(local_base).join(target);
            let rel_to_mem = memory_dir.join(target);
            let rel_to_root = project_root.join(target);

            if !rel_to_doc.exists() && !rel_to_mem.exists() && !rel_to_root.exists() {
                println!(
                    "ERROR (Broken Link): In '{}' -> target '{}' not found",
                    doc_filename, target
                );
                has_errors = true;
            }
        }

        // 2. Check orphan file references in doc body
        let referenced_paths = extract_referenced_paths(&doc.content);
        for path_str in referenced_paths {
            let full_target = project_root.join(&path_str);
            if !full_target.exists() {
                println!(
                    "WARNING (Orphan Reference): In '{}' -> referenced file '{}' does not exist on disk",
                    doc_filename, path_str
                );
                warnings_count += 1;
            } else {
                file_touch_map
                    .entry(path_str)
                    .or_default()
                    .push((doc_filename.clone(), doc_title.clone()));
            }
        }
    }

    // 3. Check conflicting decisions (multiple decisions referencing the same file)
    for (path_str, touched_docs) in file_touch_map {
        if touched_docs.len() > 1 {
            let doc_names: Vec<String> = touched_docs
                .iter()
                .map(|(fname, _)| fname.clone())
                .collect();
            println!(
                "WARNING (Potential Conflict): Multiple memory decisions touch '{}': [{}]",
                path_str,
                doc_names.join(", ")
            );
            warnings_count += 1;
        }
    }

    (has_errors, warnings_count)
}
