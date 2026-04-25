# Adding agent integrations

Minutes has several agent surfaces. Do not add a new agent by copying an existing
integration wholesale. Pick the smallest surface that matches what the host
actually supports.

## Surfaces

| Surface | Use when | Examples |
|---|---|---|
| Raw files | The agent can read `~/meetings/` directly. | Cursor, any local coding agent |
| MCP server | The host supports MCP tools/resources/prompts. | Claude Desktop, Codex, Gemini CLI |
| Portable skills | The host discovers Agent Skills-style `.agents/skills` folders. | Codex, Gemini CLI, Pi |
| Host-specific skills | The host needs a different generated shape. | Claude Code plugin, OpenCode commands |
| `agent_command` backend | Minutes should call the agent CLI for summaries. | `claude`, `codex`, `opencode`, `pi` |
| Routing eval | The agent has a non-interactive prompt mode worth benchmarking. | `npm --prefix tooling/skills run routing:agents -- --agent codex` |

## Checklist

1. Identify the host contract.
   - Can it read files?
   - Does it support MCP?
   - Does it auto-discover `.agents/skills`?
   - Does it require a host-specific skill tree?
   - Does it have a non-interactive CLI mode?

2. If the host can reuse `.agents/skills`, do not generate a duplicate tree.
   Duplicate skill names can create collisions and make the agent less reliable.

3. If the host needs a generated skill surface, update:
   - `tooling/skills/schema.ts`
   - `tooling/skills/hosts/`
   - `tooling/skills/compiler/render.ts`
   - `tooling/skills/compiler/compile.ts`
   - `tooling/skills/compiler/check.ts`
   - `tooling/skills/compiler/golden.ts`
   - generated outputs under the host-specific tree

4. If the host should be callable from Minutes summarization, update:
   - `crates/core/src/summarize.rs`
   - targeted `prepare_agent_invocation_*` tests
   - `tauri/src-tauri/src/commands.rs`
   - `tauri/src/index.html`
   - `docs/CONFIG.md`

5. If the host should participate in routing evals, update:
   - `tooling/skills/compiler/agent-routing.ts`
   - `tooling/skills/compiler/agent-routing.test.ts` if parsing or unavailable handling changes

6. Update public and agent-facing docs:
   - `README.md`
   - `site/app/for-agents/page.tsx`
   - `site/lib/product-surfaces.json`
   - `manifest.json`
   - `docs/CONFIG.md`
   - `docs/<agent>.md` when the host has provider-specific caveats
   - run `node scripts/generate_llms_txt.mjs`

7. Run the relevant gates:
   - `cargo fmt`
   - targeted Rust tests for the invocation path
   - `cargo check -p minutes-app`
   - `npm --prefix tooling/skills run build`
   - `npm --prefix tooling/skills run compile:dry`
   - `npm --prefix tooling/skills run check`
   - `npm --prefix tooling/skills run test`
   - `npm --prefix site run check:llms`
   - `npm --prefix site run build` when site pages changed

## Current agent classes

- Claude Code: host-specific plugin surface plus MCP.
- OpenCode: host-specific `.opencode/skills` and `.opencode/commands`, plus MCP when configured.
- Codex: portable `.agents/skills` plus MCP.
- Gemini CLI: portable `.agents/skills` plus MCP.
- Pi coding agent: portable `.agents/skills` plus opt-in `agent_command = "pi"` summarization. No separate `.pi/skills` tree.
- Cursor and other editors: raw meeting files and MCP where the host supports it.

When in doubt, prefer the raw file or MCP path first. Add a custom host surface
only when the agent cannot consume the existing portable one.
