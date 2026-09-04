use std::collections::HashMap;
use crate::bm25::{score_bm25, tokenize};
use crate::okf::Document;

pub fn run(_harness: &str, budget: Option<usize>, query: Option<&str>) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    
    // Auto-init if local workspace doesn't exist
    if !local_base.exists() {
        crate::cmd::init::run(false);
    } else {
        crate::xref::rebuild_memory_index(&local_base);
    }

    let memory_dir = local_base.join("memory");

    let mut docs = Vec::new();
    let mut index_doc = None;

    if memory_dir.exists() {
        let walker = walkdir::WalkDir::new(&memory_dir)
            .into_iter()
            .filter_map(|e| e.ok());
        for entry in walker {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                // Ignore files in archive directory
                if path.components().any(|c| c.as_os_str() == "archive") {
                    continue;
                }
                if let Ok(doc) = Document::from_file(path) {
                    if path.file_name().and_then(|n| n.to_str()) == Some("index.md") {
                        index_doc = Some(doc);
                    } else if path.file_name().and_then(|n| n.to_str()) != Some("log.md") {
                        docs.push(doc);
                    }
                }
            }
        }
    }

    if let Some(q) = query {
        let q_tokens = tokenize(q);
        if !q_tokens.is_empty() {
            // Compute document tokens and corpus statistics for BM25
            let doc_tokens_list: Vec<Vec<String>> = docs
                .iter()
                .map(|d| {
                    let mut text = String::new();
                    if let Some(title) = &d.frontmatter.title {
                        text.push_str(title);
                        text.push(' ');
                    }
                    if let Some(tags) = &d.frontmatter.tags {
                        text.push_str(&tags.join(" "));
                        text.push(' ');
                    }
                    text.push_str(&d.content);
                    tokenize(&text)
                })
                .collect();

            let total_docs = docs.len();
            let total_len: usize = doc_tokens_list.iter().map(|dt| dt.len()).sum();
            let avg_doc_len = if total_docs > 0 {
                total_len as f64 / total_docs as f64
            } else {
                1.0
            };

            let mut doc_freqs: HashMap<String, usize> = HashMap::new();
            for dt in &doc_tokens_list {
                let mut unique = std::collections::HashSet::new();
                for t in dt {
                    if unique.insert(t.clone()) {
                        *doc_freqs.entry(t.clone()).or_insert(0) += 1;
                    }
                }
            }

            // Score each doc
            let mut scored_docs: Vec<(f64, i32, Document)> = docs
                .into_iter()
                .enumerate()
                .map(|(i, d)| {
                    let bm25 = score_bm25(
                        &q_tokens,
                        &doc_tokens_list[i],
                        doc_tokens_list[i].len(),
                        avg_doc_len,
                        &doc_freqs,
                        total_docs,
                    );
                    let base_s = score(&d);
                    (bm25, base_s, d)
                })
                .collect();

            // Sort by relevance score: docs with BM25 > 0 come first ordered by BM25,
            // docs with BM25 == 0 come after ordered by base score.
            scored_docs.sort_by(|a, b| {
                let a_has_match = a.0 > 0.0001;
                let b_has_match = b.0 > 0.0001;
                match (b_has_match, a_has_match) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    (true, true) => b.0.partial_cmp(&a.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.1.cmp(&a.1)),
                    (false, false) => b.1.cmp(&a.1),
                }
            });

            docs = scored_docs.into_iter().map(|(_, _, d)| d).collect();

        } else {
            docs.sort_by_key(|d| -score(d));
        }
    } else {
        docs.sort_by_key(|d| -score(d));
    }

    let budget_chars = budget.unwrap_or(8000) * 4;
    let mut current_chars = 0;
    let mut included = Vec::new();

    if let Some(idx) = index_doc {
        let len = idx.content.len();
        current_chars += len;
        included.push(idx);
    }

    let total_available = docs.len() + if included.is_empty() { 0 } else { 1 };

    for doc in docs {
        let len = doc.content.len();
        if current_chars + len <= budget_chars {
            current_chars += len;
            included.push(doc);
        }
    }

    println!(
        "<!-- Heald Context: Included {}/{} memory documents (Budget: {} chars{}) -->",
        included.len(),
        total_available,
        budget_chars,
        query.map(|q| format!(", Query: \"{}\"", q)).unwrap_or_default()
    );
    for doc in included {
        if let Some(title) = &doc.frontmatter.title {
            println!("\n# {}\n\n{}", title, doc.content);
        } else {
            println!("\n# {}\n\n{}", doc.frontmatter.r#type, doc.content);
        }
    }

    // Include Learned Code Navigation Guide
    if let Some(kb) = crate::cmd::learn::load_knowledge(&local_base) {
        println!("\n# Learned Code Navigation & Feature Index\n");
        if let Some(q) = query {
            let matches = crate::cmd::learn::query_knowledge(&kb, q);
            if !matches.is_empty() {
                for (feat, matched_files, _) in matches.into_iter().take(2) {
                    println!("### Feature: {}", feat.name);
                    for (file, syms) in matched_files.into_iter().take(4) {
                        println!("- `{}` ({})", file.path, file.layer);
                        let top_syms = if !syms.is_empty() { syms } else { file.symbols.iter().collect() };
                        for sym in top_syms.into_iter().take(3) {
                            println!("  * `{}` [{}] (L{})", sym.name, sym.kind, sym.line);
                        }
                    }
                    println!();
                }
            } else {
                for feat in kb.features.iter().take(6) {
                    let files: Vec<String> = feat.files.iter().take(2).map(|f| f.path.clone()).collect();
                    println!("- **{}**: {}", feat.name, files.join(", "));
                }
            }
        } else {
            println!("*(Run `heald learn <feature>` to inspect exact functions and line numbers before modifying code)*\n");
            for feat in kb.features.iter().take(8) {
                let files: Vec<String> = feat.files.iter().take(3).map(|f| f.path.clone()).collect();
                println!("- **{}**: {}", feat.name, files.join(", "));
            }
        }
    }
}

fn score(doc: &Document) -> i32 {
    let mut s = 0;
    if let Some(tags) = &doc.frontmatter.tags {
        if tags.iter().any(|t| t == "pinned") {
            s += 1000;
        }
    }
    if let Some(ts) = &doc.frontmatter.timestamp {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
            let days = (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days();
            s += (100 - days).max(0) as i32;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_pinned() {
        let doc_pinned = Document {
            frontmatter: crate::okf::Frontmatter {
                r#type: "decision".to_string(),
                title: None,
                description: None,
                resource: None,
                tags: Some(vec!["pinned".to_string()]),
                timestamp: None,
                name: None,
                triggers: None,
            },
            content: "test".to_string(),
        };
        let doc_unpinned = Document {
            frontmatter: crate::okf::Frontmatter {
                r#type: "decision".to_string(),
                title: None,
                description: None,
                resource: None,
                tags: None,
                timestamp: None,
                name: None,
                triggers: None,
            },
            content: "test".to_string(),
        };
        assert!(score(&doc_pinned) > score(&doc_unpinned) + 500);
    }
}

