# minutes

**Your AI remembers every conversation you've had.**

Agents have run logs. Humans have conversations. **minutes** captures the human side — the decisions, the intent, the context that agents need but can't observe — and makes it queryable.

Record a meeting. Capture a voice memo on a walk. Ask Claude what was decided three weeks ago. It just works.

```
$ minutes record --context "Q2 pricing discussion with Alex"
Recording... (Ctrl-C or `minutes stop` to finish)
  Tip: add notes with `minutes note "your note"` in another terminal

^C
Stopping recording...
Transcribing.....
Saved: ~/meetings/2026-03-17-q2-pricing-discussion-with-sarah.md
```

```
$ minutes search "pricing"
2026-03-17 — Q2 Pricing Discussion with Alex [meeting]
  [4:20] I think monthly billing makes more sense for independent advisors...
```

```
$ minutes actions --assignee mat
Open action items (2):
  @user: Send pricing doc (by Friday)
    from: 2026-03-17 — Q2 Pricing Discussion with Alex
  @user: Set up monthly billing tier experiment (by next week)
    from: 2026-03-17 — Q2 Pricing Discussion with Alex
```

## How it works

```
Audio → Transcribe → Diarize → Summarize → Structured Markdown
         (local)     (speakers)   (LLM)       (decisions,
        whisper.cpp   pyannote   Claude/       action items,
                                 Ollama/       searchable)
                                 OpenAI
```

Everything runs locally. Your audio never leaves your machine (unless you opt into cloud LLM summarization).

## Install

```bash
# Homebrew (macOS)
brew tap silverstein/tap
brew install minutes

# From source (requires Rust + cmake)
# macOS 15+ / Xcode 26+: set C++ include path for whisper.cpp
export CXXFLAGS="-I$(xcrun --show-sdk-path)/usr/include/c++/v1"
cargo install --path crates/cli

# Download whisper model
minutes setup --model tiny    # Quick start (75MB, fast, less accurate)
minutes setup --model small   # Recommended (466MB, good accuracy)
minutes setup --model base    # Middle ground (141MB)
```

### Desktop app (optional)

```bash
export CXXFLAGS="-I$(xcrun --show-sdk-path)/usr/include/c++/v1"
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo tauri build --bundles app
# Opens: target/release/bundle/macos/Minutes.app
```

The desktop app adds a system tray icon, recording controls, audio visualizer, and a meeting list window. macOS will prompt for microphone permission on first recording.

### Signed macOS releases

Maintainers can build signed and notarized desktop releases with the GitHub
Actions workflow in
[.github/workflows/release-macos.yml](/Users/silverbook/Sites/minutes/.github/workflows/release-macos.yml).

The full secrets contract and maintainer steps live in
[docs/RELEASE-MACOS.md](/Users/silverbook/Sites/minutes/docs/RELEASE-MACOS.md).

Stable vs preview channels and rollback expectations live in
[docs/RELEASE-CHANNELS.md](/Users/silverbook/Sites/minutes/docs/RELEASE-CHANNELS.md).
Reproducible release-note generation lives in
[docs/RELEASE-NOTES.md](/Users/silverbook/Sites/minutes/docs/RELEASE-NOTES.md).
The current auto-update evaluation and recommendation live in
[docs/AUTO-UPDATE-EVALUATION.md](/Users/silverbook/Sites/minutes/docs/AUTO-UPDATE-EVALUATION.md).

### Troubleshooting

**No speech detected / blank audio:**
The most common cause is microphone permissions. Check System Settings → Privacy & Security → Microphone and ensure your terminal app (or Minutes.app) has access.

**tmux users:** tmux server runs as a separate process that doesn't inherit your terminal's mic permission. Either run `minutes record` from a direct terminal window (not inside tmux), or use the Minutes.app desktop bundle which gets its own mic permission.

**Build fails with C++ errors on macOS 26+:**
whisper.cpp needs the SDK include path. Set `CXXFLAGS` as shown above before building.

## Features

### Record meetings
```bash
minutes record                                    # Record from mic
minutes record --title "Standup" --context "Sprint 4 blockers"  # With context
minutes stop                                      # Stop from another terminal
```

### Take notes during meetings
```bash
minutes note "Alex wants monthly billing not annual billing"          # Timestamped, feeds into summary
minutes note "Logan agreed"                       # LLM weights your notes heavily
```

### Process voice memos
```bash
minutes process ~/Downloads/voice-memo.m4a        # Any audio format
minutes watch                                     # Auto-process new files in inbox
```

### Search everything
```bash
minutes search "pricing"                          # Full-text search
minutes search "onboarding" -t memo               # Filter by type
minutes actions                                   # Open action items across all meetings
minutes actions --assignee sarah                   # Filter by person
minutes list                                      # Recent recordings
```

### Post-meeting annotations
```bash
minutes note --meeting ~/meetings/2026-03-17-pricing.md "Alex confirmed via email"
```

## Output format

Meetings save as markdown with structured YAML frontmatter:

