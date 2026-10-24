use crate::xref::{find_references, get_memory_docs, normalize_path};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct DirStat {
    file_count: usize,
    size: u64,
}

fn build_tree(root: &Path) -> (Vec<PathBuf>, BTreeMap<PathBuf, DirStat>) {
    let mut files = Vec::new();
    let mut dir_stats: BTreeMap<PathBuf, DirStat> = BTreeMap::new();

    let walker = ignore::WalkBuilder::new(root).hidden(false).build();

    for entry in walker.flatten() {
        let path = entry.path();

        // exclude .git and .heald manually if needed
        let path_str = path.to_string_lossy();
        if path_str.contains(".git") || path_str.contains(".heald") {
            continue;
        }

        if path.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(path.to_path_buf());

            // Add stats to all parents
            let mut p = path.parent();
            while let Some(parent) = p {
                let stat = dir_stats.entry(parent.to_path_buf()).or_default();
                stat.file_count += 1;
                stat.size += size;
                if parent == root {
                    break;
                }
                p = parent.parent();
            }
        }
    }
    files.sort();
    (files, dir_stats)
}

fn is_entry_point(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(
        name,
        "main.rs" | "index.ts" | "index.js" | "app.py" | "main.go" | "main.py" | "lib.rs"
    )
}

pub fn run() {
    let root = std::env::current_dir().unwrap_or_default();
    let local_base = root.join(".heald");

    // docs
    let docs = get_memory_docs(&local_base);

    let (files, dir_stats) = build_tree(&root);

    let mut output = String::new();
    output.push_str("---\n");
    output.push_str("type: map\n");
    output.push_str("---\n\n");
    output.push_str("# Repository Map\n\n");

    let mut last_rendered_dirs: Vec<PathBuf> = Vec::new();

    for file in &files {
        if output.len() > 6000 {
            output.push_str("\n... tree truncated due to size limits ...\n");
            break;
        }

        let rel = file.strip_prefix(&root).unwrap_or(file);
        let rel_str = normalize_path(&rel.to_string_lossy());

        let parent = rel.parent().unwrap_or(Path::new(""));
        let depth = parent.components().count();
        if depth > 3 {
            // Skip files deeper than 3, just let the dir summary handle it.
            // Wait, we need to show the dir summary.
            continue;
        }

        // Render directories if we haven't
        let mut current_ancestors = Vec::new();
        let mut p = Some(parent);
        while let Some(anc) = p {
            if anc.as_os_str().is_empty() {
                break;
            }
            current_ancestors.push(anc.to_path_buf());
            p = anc.parent();
        }
        current_ancestors.reverse();

        for (i, anc) in current_ancestors.iter().enumerate() {
            if i >= last_rendered_dirs.len() || last_rendered_dirs[i] != *anc {
                let indent = "  ".repeat(i);
                let name = anc.file_name().unwrap_or_default().to_string_lossy();
                let full_anc = root.join(anc);
                let stat = dir_stats
                    .get(&full_anc)
                    .map(|s| format!("({} files, {} bytes)", s.file_count, s.size))
                    .unwrap_or_default();

                output.push_str(&format!("{}- {}/ {}\n", indent, name, stat));
            }
        }
        last_rendered_dirs = current_ancestors;

        let indent = "  ".repeat(depth);
        let name = file.file_name().unwrap_or_default().to_string_lossy();

        let mut annotations = Vec::new();
        if is_entry_point(file) {
            annotations.push("ENTRY POINT".to_string());
        }

        let refs = find_references(&docs, &rel_str);
        for r in refs {
            annotations.push(format!("see {}", r.doc_filename));
        }

        let ann_str = if annotations.is_empty() {
            "".to_string()
        } else {
            format!(" — {}", annotations.join(", "))
        };

        output.push_str(&format!("{}- {}{}\n", indent, name, ann_str));
    }

    let map_file = local_base.join("map.md");
    if local_base.exists() {
        fs::write(&map_file, output).unwrap_or(());
    }
}
