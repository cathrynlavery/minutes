#!/usr/bin/env node

/**
 * SessionStart hook: proactive meeting reminder.
 *
 * When a Claude Code session starts, check if the user has a meeting
 * in the next 60 minutes. If so, nudge them to run /minutes prep.
 *
 * Guards against being annoying:
 * - Only fires on startup (not resume/compact/clear)
 * - Only fires if the user has used /minutes prep before (~/.minutes/preps/ exists)
 * - Only fires during business hours (8am-6pm, weekdays)
 * - Can be disabled via ~/.config/minutes/config.toml: [reminders] enabled = false
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

// Guard 1: Only nudge if the user has actually used /minutes prep before
const prepsDir = join(homedir(), ".minutes", "preps");
if (!existsSync(prepsDir)) process.exit(0);

// Guard 2: Only fire during business hours (8am-6pm, weekdays)
const now = new Date();
const hour = now.getHours();
const day = now.getDay(); // 0=Sun, 6=Sat
if (day === 0 || day === 6 || hour < 8 || hour >= 18) process.exit(0);

// Guard 3: Check config for opt-out
const configPath = join(homedir(), ".config", "minutes", "config.toml");
if (existsSync(configPath)) {
  try {
    const config = readFileSync(configPath, "utf-8");
    // Simple TOML check — look for reminders.enabled = false
    if (config.includes("enabled = false") && config.includes("[reminders]")) {
      process.exit(0);
    }
  } catch {
    // Config unreadable — continue
  }
}

// Scan for recent voice memos (last 3 days, max 5)
let memoContext = "";
try {
  const memosDir = join(homedir(), "meetings", "memos");
  if (existsSync(memosDir)) {
    const { readdirSync, statSync } = await import("fs");
    const cutoff = Date.now() - 3 * 24 * 60 * 60 * 1000; // 3 days
    const files = readdirSync(memosDir)
      .filter((f) => f.endsWith(".md"))
      .map((f) => {
        const full = join(memosDir, f);
        const mtime = statSync(full).mtimeMs;
        return { name: f, path: full, mtime };
      })
      .filter((f) => f.mtime >= cutoff)
      .sort((a, b) => b.mtime - a.mtime)
      .slice(0, 5);

    if (files.length > 0) {
      const memoLines = files.map((f) => {
        // Extract title from frontmatter (first line after ---)
        try {
          const content = readFileSync(f.path, "utf-8");
          const titleMatch = content.match(/^title:\s*(.+)$/m);
          const dateMatch = content.match(/^date:\s*(.+)$/m);
          const title = titleMatch ? titleMatch[1].trim() : f.name.replace(".md", "");
          const date = dateMatch
            ? new Date(dateMatch[1].trim()).toLocaleDateString("en-US", { month: "short", day: "numeric" })
            : "recent";
          return `[${date}] ${title}`;
        } catch {
          return f.name.replace(".md", "");
        }
      });
      memoContext = `\n\nRecent voice memos: ${memoLines.join(", ")}. The user may ask about these — use search_meetings or get_meeting MCP tools to retrieve details.`;
    }
  }
} catch {
  // Non-fatal — skip voice memo scan
}

// Scan relationship graph for proactive intelligence (from SQLite index)
let relationshipContext = "";
try {
  const { execFileSync } = await import("child_process");
  const minutesBin = join(homedir(), ".local", "bin", "minutes");
  if (existsSync(minutesBin)) {
    // Get people data (auto-rebuilds if needed)
    const peopleRaw = execFileSync(minutesBin, ["people", "--json", "--limit", "10"], {
      encoding: "utf-8",
      timeout: 3000,
    });
    const people = JSON.parse(peopleRaw);

    if (Array.isArray(people) && people.length > 0) {
      // Losing touch alerts
      const losingTouch = people.filter((p) => p.losing_touch);
      if (losingTouch.length > 0) {
        const alerts = losingTouch
          .slice(0, 3)
          .map((p) => `${p.name} (${p.meeting_count} meetings, last ${Math.round(p.days_since)}d ago)`)
          .join(", ");
        relationshipContext += `\n\nLosing touch: ${alerts}. Consider reaching out.`;
      }

      // Stale commitments
      try {
        const commitsRaw = execFileSync(minutesBin, ["commitments", "--json"], {
          encoding: "utf-8",
          timeout: 3000,
        });
        const commitments = JSON.parse(commitsRaw);
        const stale = Array.isArray(commitments) ? commitments.filter((c) => c.status === "stale") : [];
        if (stale.length > 0) {
          const staleList = stale
            .slice(0, 3)
            .map((c) => `"${c.text}" for ${c.person_name || "unknown"}`)
            .join("; ");
          relationshipContext += `\n\nStale commitments (overdue): ${staleList}. Mention if relevant to today's work.`;
        }
      } catch {
        // Non-fatal
      }
    }
  }
} catch {
  // Non-fatal — relationship graph not available or not yet built
}

// Output a reminder for Claude to check the calendar
const output = {
  additionalContext: `Meeting reminder check: The user is an active Minutes user (has used /minutes prep before). If a Google Calendar MCP tool is available (gcal_list_events), check if the user has a meeting in the next 60 minutes. If they do, briefly mention it: "You have [meeting] in [N] minutes. Run /minutes prep to go in prepared." Keep it to one line — don't be pushy. If no calendar tool is available, skip silently. Do NOT mention this check if there are no upcoming meetings.${memoContext}${relationshipContext}`,
};

console.log(JSON.stringify(output));
