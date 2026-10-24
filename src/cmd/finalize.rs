use chrono::Utc;
use std::io::Read;

pub fn run(summary: Option<&str>, stdin: bool) {
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");
    
    // Auto-init if local workspace doesn't exist
    if !local_base.exists() {
        crate::cmd::init::run(false);
    }
    
    let memory_dir = local_base.join("memory");
    let log_path = memory_dir.join("log.md");

    let mut body = String::new();
    if let Some(s) = summary {
        body = s.to_string();
    } else if stdin {
        std::io::stdin().read_to_string(&mut body).unwrap_or_default();
    }

    if body.trim().is_empty() {
        eprintln!("ERROR (Heald): You must provide a summary! Example: heald finalize --summary \"What I did...\"");
        std::process::exit(1);
    }

    let timestamp = Utc::now().to_rfc3339();
    let entry = format!("\n## Session {}\n\n{}\n", timestamp, body);

    if log_path.exists() {
        let mut content = std::fs::read_to_string(&log_path).unwrap_or_default();
        content.push_str(&entry);
        std::fs::write(&log_path, content).unwrap();
    } else {
        std::fs::create_dir_all(&memory_dir).unwrap();
        std::fs::write(&log_path, format!("---\ntype: log\n---\n# Memory Log\n{}", entry)).unwrap();
    }
    
    // 1. Auto-learn: associate modified files from this session with the summary
    crate::cmd::learn::auto_learn_from_git(&body);

    // 2. Auto-clean: if memory docs exceed threshold (20), cluster and consolidate automatically
    crate::cmd::clean::run_clean(true, false, false, Some(20));

    println!("Session log finalized.");
}
