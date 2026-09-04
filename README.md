<p align="center">
  <img src="assets/heald-thumbnail.png" alt="Heald — One memory. Every AI agent." width="100%">
</p>

<p align="center">
  <strong>One memory. Every AI agent.</strong><br>
  A single-binary CLI that keeps Antigravity, Claude Code, Cursor, Hermes, and Codex all reading from the same rules and project memory — automatically.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/built_with-Rust-orange?style=flat-square&logo=rust" alt="Built with Rust">
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="MIT License">
  <img src="https://img.shields.io/badge/version-0.4.0-teal?style=flat-square" alt="v0.4.0">
  <img src="https://img.shields.io/badge/MCP-JSON--RPC%202.0-purple?style=flat-square" alt="Model Context Protocol">
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey?style=flat-square" alt="Cross-platform">
</p>

---

## The Problem

You work across multiple AI coding agents. You make a decision in Antigravity, switch to Claude Code, and it has no idea what you just decided. So you re-explain. Then Cursor needs the same thing. You re-explain again.

Even within the same agent, every new chat starts from zero. The agent has no memory of your stack choices, architectural decisions, or project conventions.

Three problems compound each other:

1. **No context on switch** — switching harnesses loses everything. You re-explain architecture, decisions, and conventions from scratch.
2. **Context rot** — tools that *do* persist memory tend to accumulate everything forever. Bloated memory degrades agent output instead of improving it.
3. **No single source of rules** — each harness wants its own format (`CLAUDE.md`, `.cursor/rules`, `AGENTS.md`, etc.). Skills you write in one agent never reach another.

![Heald before and after comparison](assets/heald-before-after.png)

---

## The Solution

<p align="center">
  <img src="assets/how-it-works.jpg" alt="How Heald Works" width="85%">
</p>

Heald is a single Rust binary. You install it once. It acts as the shared brain between all your AI agents:

- **One `heald init`** scaffolds the local project store and injects Heald's instructions into the global config of every AI agent on your machine.
- **Skills you have** in any agent (Antigravity, Cursor, Hermes) are automatically imported into `~/.heald/skills/` on init.
- **Every compiled `AGENTS.md`** gets a live routing table pointing at those global skills by absolute path — so any agent, in any project, can load any skill without local copies.
- **`heald remember`** saves decisions into the project's memory store as plain Markdown with optional tags (`--tags`). Any agent in any harness can read them back with `heald context`.
- **BM25 Relevance Scoring & Budget-Aware Pruning** means agents get the exact pertinent subset of memory for the current task (`heald context --relevant "query"`), ranking matches dynamically while respecting token/character limits.
- **Native MCP Server (`heald mcp` / `heald serve`)** speaks JSON-RPC 2.0 over stdio so AI agents can query and record memories through structured tools.
- **Skill Management (`heald skill`)** allows listing, searching, and installing reusable skills across global and local scopes.

---

## What Heald is NOT

| ❌ Not this | ✅ But this |
|---|---|
| A vector database | Plain Markdown + YAML with fast local BM25 ranking |
| A bloated cloud service | Local-first, git-friendly, purely file-based |
| Heavy background runtime | Single fast binary or instant stdio MCP server |
| A Node/Python script | Zero runtime dependencies. Just Rust. |
| An "remember everything" dumper | BM25 relevance scoring + budget-aware recency/pinned pruning |

---

## How It Compares

| Tool | Runtime | Memory | Skills sync | Auto-routing table | BM25 Relevance | MCP Server |
|---|---|---|---|---|---|---|
| **Heald** | Single Rust binary | ✅ Plain Markdown | ✅ Auto-imported from all agents | ✅ Dynamic, from actual skills | ✅ | ✅ |
| Memorix / memsearch | Node / Python MCP server | ✅ | ❌ | ❌ | Partial | ✅ |
| ai-memory | Node + web UI | ✅ LLM-written | ❌ | ❌ | ❌ | ❌ |
| agentic-stack | Node + dashboard | ✅ | Partial | ❌ | ❌ | ❌ |

---

## Installation

### Global Installation via crates.io (recommended)

```bash
cargo install heald
```

