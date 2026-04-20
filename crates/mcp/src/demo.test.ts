import { describe, expect, it, beforeAll, afterAll } from "vitest";
import { spawnSync } from "child_process";
import { mkdtempSync, readFileSync, readdirSync, existsSync, rmSync } from "fs";
import { tmpdir } from "os";
import { dirname, join, resolve } from "path";
import { fileURLToPath } from "url";

// End-to-end test for `node dist/index.js --demo`.
// Runs the built binary with HOME overridden so fixture installation lands in
// a temp dir, then asserts the setup path:
//   - fixtures are copied into $HOME/.minutes/demo/
//   - the MCP config snippet prints with MEETINGS_DIR pointing at that dir
//   - all five fixtures carry the minutes_demo: true tag + ISO-8601 date with tz
//   - process exits 0

const __filename = fileURLToPath(import.meta.url);
const PKG_ROOT = resolve(dirname(__filename), "..");
const DIST_ENTRY = join(PKG_ROOT, "dist", "index.js");

let tempHome: string;

beforeAll(() => {
  tempHome = mkdtempSync(join(tmpdir(), "minutes-mcp-demo-test-"));
});

afterAll(() => {
  if (tempHome && existsSync(tempHome)) {
    rmSync(tempHome, { recursive: true, force: true });
  }
});

describe("`minutes-mcp --demo`", () => {
  it.skipIf(!existsSync(DIST_ENTRY))(
    "copies fixtures, prints config with MEETINGS_DIR override, and exits",
    () => {
      const result = spawnSync("node", [DIST_ENTRY, "--demo"], {
        env: { ...process.env, HOME: tempHome },
        encoding: "utf8",
        timeout: 30000,
      });

      expect(result.status).toBe(0);
      expect(result.stdout).toContain("Demo corpus ready at:");
      expect(result.stdout).toContain("MEETINGS_DIR");
      expect(result.stdout).toContain("npx");
      expect(result.stdout).toContain("minutes-mcp");

      const demoDir = join(tempHome, ".minutes", "demo");
      expect(existsSync(demoDir)).toBe(true);

      const fixtures = readdirSync(demoDir).filter((f) => f.endsWith(".md"));
      expect(fixtures.length).toBeGreaterThanOrEqual(5);

      for (const name of fixtures) {
        const content = readFileSync(join(demoDir, name), "utf8");
        expect(content).toMatch(/^---\n/);
        expect(content).toMatch(/minutes_demo:\s*true/);
        // Schema requires ISO-8601 with timezone offset or Z.
        expect(content).toMatch(
          /date:\s*\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:Z|[+-]\d{2}:\d{2})/
        );
      }
    }
  );
});
