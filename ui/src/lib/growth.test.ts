import { describe, expect, it } from "vitest";
import { addedLine, lastMonths, windowTotals } from "./growth";

const pts = [
  { date: "2026-01-01", net_worth: "1000", added: "0" },
  { date: "2026-02-01", net_worth: "1300", added: "200" },
  { date: "2026-03-01", net_worth: "1100", added: "350" },
];

describe("added vs grown", () => {
  it("draws what you added from the window's first point", () => {
    expect(addedLine(pts.slice(1), pts)).toEqual([1300, 1450]);
  });
  it("carries the last known total over dates the growth series lacks", () => {
    expect(addedLine([...pts, { date: "2026-03-15", net_worth: "1200", added: "0" }], pts)).toEqual([1000, 1200, 1350, 1350]);
  });
  it("splits the window into what you added and the rest", () => {
    expect(windowTotals(pts, addedLine(pts, pts))).toEqual({ from: "2026-01-01", added: 350, grown: -250, fromYou: null });
  });
  it("gives your share only when both parts helped", () => {
    const up = [pts[0], pts[1]];
    expect(windowTotals(up, addedLine(up, pts))?.fromYou).toBe(67);
  });
  it("needs two points", () => {
    expect(windowTotals(pts.slice(0, 1), [1000])).toBeNull();
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