> Requires [Rust](https://rustup.rs/) (stable). This installs `heald` globally. The binary lands in `~/.cargo/bin/heald` and is immediately available in your path.

### From source

```bash
git clone https://github.com/Parth3930/heald
cd heald
cargo install --path .
```

---

## Quick Start

### Step 1 — Initialize in your project

```bash
cd my-project
heald init
```

This does five things in one command:
- Creates `.heald/` (local memory, rules, skills store)
- Creates `~/.heald/` (global rules and skills — shared across all projects)
- Scans your existing agent skill directories (Antigravity, Cursor, Hermes, etc.) and imports them into `~/.heald/skills/`
- Generates a lightweight repository map in `.heald/map.md`
- Compiles `AGENTS.md` in your project root with a live routing table pointing to those global skills
- Injects Heald's critical instructions into every global agent config file on your machine (`~/.gemini/config/AGENTS.md`, `~/.claude.md`, `~/.cursor/rules/heald.mdc`, `~/.hermes/AGENTS.md`)

---

## Model Context Protocol (MCP) Setup

Heald has a built-in MCP server that works over `stdio` (`heald mcp` or `heald serve`).

### Configure in Claude Desktop / Cursor / Antigravity

Add `heald` to your MCP configuration file (e.g. `claude_desktop_config.json` or Antigravity MCP settings):

```json
{
  "mcpServers": {
    "heald": {
      "command": "heald",
      "args": ["mcp"]
    }
  }
}
```

### Exposed MCP Tools:
* `heald_recall (query, budget)`: Retrieve project memories, decisions, and manifest filtered by query.
* `heald_remember (type, title, body, tags)`: Record architectural decisions with structured tags.
* `heald_forget (query)`: Remove an outdated memory document.
* `heald_map ()`: Return repository structure with memory cross-references.
* `heald_blame (path)`: Find memory decisions touching a file or directory.
* `heald_doctor ()`: Run integrity diagnostics across rules, skills, and memory bundles.

---

## CLI Reference

| Command | What it does |
|---|---|
| `heald init` | Initialize local + global store, import skills from all agents, inject hooks |
| `heald sync` | Recompile `AGENTS.md` from current rules + skills (run after editing rules) |
| `heald context agents` | Print budget-pruned memory context for the current project |
| `heald context agents --relevant "auth"` | Retrieve memories ranked by BM25 query relevance |
| `heald context agents --budget 4000` | Retrieve context with a custom token budget (default: 8000 tokens) |
| `heald remember --type decision --title "..." --body "..." --tags "auth,db"` | Save a tagged memory document |
| `heald forget <slug_or_title>` | Remove or forget an outdated memory document |
| `heald compact` | Deduplicate session logs and archive superseded duplicate memories |
| `heald map` | Generate a lightweight repo map annotated with memory cross-references |
| `heald blame <path>` | Show which memory documents touched a given file |
| `heald skill list` | List all installed global and local skills with triggers |
| `heald skill search <query>` | Search skills by name, description, or trigger keyword |
| `heald skill install <path_or_text>` | Install a new skill locally or globally |
| `heald skill info <name>` | Display detailed metadata and full instructions of a skill |
| `heald doctor` | Validate markdown links, check orphan file references, and flag conflicts |
| `heald mcp` / `heald serve` | Run JSON-RPC 2.0 / Model Context Protocol server over stdio |
| `heald finalize --summary "..."` | Append session summary to the log |

---

## File Structure

```
~/.heald/                        ← Global (shared across all projects)
├── rules/                       ← Universal rules (always included in every AGENTS.md)
├── skills/                      ← Imported skills from all agents (theme.md, backend.md, etc.)
└── AGENTS.md                    ← Canonical universal reference — what every global agent hook points at

my-project/
├── AGENTS.md                    ← Compiled project context (rules + skill routing table + memory hooks)
└── .heald/
    ├── map.md                   ← Auto-generated lightweight repository map
    ├── rules/                   ← Project-specific rules
    ├── skills/                  ← Project-specific skills (usually empty; uses global)
    └── memory/
        ├── index.md             ← Always-included memory summary (pinned)
        ├── log.md               ← Session finalization log
        └── *.md                 ← Individual memory documents (decisions, context, etc.)
```

---

## Per-Agent Integration Guide

Heald's `init` command handles all of this automatically. This section is for reference or manual setup.

### Antigravity (agy)
**Global config:** `~/.gemini/config/AGENTS.md`  
Heald appends its CRITICAL INSTRUCTIONS block. Antigravity reads this on every session start.

### Claude Code
**Global config:** `~/.claude.md`  
Heald appends instructions. Claude Code reads `~/.claude.md` at the start of every conversation.

### Cursor
**Global config:** `~/.cursor/rules/heald.mdc`  
Heald creates this file. Cursor loads all `.mdc` files in `~/.cursor/rules/` globally.

### Hermes
**Global config:** `~/.hermes/AGENTS.md`  
Heald appends instructions. Hermes picks it up automatically.

### Codex
**Global config:** `~/.agents/AGENTS.md`  
Heald appends instructions to the global `.agents` format config.

### Any other agent
Point it at `~/.heald/AGENTS.md`. This is the canonical universal reference file that contains your rules, the routing table for all skills, and the memory instructions. Any agent that can read a Markdown file can use Heald.

---

## OKF — The Storage Format

Heald uses the **Open Knowledge Format (OKF)** — plain Markdown files with YAML frontmatter. No proprietary schema, no binary files, no database.

**Memory document:**
```markdown
---
type: decision
title: "Chose PostgreSQL"
timestamp: 2026-08-09T08:00:00Z
tags: [pinned]
---
We need ACID guarantees for financial transactions. MongoDB's eventual consistency
model was incompatible with our requirements.
```

**Rule file:**
```markdown
---
type: rule
title: "API Style"
---
All APIs must be RESTful. GraphQL only if the client needs field-level selection.
```

**Skill file:**
```markdown
---
type: skill
name: "backend-principles"
description: "Use for any backend/API work — building endpoints, auth, data access"
triggers: ["API", "endpoints", "auth", "backend logic"]
---
# Backend Principles
...
```

You can edit these files by hand, `git diff` them, review them in GitHub, or use them without Heald installed at all.

---

## Memory Clustering & Auto-Clean (`heald clean`)

Over time, active coding agents create dozens of micro-decisions and bugfix notes. Without consolidation, raw memory files bloat your token budget and obscure key architectural patterns.

`heald clean` uses domain-clustering and token heuristics to:
1. **Connect the dots & Merge related memories**: Automatically groups fragmented decisions (e.g. 20 disparate notes on calendar prefetching, recurring tasks, or OAuth tokens) into unified, authoritative architecture documents (`consolidated-calendar-and-recurring-tasks.md`, `consolidated-social-integrations-and-oauth.md`).
2. **Prune trivial & repealed notes**: Identifies repealed scratch experiments and micro-logs (e.g. routine version bumps, typo fixes), archiving them into `.heald/memory/archive/`.
3. **Auto-compact on finalize**: When running `heald finalize`, if active memory documents exceed 20, Heald automatically consolidates clusters to keep active context lean and fast.

```bash
# Preview consolidation and pruning without touching files
heald clean --dry-run

# Run full memory consolidation
heald clean

# Consolidate with custom threshold
heald clean --threshold 15
```

---

## Codebase Learning Engine (`heald learn`)

Eliminates the "agent reread loop" where agents reread dozens of files to locate where features, routes, hooks, and database functions live.

`heald learn` scans TypeScript, JavaScript, React/JSX, Rust, Python, and Go codebases:
- **Indexes symbols**: Exported functions, React hooks (`use*`), Convex mutations/queries, components, and routes with exact file paths and line numbers.
- **Categorizes features**: Maps subsystems (e.g. Calendar, OAuth, Inbox & DM Automation, Mascot, Database) to their primary files and entry points.
- **Learns workflows**: When an agent finishes a task via `heald finalize`, Heald auto-learns recently modified files from git diff and updates learned guidelines.
- **Provides instant navigation**: Agents can query `heald learn <feature>` to instantly receive the exact files and functions to modify.

```bash
# Force full codebase scan and symbol indexing
heald learn --scan

# Query feature navigation and symbols
heald learn calendar
heald learn oauth
heald learn inbox

# Record an explicit pattern or workflow instruction
heald learn --record --feature "OAuth" --files "src/routes/oauth.ts,src/services/auth.ts" --notes "Ensure Android intent:// fallback is preserved"
```

In addition, `heald context agents` automatically embeds the learned navigation index directly into the agent's context prompt!

---

## FAQ

**Why not just maintain `CLAUDE.md` / `AGENTS.md` by hand?**
You can, for one agent. The problem is five different agents each want their own
format and location (`~/.claude.md`, `.cursor/rules/`, `~/.gemini/config/AGENTS.md`...).
Heald keeps one source of truth and compiles/injects it into every format automatically,
so you edit rules in one place instead of five.

**What happens to memory that's not relevant anymore?**
`heald forget <title>` removes a specific memory document. Budget-aware pruning also
keeps low-value (old, unpinned) memory out of context automatically without deleting it —
so it's still on disk if you need it, just not clogging every prompt.

**Does this send my code or memory anywhere?**
No. Heald is local-first — everything lives in `.heald/` and `~/.heald/` as plain
Markdown files. No network calls, no telemetry, no cloud sync.

**What if two agents write memory at the same time?**
Known v0.1 limitation — no file locking yet on shared files (`AGENTS.md`, `log.md`).
Avoid running multiple agents concurrently against the same project until this lands
(tracked in Known Limitations / Contributing).

---

## Known Limitations (v0.1)

- **No file locking:** Concurrent `heald sync` or `heald finalize` calls can race on shared files (`AGENTS.md`, `log.md`). Documented limitation — avoid running multiple agents simultaneously writing memory.
- **Silent skip on malformed files:** `heald context` skips broken OKF files rather than crashing the agent session. Use `heald doctor` to find them.
- **Codex:** Hook injection is not yet implemented for Codex-specific native format. The `.agents/AGENTS.md` fallback works.

---

## Contributing

Contributions are welcome. Please open an issue before starting significant work so we can align on direction.

**To contribute:**

1. Fork the repo
2. Create a branch: `git checkout -b feat/your-feature`
3. Make your changes with tests where applicable
4. Open a pull request with a clear description of what and why

**Areas that would benefit from contributions:**
- Codex native format compiler
- File locking for concurrent safety (`fs2` crate)
- Shell completions (bash, zsh, fish, PowerShell)
- Windows path normalization edge cases

---

## License

MIT License

Copyright (c) 2026 Parth Sharma

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

## Authors

| Name | Role |
|---|---|
| **Parth Sharma** | Creator & maintainer |

---

<p align="center">
  <em>Heald (n.) — a wire on a loom that threads through and guides each warp strand so every one stays aligned to the same pattern, controlled by the loom's harness. Same job here.</em>
</p>

