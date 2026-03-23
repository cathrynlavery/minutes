# TODOS.md — Minutes

## P1: Agent Memory SDK (TypeScript) — IN PROGRESS
**What:** Extract the pure-TS file reading from the npx MCP server into a standalone `minutes-sdk` npm package. Any agent framework (LangChain, CrewAI, Autogen) can `import { searchMeetings } from 'minutes-sdk'` to query human conversation memory.
**Why:** Positions Minutes as infrastructure, not just a tool. The bridge between Mem0 (agent memory, 41k stars) and human conversation capture. Every agent developer becomes a potential user.
**Context:** Added to active scope during CEO review (2026-03-22). Ships after npx minutes-mcp.
**Effort:** M (human: ~1 week / CC: ~30 min)
**Depends on:** npx minutes-mcp shipping first (pure-TS file reading is the foundation).

## P2: Claude Code Plugin Standalone Distribution — IN PROGRESS
**What:** Publish the `.claude/plugins/minutes/` plugin so any Claude Code user can install it without cloning the Minutes repo. `/minutes record` just works in any CC session.
**Why:** Every Claude Code user becomes a potential Minutes user. Massive distribution channel, zero-friction adoption.
**Context:** Added to active scope during CEO review (2026-03-22). Research CC plugin distribution model first.
**Effort:** M (human: ~1 week / CC: ~30 min)
**Depends on:** Understanding CC plugin distribution model (research first).

## P3: Open Source Interactive Skill Template
**What:** Extract the multi-phase interactive skill pattern into a reusable SKILL-TEMPLATE-INTERACTIVE.md that other Claude Code plugin authors can follow.
**Why:** Positions Minutes as the reference implementation for great Claude Code plugin skills. Community multiplier.
**Pros:** Low effort, high community impact. Documents patterns that would otherwise live only in our heads.
**Cons:** Template may need revision as patterns evolve. Premature extraction risk if patterns aren't battle-tested.
**Context:** Deferred from interactive skills ecosystem CEO review (2026-03-19). Extract after the interactive skills have been used for 2-4 weeks and the patterns are proven.
**Effort:** S (human: ~1 day / CC: ~15 min)
**Depends on:** Interactive skills being battle-tested (2-4 weeks of usage).

## P2: Weekly Synthesis as First-Class Recall Panel View
**What:** Add a "Weekly" phase to the Recall panel that renders the weekly synthesis directly, rather than only running as a CLI skill in the terminal.
**Why:** Completes the lifecycle loop (prep → record → debrief → weekly) in the UI. Currently `/minutes weekly` runs in the terminal but the output isn't visually distinct from a regular conversation.
**Pros:** Full lifecycle coverage in one surface. Panel header shows "Weekly — Mar 10-14" with the synthesis.
**Cons:** Needs a way to distinguish "weekly view" from regular terminal output — may need richer rendering beyond raw xterm.
**Context:** Deferred from Recall panel CEO review (2026-03-19). Ship the base panel first, then evaluate whether weekly deserves a distinct rendering mode.
**Effort:** M (human: ~1 week / CC: ~30 min)
**Depends on:** Recall panel shipping.

## P3: Multi-Thread Conversations (Per-Meeting Chat History)
**What:** Instead of one singleton PTY session, each meeting gets its own conversation thread. Switching to a different meeting in the Recall panel loads that meeting's conversation history.
**Why:** Currently, context-switching via CURRENT_MEETING.md gives Claude the context, but the terminal scroll buffer from the previous meeting is still visible. Per-meeting threads would give clean separation.
**Pros:** You can return to a meeting discussion days later with full context.
**Cons:** Major architectural change — either multiple PTY sessions or a conversation persistence layer. Breaks the singleton model. May need hybrid approach beyond raw xterm.js.
**Context:** Deferred from Recall panel CEO review (2026-03-19). Evaluate after panel usage shows whether users actually want to revisit past meeting conversations.
**Effort:** L (human: ~2 weeks / CC: ~2 hours)
**Depends on:** Recall panel + usage data on whether users need per-meeting threads.

## P3: Publish to crates.io
**What:** Publish `minutes` to crates.io so users can install via `cargo install minutes`.
**Why:** The MCPB extension bundles only the Node.js MCP server, not the Rust binary. Users on any platform need to install the binary separately. `cargo install minutes` is the simplest universal method.
**Pros:** Standard Rust distribution. Shows up in crates.io search. Enables version pinning.
**Cons:** Need to resolve crate name availability. Requires maintaining a publish workflow.
**Context:** Deferred from windows-support eng review (2026-03-21). Currently users must use `cargo install --git` or download release binaries.
**Effort:** S (human: ~2 hours / CC: ~15 min)
**Depends on:** Windows support landing (cross-platform CI must pass first).

## P3: WASM compilation of minutes-reader for SDK
**What:** Compile `minutes-reader` (Rust crate, no audio deps) to WASM and use it as the npm SDK's parsing core instead of the TypeScript reimplementation.
**Why:** Eliminates TS/Rust parsing divergence, guarantees exact parity with the Rust pipeline, leverages battle-tested Rust code with existing tests.
**Context:** Eureka moment from eng review (2026-03-22). The `minutes-reader` crate was designed with no audio dependencies — it's a clean WASM target. Only matters when the TS SDK has real users and parsing edge cases surface.
**Effort:** M (human: ~1 week / CC: ~30 min)
**Depends on:** SDK shipping + user feedback on parsing edge cases.

## P3: Create DESIGN.md
**What:** Formalize the implicit design system (CSS variables, component patterns, typography, spacing, color usage) into a DESIGN.md file.
**Why:** The codebase has a strong implicit design language in the CSS but no documentation. As the UI grows (Recall panel, future features), having a reference prevents drift.
**Pros:** Single source of truth for all design decisions. Makes design reviews faster. Prevents contributors from introducing conflicting visual patterns.
**Cons:** Maintenance overhead — must be updated when CSS changes.
**Context:** Deferred from Recall panel design review (2026-03-19). Extract from existing CSS variables + the new Recall panel patterns. Include: color tokens, typography scale, spacing scale, radius values, component patterns (pills, badges, buttons, overlays, bars), animation timing curves.
**Effort:** S (human: ~2 hours / CC: ~15 min)
**Depends on:** Recall panel implementation (new patterns should be included).
