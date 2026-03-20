# Napkin

## Corrections
| Date | Source | What Went Wrong | What To Do Instead |
|------|--------|----------------|-------------------|
| 2026-03-18 | self | Updated `start_recording` signature for processing-state wiring but missed the tray menu call site in `tauri/src-tauri/src/main.rs` | After widening Tauri command/helper signatures, run `rg` for all call sites before testing so the state plumbing stays consistent |
| 2026-03-19 | self | The Tauri live-recording path was still injecting timestamp titles, which quietly bypassed the smart-title pipeline we had already shipped | When adding UX polish around recording labels, verify we are not overriding downstream title generation or artifact heuristics by accident |
| 2026-03-19 | self | Tried a direct `cargo run -p minutes-cli` sanity check without the repo's usual macOS `CXXFLAGS`, which failed in `whisper-rs-sys` even though the targeted tests had already passed | On this machine, use the `CXXFLAGS=\"-I$(xcrun --show-sdk-path)/usr/include/c++/v1\"` prefix for any Rust command that may build `whisper-rs`, not just tests |
| 2026-03-19 | self | Used backticks inside a shell `rg` argument during verification, and `zsh` treated them as command substitution | When grepping for literal backtick-delimited strings in shell commands, wrap the whole pattern safely or avoid backticks in the query altogether |
| 2026-03-19 | self | Assumed parsed `Frontmatter` carried the runtime-style `content_type` field and wired a consistency heuristic to a field that does not exist | When adding report features on top of markdown frontmatter, re-open the actual `Frontmatter` struct and map from `r#type` explicitly instead of assuming it mirrors downstream write results |
| 2026-03-19 | self | Probed `qmd collection add` assuming the old plan syntax and accidentally created a real collection while trying to discover the interface | For external CLIs, inspect the shipped help or source before probing mutating subcommands; for QMD specifically, `collection add` takes `<path> --name <name>` and `collection list` does not include paths, so pair it with `collection show` |
| 2026-03-19 | self | Guessed the Tauri crate package name for `cargo check` instead of reading `tauri/src-tauri/Cargo.toml` first | When verifying a workspace member, read the manifest or use `--manifest-path` before assuming the package name |

## User Preferences
- For coding/debugging/testing/review tasks, prioritize technical implementation detail and concrete verification.
- For repo reviews, findings should be the primary output, ordered by severity with file/line references.

## Patterns That Work
- Start by checking repo instructions plus `bd` workflow, then inspect both the Rust crates and the MCP/Tauri surfaces before making claims about app behavior.
- On macOS 26+, Rust tests that compile `whisper-rs` need `CXXFLAGS="-I$(xcrun --show-sdk-path)/usr/include/c++/v1"`; core tests pass once that is set.

## Patterns That Don't Work
- Assuming this repo is only a CLI tool misses the Tauri desktop app and MCP integration surfaces that need review too.
- Trusting `path.resolve(...).startsWith(...)` in Node is not a safe allowlist check here; it misses sibling-prefix and symlink cases.

## Domain Notes
- `minutes` is a local-first meeting capture app with Rust core/CLI, a Tauri desktop app, and a TypeScript MCP server.
- The worktree may already contain user changes; review around them carefully and do not revert unrelated edits.
- The desktop app mixes in-memory recording state with PID-file-based status, so app restarts and cross-surface recording flows are easy places for desync bugs.