```yaml
---
title: Q2 Pricing Discussion with Alex
type: meeting
date: 2026-03-17T14:00:00
duration: 42m
context: "Discuss Q2 pricing, follow up on annual billing decision"
action_items:
  - assignee: mat
    task: Send pricing doc
    due: Friday
    status: open
  - assignee: sarah
    task: Review competitor grid
    due: March 21
    status: open
decisions:
  - text: Run pricing experiment at monthly billing with 10 advisors
    topic: pricing experiment
---

## Summary
- Alex proposed lowering API launch timeline from annual billing to monthly billing/mo
- Compromise: run experiment with 10 advisors at monthly billing

## Notes
- [4:23] Alex wants monthly billing not annual billing
- [12:10] Logan agreed

## Decisions
- [x] Run pricing experiment at monthly billing with 10 advisors

## Action Items
- [ ] @user: Send pricing doc by Friday
- [ ] @sarah: Review competitor grid by March 21

## Transcript
[SPEAKER_0 0:00] So let's talk about the pricing...
[SPEAKER_1 4:20] I think monthly billing makes more sense...
```

Works with [Obsidian](https://obsidian.md), grep, or any markdown tool. Action items and decisions are queryable via the CLI and MCP tools.

## Claude integration

minutes is a native extension for the Claude ecosystem. **No API keys needed** — Claude summarizes your meetings when you ask, using your existing Claude subscription.

The pipeline captures and transcribes locally. Claude does the intelligence:

```
You: "Summarize my last meeting"
Claude: [calls get_meeting] → reads transcript → summarizes in conversation

You: "What did Alex say about pricing?"
Claude: [calls search_meetings] → finds matches → synthesizes answer

You: "Any open action items for me?"
Claude: [calls list_meetings] → scans frontmatter → reports open items
```

No `ANTHROPIC_API_KEY`. No extra cost. Just your Claude subscription doing what it already does — but with your meeting history as context.

### Claude Desktop (MCP)
```json
{
  "mcpServers": {
    "minutes": {
      "command": "node",
      "args": ["path/to/minutes/crates/mcp/dist/index.js"]
    }
  }
}
```

8 MCP tools: `start_recording`, `stop_recording`, `get_status`, `list_meetings`, `search_meetings`, `get_meeting`, `process_audio`, `add_note`

### Claude Code (Plugin)
```
.claude/plugins/minutes/
├── 5 skills: /minutes record, search, list, recap, note
├── 1 agent: meeting-analyst (cross-meeting intelligence)
└── 1 hook: auto-tags meetings with current git repo
```

### Cowork / Dispatch
MCP tools are automatically available in Cowork. From your phone via Dispatch: *"Start recording"* → Mac captures → Claude processes → summary on your phone.

### Optional: automated summarization

If you want summaries generated automatically in the pipeline (without asking Claude each time), you can configure a local LLM:

```toml
[summarization]
engine = "ollama"         # Free, local, no API key
ollama_model = "llama3.2"
```

Or use API keys for cloud LLMs — but most users won't need this since Claude handles it conversationally.

## Voice memos (iPhone → Mac)

No iOS app needed. Use Apple's built-in Voice Memos + a Shortcut:

1. Run `minutes watch` on your Mac (or install the launchd service for auto-start)
2. Record in Voice Memos on iPhone
3. Share → "Save to Minutes" (iCloud Drive sync)
4. Markdown appears in `~/meetings/memos/`

Supports `.m4a`, `.mp3`, `.wav`, `.ogg`, `.webm`. Format conversion is automatic via [symphonia](https://github.com/pdeljanov/Symphonia).

## Configuration

Optional — minutes works out of the box.

```toml
# ~/.config/minutes/config.toml

[transcription]
model = "small"           # tiny (75MB), base, small (466MB), medium, large-v3 (3.1GB)

[summarization]
engine = "none"           # Default: Claude summarizes conversationally via MCP
                          # Set to "ollama" for automatic local summaries
ollama_url = "http://localhost:11434"
ollama_model = "llama3.2"

[diarization]
engine = "pyannote"       # pyannote (best quality) or none

[search]
engine = "builtin"        # builtin (regex) or qmd (semantic)

[watch]
paths = ["~/.minutes/inbox"]
settle_delay_ms = 2000    # iCloud sync safety delay
```

## Architecture

```
minutes/
├── crates/core/    12 Rust modules — the engine (shared by all interfaces)
├── crates/cli/     CLI binary — 12 commands
├── crates/mcp/     MCP server — 8 tools for Claude Desktop
├── tauri/          Menu bar app — system tray, dark-mode UI
└── .claude/plugins/minutes/   Claude Code plugin — 5 skills + 1 agent
```

Single `minutes-core` library shared by CLI, MCP server, and Tauri app. Zero code duplication.

**Built with:** Rust, [whisper.cpp](https://github.com/ggerganov/whisper.cpp), [symphonia](https://github.com/pdeljanov/Symphonia), [cpal](https://github.com/RustAudio/cpal), [Tauri v2](https://v2.tauri.app/), [ureq](https://github.com/algesten/ureq)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
