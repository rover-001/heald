use crate::xref::{find_references, get_memory_docs, normalize_path};
use serde_json::json;
use std::path::Path;

pub fn run(path: &str, is_json: bool) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let docs = get_memory_docs(&local_base);

    let target_path = Path::new(path);
    let mut files_to_check = Vec::new();

    if target_path.is_dir() {
        let walker = ignore::WalkBuilder::new(target_path).build();
        for result in walker {
            if let Ok(entry) = result
                && entry.path().is_file()
            {
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
        let norm = normalize_path(&file_str);
        let refs = find_references(&docs, &norm);
        if !refs.is_empty() {
            total_refs += refs.len();
            all_refs.insert(norm, refs);
        }
    }

    if is_json {
        let mut json_out = serde_json::Map::new();
        for (file, refs) in all_refs {
            let mut refs_arr = Vec::new();
            for r in refs {
                refs_arr.push(json!({
                    "title": r.title,
                    "timestamp": r.timestamp,
                    "snippet": r.snippet,
                    "doc_filename": r.doc_filename,
                }));
            }
            json_out.insert(file, serde_json::Value::Array(refs_arr));
        }
        println!("{}", serde_json::to_string_pretty(&json_out).unwrap());
        return;
    }

    if total_refs == 0 {
        println!("No decisions on record for {}", path);
        return;
    }

    for (file, refs) in all_refs {
        println!("{}", file);
        for r in refs {
            println!(
                "  {:<20} {:<10} \"{}\"",
                r.doc_filename, r.timestamp, r.snippet
            );
        }
    }
}
