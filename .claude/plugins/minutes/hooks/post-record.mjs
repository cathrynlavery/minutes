#!/usr/bin/env node

/**
 * PostToolUse hook: auto-tag meetings + post-recording intelligence alerts.
 *
 * When `minutes stop` or `minutes process` completes, this hook:
 * 1. Tags the meeting with the current git repo name (project context)
 * 2. Checks for decision conflicts with prior meetings
 * 3. Checks for overdue action items involving attendees
 * 4. Outputs alert context if conflicts or overdue items found
 *
 * Hook event: PostToolUse
 * Tool: Bash
 * Matcher: minutes stop|minutes process
 *
 * Errors are logged to ~/.minutes/logs/hook-errors.log, never silently swallowed.
 */

import { execFileSync } from "child_process";
import {
  readFileSync,
  writeFileSync,
  appendFileSync,
  existsSync,
  mkdirSync,
} from "fs";
import { join } from "path";
import { homedir } from "os";

const LOG_DIR = join(homedir(), ".minutes", "logs");
const LOG_FILE = join(LOG_DIR, "hook-errors.log");

function logError(context, error) {
  try {
    mkdirSync(LOG_DIR, { recursive: true });
    const timestamp = new Date().toISOString();
    const entry = `${timestamp} [post-record] ${context}: ${error.message || error}\n`;
    appendFileSync(LOG_FILE, entry);
  } catch {
    // Last resort — can't even log. Give up silently.
  }
}

// Check if this was a minutes command
const input = JSON.parse(process.argv[2] || "{}");
const toolName = input.tool_name || "";
const toolInput = input.tool_input || {};

if (toolName !== "Bash") process.exit(0);

const command = toolInput.command || "";
if (
  !command.includes("minutes stop") &&
  !command.includes("minutes process")
) {
  process.exit(0);
}

// Get the current git repo name
let projectName = null;
try {
  projectName = execFileSync("git", ["rev-parse", "--show-toplevel"], {
    encoding: "utf-8",
    timeout: 5000,
  })
    .trim()
    .split("/")
    .pop();
} catch {
  // Not in a git repo — skip project tagging but continue with alerts
}

// Find the most recently processed meeting file
const lastResult = join(homedir(), ".minutes", "last-result.json");
if (!existsSync(lastResult)) process.exit(0);

let meetingPath = null;
let meetingContent = null;

try {
  const result = JSON.parse(readFileSync(lastResult, "utf-8"));
  meetingPath = result.file;

  if (!meetingPath || !existsSync(meetingPath)) process.exit(0);

  meetingContent = readFileSync(meetingPath, "utf-8");
} catch (err) {
  logError("read-last-result", err);
  process.exit(0);
}

// --- Phase 1: Project tagging (existing behavior) ---
if (projectName) {
  try {
    if (
      !meetingContent.includes(`project: ${projectName}`) &&
      meetingContent.startsWith("---")
    ) {
      const endIdx = meetingContent.indexOf("\n---", 3);
      if (endIdx > 0) {
        const before = meetingContent.slice(0, endIdx);
        const after = meetingContent.slice(endIdx);
        meetingContent = `${before}\nproject: ${projectName}${after}`;
        writeFileSync(meetingPath, meetingContent, { mode: 0o600 });
      }
    }
  } catch (err) {
    logError("project-tagging", err);
  }
}

// --- Phase 2: Post-recording intelligence alerts ---
// These run with a 5-second timeout to avoid blocking the user.

const alerts = [];

// 2a. Check for decision conflicts
try {
  // Extract decisions from the meeting's frontmatter.
  // Look for topic: fields anywhere in the decisions block.
  // Handle both top-level and indented `decisions:` in YAML.
  const topicsInMeeting = [];
  const frontmatterEnd = meetingContent.indexOf("\n---", 3);
  if (frontmatterEnd > 0) {
    const frontmatter = meetingContent.slice(0, frontmatterEnd);
    // Find all topic: values in the frontmatter (regardless of nesting)
    const topicMatches = frontmatter.matchAll(/topic:\s*(.+)/g);
    for (const m of topicMatches) {
      topicsInMeeting.push(m[1].trim());
    }
  }

  // For each topic, search for prior decisions
  for (const topic of topicsInMeeting.slice(0, 3)) {
    try {
      const searchResult = execFileSync(
        "minutes",
        ["search", topic, "--limit", "5"],
        {
          encoding: "utf-8",
          timeout: 5000,
        }
      );

      const results = JSON.parse(searchResult);
      const priorMeetings = results.filter((r) => r.path !== meetingPath);

      if (priorMeetings.length > 0) {
        alerts.push(
          `Decision on "${topic}" — ${priorMeetings.length} prior meeting(s) also discussed this. Run /minutes debrief to check for conflicts.`
        );
      }
    } catch (searchErr) {
      logError(`decision-search-${topic}`, searchErr);
    }
  }
} catch (err) {
  logError("decision-conflict-check", err);
}

// 2b. Check for overdue action items
try {
  const actionResult = execFileSync("minutes", ["actions"], {
    encoding: "utf-8",
    timeout: 5000,
  });

  const actions = JSON.parse(actionResult);
  const today = new Date().toISOString().slice(0, 10);
  const overdue = actions.filter((a) => a.due && a.due < today && a.status === "open");

  if (overdue.length > 0) {
    const oldest = overdue.sort((a, b) => a.due.localeCompare(b.due))[0];
    alerts.push(
      `${overdue.length} overdue action item(s). Oldest: "${oldest.task}" (due ${oldest.due}).`
    );
  }
} catch (err) {
  // minutes actions might not be available or might fail — that's OK
  if (err.message && !err.message.includes("ETIMEDOUT")) {
    logError("overdue-action-check", err);
  }
}

// 2c. Check for speaker attributions that need confirmation
try {
  if (meetingContent.includes("speaker_map:")) {
    const hasMedium = meetingContent.includes("confidence: medium");
    const hasHigh = meetingContent.includes("confidence: high");

    if (hasMedium && !hasHigh) {
      alerts.push(
        `Speaker attributions are auto-detected (medium confidence). Run \`minutes confirm --meeting ${meetingPath}\` to confirm who is who.`
      );
    }
  } else if (meetingContent.includes("SPEAKER_")) {
    alerts.push(
      `Meeting has anonymous speaker labels. Run \`minutes confirm --meeting ${meetingPath}\` to tag speakers with real names.`
    );
  }
} catch (err) {
  logError("speaker-attribution-check", err);
}

// --- Phase 3: Output alerts as additional context ---
if (alerts.length > 0) {
  const output = {
    additionalContext: `Post-meeting alert:\n${alerts.map((a) => `- ${a}`).join("\n")}\n\nRun /minutes debrief for the full post-meeting analysis.`,
  };
  console.log(JSON.stringify(output));
}
