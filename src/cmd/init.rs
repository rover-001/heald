pub fn run(global: bool) {
    let global_base = dirs::home_dir().unwrap_or_default().join(".heald");
    let local_base = std::env::current_dir().unwrap_or_default().join(".heald");

    // Always create global structure
    std::fs::create_dir_all(global_base.join("rules")).unwrap();
    std::fs::create_dir_all(global_base.join("skills")).unwrap();

    // Create local structure if we are not explicitly ONLY doing global
    if !global {
        std::fs::create_dir_all(local_base.join("rules")).unwrap();
        std::fs::create_dir_all(local_base.join("skills")).unwrap();
        let memory_dir = local_base.join("memory");
        std::fs::create_dir_all(&memory_dir).unwrap();
        
        let log_path = memory_dir.join("log.md");
        if !log_path.exists() {
            std::fs::write(&log_path, "---\ntype: log\n---\n# Memory Log\n").unwrap();
        }
        crate::xref::rebuild_memory_index(&local_base);
        println!("Initialized Heald locally at {}", local_base.display());
    }
    
    println!("Initialized Heald globally at {}", global_base.display());

    // Import existing skills to global
    let global_skills_dir = global_base.join("skills");
    let _ = std::fs::create_dir_all(&global_skills_dir);
    import_skills(&global_skills_dir);

    // Generate all harness-specific files
    crate::cmd::sync::run(None, true);

    // Map all resources
    crate::cmd::map::run();

    // Inject critical hook into every global agent config file
    let global_configs: Vec<std::path::PathBuf> = if let Some(home) = dirs::home_dir() {
        vec![
            home.join(".gemini").join("config").join("AGENTS.md"),
            home.join(".agents").join("AGENTS.md"),
            home.join(".hermes").join("AGENTS.md"),
            home.join(".claude.md"),
            home.join(".cursor").join("rules").join("heald.mdc"),
        ]
    } else {
        vec![]
    };

    let global_heald_path = global_base.to_str().unwrap_or("").replace('\\', "/");
    let hook_text = format!(
        "\n\n## CRITICAL HEALD INSTRUCTIONS\n\
         Your universal rules and skill routing table are in `{}/AGENTS.md` — read it at the start of every session.\n\
         You MUST run `heald context agents` ONCE as your very first tool call in a new session/conversation to load project memory and learned code navigation.\n\
         You CAN run `heald learn <feature-or-query>` to immediately identify which files and functions to modify WITHOUT rereading the repository.\n\
         You MUST run `heald remember --type decision --title '...' --body '...'` ONLY when settling major architectural choices (stack, schemas, contracts). DO NOT record routine bugfixes or version bumps.\n\
         You CAN run `heald clean` to consolidate related decisions and prune memory.\n\
         You CAN run `heald forget <slug-or-title>` to remove a memory document if it becomes outdated or incorrect.\n\
         You CAN run `heald map` to see a fast repository map and cross-references.\n\
         You CAN run `heald blame <path>` to see which memory decisions touched a given file.\n\
         You MUST run `heald finalize --summary '...'` as your very last tool call before finishing the session to record your summary and auto-learn touched files.\n",
        global_heald_path
    );

    for config_path in global_configs {
        if config_path.exists() {
            let current = std::fs::read_to_string(&config_path).unwrap_or_default();
            if current.contains("## Heald Instructions") {
                let new_content = current.replace(
                    "## Heald Instructions\nRun `heald context agents` at the start of a task to get context.\nRun `heald finalize` near the end of a task to save context.\n",
                    &hook_text
                );
                let _ = std::fs::write(&config_path, new_content);
                println!("Upgraded Heald hook in {}", config_path.display());
            } else if !current.contains("## CRITICAL HEALD INSTRUCTIONS") {
                let _ = std::fs::write(&config_path, format!("{}{}", current, hook_text));
                println!("Appended Heald hook to {}", config_path.display());
            }
        } else {
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&config_path, &hook_text);
            println!("Created {} with Heald hook", config_path.display());
        }
    }
}

