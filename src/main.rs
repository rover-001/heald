use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "heald")]
#[command(version)]
#[command(about = "Canonical AI coding harness rules and memory store", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold .heald/ (project) or ~/.heald/ (global)
    Init {
        #[arg(long)]
        global: bool,
    },
    /// Compile canonical rules/skills into harness-specific format
    Sync {
        #[arg(long)]
        harness: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Print assembled, pruned memory context
    Context {
        harness: String,
        #[arg(long)]
        budget: Option<usize>,
        #[arg(short = 'q', long = "relevant", aliases = ["query"])]
        query: Option<String>,
        #[arg(short = 'c', long = "compact")]
        compact: bool,
    },
    /// Append a new memory document
    Remember {
        #[arg(long)]
        r#type: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,
        #[arg(long)]
        stdin: bool,
    },
    /// Run end-of-session consolidation
    Finalize {
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        stdin: bool,
    },
    /// Consolidate and archive old memory documents and deduplicate session logs
    Compact {
        #[arg(long)]
        dry_run: bool,
    },
    /// Consolidate related memory documents, merge topic clusters, and prune trivial notes
    Clean {
        #[arg(long)]
        auto: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        threshold: Option<usize>,
    },
    /// Self-learn and navigate codebase features, files, and functions
    Learn {
        query: Option<String>,
        #[arg(long)]
        scan: bool,
        #[arg(long)]
        record: bool,
        #[arg(long)]
        feature: Option<String>,
        #[arg(long, value_delimiter = ',')]
        files: Option<Vec<String>>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Validate bundle integrity
    Doctor,
    /// Forget a memory document
    Forget {
        query: String,
        #[arg(short, long)]
        yes: bool,
    },
    /// Show memory docs that touched a given file
    Blame {
        path: String,
        #[arg(long)]
        json: bool,
    },
    /// Generate repo map
    Map,
    /// Run JSON-RPC 2.0 / Model Context Protocol (MCP) server over stdio
    Mcp,
    /// Run MCP server over stdio (alias for mcp)
    Serve,
    /// Manage global and local skills
    #[command(subcommand)]
    Skill(SkillCommands),
}

#[derive(Subcommand, Clone, Debug)]
pub enum SkillCommands {
    /// List all installed skills (global and local)
    List {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        local: bool,
    },
    /// Search installed skills by keyword or trigger
    Search {
        query: String,
    },
    /// Install or add a skill from a local file or markdown text
    #[command(alias = "add")]
    Install {
        #[arg(allow_hyphen_values = true)]
        source: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        global: bool,
    },
    /// Show detailed info and content of a skill
    Info {
        name: String,
    },
}

pub mod bm25;
pub mod cmd;
pub mod okf;
pub mod xref;

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Init { global } => cmd::init::run(*global),
        Commands::Sync { harness, all } => cmd::sync::run(harness.as_deref(), *all),
        Commands::Context { harness, budget, query, compact } => cmd::context::run(harness, *budget, query.as_deref(), *compact),
        Commands::Remember { r#type, title, body, tags, stdin } => {
            cmd::remember::run(r#type, title, body.as_deref(), tags.as_deref(), *stdin)
        }
        Commands::Finalize { summary, stdin } => cmd::finalize::run(summary.as_deref(), *stdin),
        Commands::Compact { dry_run } => cmd::compact::run(*dry_run),
        Commands::Clean { auto, dry_run, force, threshold } => {
            cmd::clean::run(*auto, *dry_run, *force, *threshold)
        }
        Commands::Learn { query, scan, record, feature, files, notes, json } => {
            cmd::learn::run(query.as_deref(), *scan, *record, feature.as_deref(), files.as_deref(), notes.as_deref(), *json)
        }
        Commands::Doctor => cmd::doctor::run(),
        Commands::Forget { query, yes } => cmd::forget::run(query, *yes),
        Commands::Blame { path, json } => cmd::blame::run(path, *json),
        Commands::Map => cmd::map::run(),
        Commands::Mcp | Commands::Serve => cmd::mcp::run(),
        Commands::Skill(skill_cmd) => cmd::skill::run(skill_cmd),
    }
}

