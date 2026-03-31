import { describe, expect, it } from "vitest";

import { MEETING_INSIGHT_KINDS } from "./index.js";

describe("meeting insight contract", () => {
  it("exports only the insight kinds the pipeline emits today", () => {
    expect(MEETING_INSIGHT_KINDS).toEqual(["decision", "commitment", "question"]);
  });
});