fn import_skills(global_skills_dir: &std::path::Path) {
    let mut search_paths = vec![];
    
    // Global paths
    if let Some(home) = dirs::home_dir() {
        search_paths.push(home.join(".gemini").join("config").join("skills"));
        search_paths.push(home.join(".agents").join("skills"));
    }
    
    // Local paths
    if let Ok(cwd) = std::env::current_dir() {
        search_paths.push(cwd.join(".agents").join("skills"));
        search_paths.push(cwd.join(".cursor").join("rules"));
    }

    for path in search_paths {
        if !path.exists() { continue; }
        let walker = walkdir::WalkDir::new(&path).into_iter().filter_map(|e| e.ok());
        for entry in walker {
            let p = entry.path();
            if !p.is_file() { continue; }
            
            let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            
            // Folder-based skills (SkillName/SKILL.md)
            if file_name == "SKILL.md" || file_name == "skill.md" {
                if let Some(parent) = p.parent()
                    && let Some(skill_name) = parent.file_name().and_then(|n| n.to_str()) {
                        let dest = global_skills_dir.join(format!("{}.md", skill_name));
                        if !dest.exists() {
                            let raw = std::fs::read_to_string(p).unwrap_or_default();
                            let wrapped = ensure_okf_frontmatter(&raw, skill_name);
                            let _ = std::fs::write(&dest, wrapped);
                            println!("Imported skill '{}' into global Heald store.", skill_name);
                        }
                    }
            } 
            // Flat file rules (.mdc or .md)
            else if (ext == "mdc" || ext == "md")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let upper = stem.to_uppercase();
                    if upper == "AGENTS" || upper == "CLAUDE" || upper == "README" {
                        continue;
                    }
                    let dest = global_skills_dir.join(format!("{}.md", stem));
                    if !dest.exists() {
                        let raw = std::fs::read_to_string(p).unwrap_or_default();
                        let wrapped = ensure_okf_frontmatter(&raw, stem);
                        let _ = std::fs::write(&dest, wrapped);
                        println!("Imported skill/rule '{}' into global Heald store.", stem);
                    }
                }
        }
    }
}

/// Ensure a skill file has valid OKF frontmatter.
/// If the file already has `---` frontmatter, leave it alone.
/// Otherwise, wrap it with inferred metadata including auto-detected triggers.
fn ensure_okf_frontmatter(raw: &str, skill_name: &str) -> String {
    if raw.trim_start().starts_with("---") {
        // File already has frontmatter — leave content alone, just return it.
        return raw.to_string();
    }
    // Infer triggers from common skill name patterns
    let triggers = infer_triggers(skill_name);
    let triggers_yaml = if triggers.is_empty() {
        String::new()
    } else {
        format!("triggers: [{}]\n", triggers.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(", "))
    };
    format!(
        "---\ntype: skill\ntitle: \"{}\"\ndescription: \"Skill: {}\"\n{}---\n\n{}",
        skill_name,
        skill_name,
        triggers_yaml,
        raw
    )
}

/// Infer routing triggers from a skill's name.
/// This is a best-effort heuristic — users can override by adding
/// `triggers:` to their skill's frontmatter directly.
fn infer_triggers(name: &str) -> Vec<&'static str> {
    let lower = name.to_lowercase();
    let mut t = vec![];
    if lower.contains("theme") || lower.contains("style") || lower.contains("ui") || lower.contains("css") {
        t.extend(["UI", "styling", "visual", "component", "page design"]);
    }
    if lower.contains("ux") || lower.contains("flow") || lower.contains("form") {
        t.extend(["user flow", "forms", "navigation", "layout"]);
    }
    if lower.contains("backend") || lower.contains("api") || lower.contains("server") {
        t.extend(["API", "endpoints", "auth", "backend logic"]);
    }
    if lower.contains("database") || lower.contains("db") || lower.contains("schema") || lower.contains("migration") {
        t.extend(["schema", "migration", "queries", "indexes"]);
    }
    if lower.contains("security") || lower.contains("auth") {
        t.extend(["security", "secrets", "authentication", "user input"]);
    }
    if lower.contains("test") {
        t.extend(["testing", "coverage", "unit tests"]);
    }
    if lower.contains("git") || lower.contains("commit") || lower.contains("pr") {
        t.extend(["committing", "branching", "pull request"]);
    }
    if lower.contains("deploy") || lower.contains("ci") || lower.contains("k8s") || lower.contains("docker") {
        t.extend(["CI/CD", "deployment", "Docker", "Kubernetes"]);
    }
    if lower.contains("folder") || lower.contains("structure") || lower.contains("scaffold") {
        t.extend(["new project", "scaffolding", "folder structure"]);
    }
    t
}
