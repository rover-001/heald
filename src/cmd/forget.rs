use chrono::Utc;
use std::io::Write;
use std::path::PathBuf;

pub fn run(query: &str, yes: bool) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    let memory_dir = local_base.join("memory");

    if !memory_dir.exists() {
        eprintln!("ERROR (Heald): No memory directory found. Run `heald context agents` to see available memory docs.");
        std::process::exit(1);
    }

    if query == "index" {
        eprintln!("ERROR (Heald): Refusing to delete index.md. Please edit it directly.");
        std::process::exit(1);
    }

    let mut exact_match: Option<(PathBuf, String, String)> = None;
    let mut fuzzy_matches = Vec::new();

    let walker = walkdir::WalkDir::new(&memory_dir).into_iter().filter_map(|e| e.ok());
    for entry in walker {
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            let file_stem = entry.path().file_stem().unwrap().to_str().unwrap();
            
            if file_stem == "log" || file_stem == "index" {
                continue;
            }

            if let Ok(doc) = crate::okf::Document::from_file(entry.path()) {
                let title = doc.frontmatter.effective_title().to_string();
                let first_line = doc.content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();

                if file_stem == query {
                    exact_match = Some((entry.path().to_path_buf(), title, first_line));
                    break;
                }

                if title.to_lowercase().contains(&query.to_lowercase()) {
                    fuzzy_matches.push((entry.path().to_path_buf(), file_stem.to_string(), title, first_line));
                }
            }
        }
    }

    let (target_path, target_title, target_first_line) = if let Some(exact) = exact_match {
        exact
    } else if fuzzy_matches.len() == 1 {
        let m = fuzzy_matches.pop().unwrap();
        (m.0, m.2, m.3)
    } else if fuzzy_matches.len() > 1 {
        eprintln!("Multiple matches found for \"{}\". Please be more specific:", query);
        for m in fuzzy_matches {
            eprintln!("  - {} (slug: {})", m.2, m.1);
        }
        std::process::exit(1);
    } else {
        eprintln!("No memory document found matching \"{}\".", query);
        eprintln!("Run `heald context agents` to see available memory docs.");
        std::process::exit(1);
    };

    if !yes {
        println!("Document: {}", target_title);
        println!("Preview: {}", target_first_line);
        print!("Are you sure you want to forget this? [y/N] ");
        std::io::stdout().flush().unwrap();
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            std::process::exit(0);
        }
    }

    std::fs::remove_file(&target_path).unwrap();

    let log_path = memory_dir.join("log.md");
    let timestamp = Utc::now().to_rfc3339();
    let log_entry = format!("Forgot: \"{}\" ({})\n", target_title, timestamp);
    
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        file.write_all(log_entry.as_bytes()).unwrap();
    }
    
    crate::xref::rebuild_memory_index(&local_base);

    println!("Forgot \"{}\" (deleted {})", target_title, target_path.file_name().unwrap().to_str().unwrap());
}
