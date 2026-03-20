#!/usr/bin/env node

/**
 * SessionStart hook: proactive meeting reminder.
 *
 * When a Claude Code session starts, check if the user has a meeting
 * in the next 60 minutes. If so, nudge them to run /minutes prep.
 *
 * This hook is lightweight — it exits quickly if no calendar source
 * is available. It uses the Google Calendar MCP tool availability as
 * a signal, but since hooks can't call MCP tools directly, it outputs
 * additionalContext that tells Claude to check the calendar.
 *
 * Hook event: SessionStart
 * Matcher: startup
 */

import { existsSync, readFileSync } from "fs";
import { join } from "path";
import { homedir } from "os";

// Only run on startup, not resume/compact/clear
const input = JSON.parse(process.argv[2] || "{}");
const event = input.session_event || input.event || "";

if (event !== "startup") process.exit(0);

// Check if the user has any prep infrastructure set up
// (i.e., they've used /minutes prep before)
const prepsDir = join(homedir(), ".minutes", "preps");
const meetingsDir = join(homedir(), "meetings");

// Only nudge if the user has meeting files (active Minutes user)
if (!existsSync(meetingsDir)) process.exit(0);

// Output a reminder for Claude to check the calendar
const output = {
  additionalContext: `Meeting reminder check: The user has Minutes installed with meeting history at ~/meetings/. If a Google Calendar MCP tool is available (gcal_list_events), check if the user has a meeting in the next 60 minutes. If they do, briefly mention it: "You have [meeting] in [N] minutes. Run /minutes prep to go in prepared." Keep it to one line — don't be pushy. If no calendar tool is available, skip silently.`,
};

console.log(JSON.stringify(output));
