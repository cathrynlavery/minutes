# Minutes 0.16.1: update notes, cleaner first demo, and Recall instructions

Minutes 0.16.1 is a patch release for the 0.16 line. It fixes the release-note delivery path that could make the desktop "What's New" screen appear without the 0.16.0 story, tightens the macOS release workflow, gives the built-in demo real speech audio, and makes Recall's terminal-agent instructions work for both `CLAUDE.md` and `AGENTS.md` agents.

The short version: if you missed the 0.16.0 notes in the app, this patch carries the important part forward without making the in-app modal a wall of text.

## What changed

- **The 0.16.0 story is visible again.** 0.16.0 added Agent Event Bus v0, faster SQLite-backed search, safer desktop recording during calls, prompt-only templates, and broad lowercase-`m` design/packaging polish. This patch makes sure desktop users actually see that story after updating.
- **Release notes are more reliable in the app.** The updater banner now renders updater-provided notes, caches them during install, and the post-update "What's New" modal falls back to `latest.json` if the GitHub release API returns an empty body or is unavailable.
- **macOS patch assets are safer to rebuild.** Manual `Release macOS` dispatches now check out and verify the requested tag before uploading assets to that tag's release.
- **`minutes demo` feels like a real demo.** The bundled demo audio now contains short speech instead of a beep/silence fixture, and failures now point users to `minutes health` instead of assuming setup is missing.
- **Recall gives terminal agents better context.** The assistant workspace now writes matching `CLAUDE.md` and `AGENTS.md` instructions, including a Minutes-native response contract that allows conversational, narrative, or report-style answers instead of forcing short bullets.

## Who should care

- Desktop users who updated to 0.16.0 but saw blank or thin release notes.
- Maintainers cutting or repairing macOS release assets.
- New users trying `minutes demo` before recording a real meeting.
- People using Recall through Claude Code, Codex, OpenCode, Gemini, Pi, or other terminal agents.

## CLI / MCP / desktop impact

- **CLI:** `minutes demo` now uses a real speech fixture and gives better diagnostic guidance on failure. No command syntax changed.
- **MCP / agent integrations:** no MCP tool contract changes. Recall's local assistant instructions are now mirrored for `CLAUDE.md` and `AGENTS.md` conventions.
- **Desktop:** update banners and "What's New" have a more reliable release-note fallback path. macOS release packaging is safer for patch asset rebuilds.

## Breaking changes or migration notes

No breaking changes are expected.

The 0.16.0 event bus, search index, templates, and desktop reliability work remain additive. If this is your first 0.16-line update, the first search-index sync may take a moment on a large archive.

## Known issues

Windows desktop artifacts remain unsigned/advanced-user builds.

MCP auto-install verifies `SHA256SUMS`, but full signature verification is still a follow-up.

Native call capture still depends on local audio routing, permissions, and meeting-app behavior. Keep real-world call-capture regressions open until reporter confirmation lands.
