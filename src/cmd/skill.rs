use crate::okf::Document;
use std::path::{Path, PathBuf};

// When main calls cmd::skill::run, it passes SkillCommands from crate::SkillCommands
pub fn run(cmd: &crate::SkillCommands) {
    match cmd {
        crate::SkillCommands::List { global, local } => list_skills(*global, *local),
        crate::SkillCommands::Search { query } => search_skills(query),
        crate::SkillCommands::Install {
            source,
            name,
            global,
        } => install_skill(source, name.as_deref(), *global),
        crate::SkillCommands::Info { name } => show_skill_info(name),
    }
}

pub struct SkillEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_global: bool,
    pub doc: Document,
}

pub fn load_installed_skills(include_global: bool, include_local: bool) -> Vec<SkillEntry> {
    let mut skills = Vec::new();
    let global_base = dirs::home_dir()
        .unwrap_or_default()
        .join(".heald")
        .join("skills");
    let local_base = std::env::current_dir()
        .unwrap_or_default()
        .join(".heald")
        .join("skills");

    if include_global && global_base.exists() {
        collect_skills_from_dir(&global_base, true, &mut skills);
    }
    if include_local && local_base.exists() {
        collect_skills_from_dir(&local_base, false, &mut skills);
    }

    skills.sort_by_key(|a| a.name.to_lowercase());
    skills
}

fn collect_skills_from_dir(dir: &Path, is_global: bool, results: &mut Vec<SkillEntry>) {
    let walker = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok());
    for entry in walker {
        let p = entry.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "md" || ext == "mdc") {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.eq_ignore_ascii_case("index") || stem.eq_ignore_ascii_case("log") {
                continue;
            }
            if let Ok(doc) = Document::from_file(p) {
                let name = doc.frontmatter.effective_title().to_string();
                results.push(SkillEntry {
                    name,
                    path: p.to_path_buf(),
                    is_global,
                    doc,
                });
            }
        }
    }
}

pub fn list_skills(only_global: bool, only_local: bool) {
    let (inc_global, inc_local) = match (only_global, only_local) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        _ => (true, true),
    };

    let skills = load_installed_skills(inc_global, inc_local);
    if skills.is_empty() {
        println!("No skills found. Run `heald skill install <path_or_content>` to install one.");
        return;
    }

    println!(
        "{:<24} {:<8} {:<40}",
        "SKILL", "SCOPE", "DESCRIPTION / TRIGGERS"
    );
    println!("{:-<24} {:-<8} {:-<40}", "", "", "");

    for skill in skills {
        let scope = if skill.is_global { "global" } else { "local" };
        let triggers = skill
            .doc
            .frontmatter
            .triggers
            .as_ref()
            .map(|t| t.join(", "))
            .or_else(|| skill.doc.frontmatter.description.clone())
            .unwrap_or_else(|| "-".to_string());

        let desc_display = if triggers.len() > 50 {
            format!("{}...", &triggers[..47])
        } else {
            triggers
        };

        println!("{:<24} {:<8} {:<40}", skill.name, scope, desc_display);
    }
}

pub fn search_skills(query: &str) {
    let query_lower = query.to_lowercase();
    let skills = load_installed_skills(true, true);

    let matches: Vec<&SkillEntry> =
        skills
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.doc
                        .frontmatter
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
                    || s.doc.frontmatter.triggers.as_ref().is_some_and(|tr| {
                        tr.iter().any(|t| t.to_lowercase().contains(&query_lower))
                    })
                    || s.doc.content.to_lowercase().contains(&query_lower)
            })
            .collect();

    if matches.is_empty() {
        println!("No skills matching \"{}\".", query);
        return;
    }

    println!("Found {} skill(s) matching \"{}\":\n", matches.len(), query);
    for s in matches {
        let scope = if s.is_global { "global" } else { "local" };
        let triggers = s
            .doc
            .frontmatter
            .triggers
            .as_ref()
            .map(|t| t.join(", "))
            .or_else(|| s.doc.frontmatter.description.clone())
            .unwrap_or_else(|| "-".to_string());
        println!("• {} ({})", s.name, scope);
        println!("  File:     {}", s.path.display());
        println!("  Triggers: {}", triggers);
        println!();
    }
}

pub fn show_skill_info(name: &str) {
    let name_lower = name.to_lowercase();
    let skills = load_installed_skills(true, true);

    let found = skills.iter().find(|s| {
        let stem = s
            .path
            .file_stem()
            .and_then(|st| st.to_str())
            .unwrap_or("")
            .to_lowercase();
        s.name.to_lowercase() == name_lower || stem == name_lower
    });

    match found {
        Some(s) => {
            println!("Skill:       {}", s.name);
            println!(
                "Location:    {} ({})",
                s.path.display(),
                if s.is_global { "global" } else { "local" }
            );
            if let Some(desc) = &s.doc.frontmatter.description {
                println!("Description: {}", desc);
            }
            if let Some(triggers) = &s.doc.frontmatter.triggers {
                println!("Triggers:    {}", triggers.join(", "));
            }
            if let Some(tags) = &s.doc.frontmatter.tags {
                println!("Tags:        {}", tags.join(", "));
            }
            println!("\n--- Content ---\n{}", s.doc.content);
        }
        None => {
            eprintln!(
                "Skill \"{}\" not found. Run `heald skill list` to view available skills.",
                name
            );
            std::process::exit(1);
        }
    }
}

pub fn install_skill(source: &str, name_opt: Option<&str>, is_global: bool) {
    let target_dir = if is_global {
        let p = dirs::home_dir()
            .unwrap_or_default()
            .join(".heald")
            .join("skills");
        std::fs::create_dir_all(&p).unwrap();
        p
    } else {
        let p = std::env::current_dir()
            .unwrap_or_default()
            .join(".heald")
            .join("skills");
        std::fs::create_dir_all(&p).unwrap();
        p
    };

    let (raw_content, inferred_name) = if source.len() < 260
        && Path::new(source).exists()
        && Path::new(source).is_file()
    {
        let source_path = Path::new(source);
        let content = std::fs::read_to_string(source_path).unwrap_or_else(|e| {
            eprintln!("ERROR: Failed to read source file {}: {}", source, e);
            std::process::exit(1);
        });
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("skill")
            .to_string();
        (content, stem)
    } else if source.starts_with("http://") || source.starts_with("https://") {
        eprintln!(
            "ERROR: Remote URL downloading is not currently configured. Please provide a local file or raw markdown content."
        );
        std::process::exit(1);
    } else {
        // Raw content passed directly
        let stem = name_opt.unwrap_or("custom-skill").to_string();
        (source.to_string(), stem)
    };

    let final_name = name_opt.map(|s| s.to_string()).unwrap_or(inferred_name);
    let slug = final_name
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>();

    let content_to_write = if raw_content.trim_start().starts_with("---") {
        raw_content
    } else {
        format!(
            "---\ntype: skill\ntitle: \"{}\"\ndescription: \"Skill: {}\"\n---\n\n{}",
            final_name, final_name, raw_content
        )
    };

    let dest = target_dir.join(format!("{}.md", slug));
    if let Err(e) = std::fs::write(&dest, content_to_write) {
        eprintln!("ERROR: Failed to save skill to {}: {}", dest.display(), e);
        std::process::exit(1);
    }

    println!(
        "Installed skill '{}' to {} ({})",
        final_name,
        dest.display(),
        if is_global { "global" } else { "local" }
    );

    // Trigger sync to update AGENTS.md routing table
    crate::cmd::sync::run(None, true);
}
