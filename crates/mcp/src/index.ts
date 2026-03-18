#!/usr/bin/env node

/**
 * Minutes MCP Server
 *
 * MCP tools for Claude Desktop / Cowork / Dispatch:
 *   - start_recording: Start recording audio from the default input device
 *   - stop_recording: Stop recording and process through the pipeline
 *   - get_status: Check if a recording is in progress
 *   - list_meetings: List recent meetings and voice memos
 *   - search_meetings: Search meeting transcripts
 *   - get_meeting: Get full transcript of a specific meeting
 *   - process_audio: Process an audio file through the pipeline
 *
 * All tools use execFile (not exec) to shell out to the `minutes` CLI binary.
 * No shell interpolation — safe from injection.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { execFile, spawn } from "child_process";
import { promisify } from "util";
import { existsSync } from "fs";
import { readFile } from "fs/promises";
import { join } from "path";
import { homedir } from "os";

const execFileAsync = promisify(execFile);

// ── Find the minutes binary ─────────────────────────────────

function findMinutesBinary(): string {
  const candidates = [
    join(__dirname, "..", "..", "..", "target", "release", "minutes"),
    join(__dirname, "..", "..", "..", "target", "debug", "minutes"),
    join(homedir(), ".cargo", "bin", "minutes"),
    "/opt/homebrew/bin/minutes",
    "/usr/local/bin/minutes",
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  // Fall back to PATH lookup
  return "minutes";
}

const MINUTES_BIN = findMinutesBinary();

// ── Helper: run minutes CLI command (uses execFile, not exec) ──

async function runMinutes(
  args: string[],
  timeoutMs: number = 30000
): Promise<{ stdout: string; stderr: string }> {
  try {
    const { stdout, stderr } = await execFileAsync(MINUTES_BIN, args, {
      timeout: timeoutMs,
      env: { ...process.env, RUST_LOG: "info" },
    });
    return { stdout: stdout.trim(), stderr: stderr.trim() };
  } catch (error: any) {
    if (error.killed) {
      throw new Error(`Command timed out after ${timeoutMs}ms`);
    }
    const stderr = error.stderr?.trim() || "";
    const stdout = error.stdout?.trim() || "";
    throw new Error(stderr || stdout || error.message);
  }
}

function parseJsonOutput(stdout: string): any {
  try {
    return JSON.parse(stdout);
  } catch {
    return { raw: stdout };
  }
}

// ── MCP Server ──────────────────────────────────────────────

const server = new McpServer({
  name: "minutes",
  version: "0.1.0",
});

// ── Tool: start_recording ───────────────────────────────────

server.tool(
  "start_recording",
  "Start recording audio from the default input device. The recording runs until stop_recording is called.",
  {
    title: z.string().optional().describe("Optional title for this recording"),
  },
  async ({ title }) => {
    const { stdout: statusOut } = await runMinutes(["status"]);
    const status = parseJsonOutput(statusOut);
    if (status.recording) {
      return {
        content: [
          {
            type: "text" as const,
            text: `Already recording (PID: ${status.pid}). Run stop_recording first.`,
          },
        ],
      };
    }

    // Spawn detached — recording is a foreground process that blocks,
    // so we spawn it and let it run independently
    const args = ["record"];
    if (title) args.push("--title", title);

    const child = spawn(MINUTES_BIN, args, {
      detached: true,
      stdio: "ignore",
      env: { ...process.env, RUST_LOG: "info" },
    });
    child.unref();

    // Wait for PID file to appear
    await new Promise((r) => setTimeout(r, 1000));

    const { stdout: newStatus } = await runMinutes(["status"]);
    const result = parseJsonOutput(newStatus);

    return {
      content: [
        {
          type: "text" as const,
          text: result.recording
            ? `Recording started (PID: ${result.pid}). Say "stop recording" when done.`
            : "Recording failed to start. Check `minutes logs` for details.",
        },
      ],
    };
  }
);

// ── Tool: stop_recording ────────────────────────────────────

server.tool(
  "stop_recording",
  "Stop the current recording and process it (transcribe, diarize, summarize).",
  {},
  async () => {
    try {
      const { stdout, stderr } = await runMinutes(["stop"], 180000);
      const result = parseJsonOutput(stdout);

      const message = result.file
        ? `Recording saved: ${result.file}\nTitle: ${result.title}\nWords: ${result.words}`
        : stderr || "Recording stopped.";

      return { content: [{ type: "text" as const, text: message }] };
    } catch (error: any) {
      return {
        content: [{ type: "text" as const, text: `Stop failed: ${error.message}` }],
      };
    }
  }
);

// ── Tool: get_status ────────────────────────────────────────

server.tool(
  "get_status",
  "Check if a recording is currently in progress.",
  {},
  async () => {
    const { stdout } = await runMinutes(["status"]);
    const status = parseJsonOutput(stdout);
    const text = status.recording
      ? `Recording in progress (PID: ${status.pid})`
      : "No recording in progress.";
    return { content: [{ type: "text" as const, text }] };
  }
);

// ── Tool: list_meetings ─────────────────────────────────────

server.tool(
  "list_meetings",
  "List recent meetings and voice memos.",
  {
    limit: z.number().optional().default(10).describe("Maximum results"),
    type: z.enum(["meeting", "memo"]).optional().describe("Filter by type"),
  },
  async ({ limit, type: contentType }) => {
    const args = ["list", "--limit", String(limit)];
    if (contentType) args.push("-t", contentType);

    const { stdout } = await runMinutes(args);
    const meetings = parseJsonOutput(stdout);

    if (Array.isArray(meetings) && meetings.length === 0) {
      return { content: [{ type: "text" as const, text: "No meetings or memos found." }] };
    }

    const text = Array.isArray(meetings)
      ? meetings
          .map((m: any) => `${m.date} — ${m.title} [${m.content_type}]\n  ${m.path}`)
          .join("\n\n")
      : stdout;

    return { content: [{ type: "text" as const, text }] };
  }
);

// ── Tool: search_meetings ───────────────────────────────────

server.tool(
  "search_meetings",
  "Search meeting transcripts and voice memos.",
  {
    query: z.string().describe("Text to search for"),
    type: z.enum(["meeting", "memo"]).optional().describe("Filter by type"),
    since: z.string().optional().describe("Only results after this date (ISO)"),
    limit: z.number().optional().default(10).describe("Maximum results"),
  },
  async ({ query, type: contentType, since, limit }) => {
    const args = ["search", query, "--limit", String(limit)];
    if (contentType) args.push("-t", contentType);
    if (since) args.push("--since", since);

    const { stdout } = await runMinutes(args);
    const results = parseJsonOutput(stdout);

    if (Array.isArray(results) && results.length === 0) {
      return {
        content: [{ type: "text" as const, text: `No results found for "${query}".` }],
      };
    }

    const text = Array.isArray(results)
      ? results
          .map(
            (r: any) =>
              `${r.date} — ${r.title} [${r.content_type}]\n  ${r.snippet}\n  ${r.path}`
          )
          .join("\n\n")
      : stdout;

    return { content: [{ type: "text" as const, text }] };
  }
);

// ── Tool: get_meeting ───────────────────────────────────────

server.tool(
  "get_meeting",
  "Get the full transcript and details of a specific meeting or memo.",
  {
    path: z.string().describe("Path to the meeting markdown file"),
  },
  async ({ path }) => {
    try {
      const content = await readFile(path, "utf-8");
      return { content: [{ type: "text" as const, text: content }] };
    } catch (error: any) {
      return {
        content: [{ type: "text" as const, text: `Could not read: ${error.message}` }],
      };
    }
  }
);

// ── Tool: process_audio ─────────────────────────────────────

server.tool(
  "process_audio",
  "Process an audio file through the transcription pipeline.",
  {
    file_path: z.string().describe("Path to audio file (.wav, .m4a, .mp3)"),
    type: z.enum(["meeting", "memo"]).optional().default("memo").describe("Content type"),
    title: z.string().optional().describe("Optional title"),
  },
  async ({ file_path, type: contentType, title }) => {
    const args = ["process", file_path, "-t", contentType];
    if (title) args.push("--title", title);

    try {
      const { stdout } = await runMinutes(args, 300000);
      const result = parseJsonOutput(stdout);

      return {
        content: [
          {
            type: "text" as const,
            text: result.file
              ? `Processed: ${result.file}\nTitle: ${result.title}\nWords: ${result.words}`
              : stdout,
          },
        ],
      };
    } catch (error: any) {
      return {
        content: [{ type: "text" as const, text: `Failed: ${error.message}` }],
      };
    }
  }
);

// ── Tool: add_note ───────────────────────────────────────────

server.tool(
  "add_note",
  "Add a note to the current recording. Notes are timestamped and included in the meeting summary. If no recording is active, annotate an existing meeting file with --meeting.",
  {
    text: z.string().describe("The note text (plain text, no markdown needed)"),
    meeting_path: z
      .string()
      .optional()
      .describe("Path to an existing meeting file to annotate (for post-meeting notes)"),
  },
  async ({ text, meeting_path }) => {
    try {
      const args = ["note", text];
      if (meeting_path) args.push("--meeting", meeting_path);

      const { stdout, stderr } = await runMinutes(args);
      return {
        content: [{ type: "text" as const, text: stderr || stdout || "Note added." }],
      };
    } catch (error: any) {
      return {
        content: [{ type: "text" as const, text: `Note failed: ${error.message}` }],
      };
    }
  }
);

// ── Start server ────────────────────────────────────────────

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("Minutes MCP server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
