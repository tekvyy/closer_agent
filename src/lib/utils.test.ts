import { describe, expect, test } from "bun:test";

import { toDate } from "./utils";

describe("toDate", () => {
  test("reads seconds and milliseconds as the same moment", () => {
    // Leads and people are stamped in seconds, jobs in milliseconds
    expect(toDate(1795017600).toISOString()).toBe("2026-11-18T16:00:00.000Z");
    expect(toDate(1795017600000).toISOString()).toBe("2026-11-18T16:00:00.000Z");
  });
});
