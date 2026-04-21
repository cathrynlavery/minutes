import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { MEETING_INSIGHT_KINDS, parseKnowledgeConfig, shouldRunMainEntry } from "./index.js";

describe("meeting insight contract", () => {
  it("exports only the insight kinds the pipeline emits today", () => {
    expect(MEETING_INSIGHT_KINDS).toEqual(["decision", "commitment", "question"]);
  });
});

describe("parseKnowledgeConfig", () => {
  it("only treats enabled=true inside the knowledge section as enabling the knowledge base", () => {
    const parsed = parseKnowledgeConfig(`
[recording]
enabled = true

[knowledge]
enabled = false
path = "~/kb"
`);

    expect(parsed).toEqual({
      enabled: false,
      path: "~/kb",
      adapter: "wiki",
      engine: "none",
    });
  });

  it("reads knowledge settings from the knowledge section", () => {
    const parsed = parseKnowledgeConfig(`
[knowledge]
enabled = true
path = "~/kb"
adapter = "para"
engine = "agent"
`);

    expect(parsed).toEqual({
      enabled: true,
      path: "~/kb",
      adapter: "para",
      engine: "agent",
    });
  });
});

describe("shouldRunMainEntry", () => {
  it("accepts npm .bin shims that realpath to the module file", () => {
    const tempRoot = mkdtempSync(join(tmpdir(), "minutes-mcp-entry-"));
    const packageDir = join(tempRoot, "node_modules", "minutes-mcp", "dist");
    const binDir = join(tempRoot, "node_modules", ".bin");
    const modulePath = join(packageDir, "index.js");
    const shimPath = join(binDir, "minutes-mcp");

    mkdirSync(packageDir, { recursive: true });
    mkdirSync(binDir, { recursive: true });
    writeFileSync(modulePath, "export {};\n");
    symlinkSync(modulePath, shimPath);

    try {
      expect(shouldRunMainEntry(shimPath, modulePath)).toBe(true);
    } finally {
      rmSync(tempRoot, { recursive: true, force: true });
    }
  });

  it("accepts equivalent paths once symlinks are resolved", () => {
    expect(shouldRunMainEntry(import.meta.filename, import.meta.filename)).toBe(true);
  });

  it("rejects unrelated worker entrypoints", () => {
    expect(
      shouldRunMainEntry(
        "/Users/dev/project/node_modules/vitest/dist/workers/forks.js",
        "/Users/dev/project/crates/mcp/src/index.ts"
      )
    ).toBe(false);
  });
});
