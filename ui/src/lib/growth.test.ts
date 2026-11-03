import { describe, expect, it } from "vitest";
import { lastMonths, rangeCut, splitSince } from "./growth";

const pts = [
  { date: "2026-01-01", net_worth: "1000", added: "0" },
  { date: "2026-02-01", net_worth: "1300", added: "200" },
  { date: "2026-03-01", net_worth: "1100", added: "350" },
];

describe("added vs grown", () => {
  it("splits the move into what you added and the rest", () => {
    expect(splitSince(pts, null)).toEqual({ from: "2026-01-01", to: "2026-03-01", start: 1000, end: 1100, added: 350, grown: -250 });
  });
  it("starts from the point before the cut, like the chart", () => {
    const s = splitSince(pts, "2026-02-15");
    expect(s?.from).toBe("2026-02-01");
    expect(s?.added).toBe(150);
    expect(s?.grown).toBe(-350);
  });
  it("needs two points", () => {
    expect(splitSince(pts.slice(0, 1), null)).toBeNull();
  });
  it("cuts 3M and 1Y back from the last date", () => {
    expect(rangeCut("3m", "2026-11-03")).toBe("2026-08-03");
    expect(rangeCut("1y", "2026-11-03")).toBe("2025-11-02");
    expect(rangeCut("all", "2026-11-03")).toBeNull();
  });
});

describe("last months", () => {
  it("fills missing months with zero and ends at the as-of month", () => {
    const m = lastMonths([{ month: "2026-10", added: "500", grown: "-20" }], "2026-11-03", 3);
    expect(m).toEqual([
      { month: "2026-09", added: 0, grown: 0 },
      { month: "2026-10", added: 500, grown: -20 },
      { month: "2026-11", added: 0, grown: 0 },
    ]);
  });
});
