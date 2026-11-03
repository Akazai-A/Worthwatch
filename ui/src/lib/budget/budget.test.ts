import { describe, expect, it } from "vitest";
import { bullet, byDay, chart, monthLabel, paceLine, shiftMonth } from "./budget";
import type { BudgetView, Txn } from "../api";

function view(over: Partial<BudgetView> = {}): BudgetView {
  const days = 30;
  const daily = Array.from({ length: days }, (_, i) => (i < 10 ? "50" : null));
  let run = 0;
  const cumulative = daily.map((d) => (d == null ? null : String((run += Number(d)))));
  const cumulative_provisional = daily.map((d, i) => (d == null ? null : i >= 8 ? String((i - 7) * 50) : "0"));
  return {
    version: 1, as_of: "2026-11-10", base_currency: "CAD", month: "2026-11-01", days, day: 10,
    spent: "500", provisional: "100", income: "0", budget: "1500", pace: "500", pace_by_day: Array.from({ length: days }, (_, i) => String((1500 * (i + 1)) / days)),
    categories: [], daily, cumulative, cumulative_provisional, bills: [], txns: [], review: [], unlinked: [],
    missing_fx: 0, first_month: null, options: [], accounts: [], settings: {},
    feed: { mode: "off", last_tap: null, taps: 0 }, store_errors: [], ...over,
  };
}

describe("months", () => {
  it("labels and steps across years", () => {
    expect(monthLabel("2026-11-01")).toBe("November 2026");
    expect(shiftMonth("2026-01-01", -1)).toBe("2025-12");
    expect(shiftMonth("2026-12-01", 1)).toBe("2027-01");
  });
});

describe("pace", () => {
  it("says what is left per day and how far off pace", () => {
    const p = paceLine(view());
    expect(p.left).toBe(1000);
    expect(p.perDay).toBeCloseTo(50);
    expect(p.vsPace).toBe(0);
    expect(paceLine(view({ spent: "1600" })).perDay).toBeNull();
  });
});

describe("bullet", () => {
  it("fills confirmed first, then taps, inside the limit", () => {
    const b = bullet({ spent: "300", provisional: "100", limit: "400" }, 0.5);
    expect(b.confirmed).toBe(50);
    expect(b.provisional).toBe(25);
    expect(b.over).toBe(0);
    expect(b.paceAt).toBe(50);
    expect(b.isOver).toBe(false);
  });
  it("shows an overspend as a tail past the limit mark", () => {
    const b = bullet({ spent: "500", provisional: "0", limit: "400" }, 1);
    expect(b.limitAt).toBe(80);
    expect(b.over).toBe(20);
    expect(b.confirmed + b.provisional + b.over).toBeCloseTo(100);
    expect(b.isOver).toBe(true);
  });
  it("works without a limit", () => {
    const b = bullet({ spent: "50", provisional: "50", limit: null }, 0.3);
    expect(b.provisional).toBe(100);
    expect(b.limitAt).toBeNull();
  });
});

describe("chart", () => {
  it("draws confirmed solid, taps dashed from the first tap day, and a pace line", () => {
    const c = chart(view(), 780, 230);
    expect(c.confirmed.startsWith("M")).toBe(true);
    expect(c.confirmed.split("L").length - 1).toBe(10);
    expect(c.total).not.toBeNull();
    expect(c.total!.split("L").length - 1).toBe(2); // days 9 and 10
    expect(c.pace).not.toBeNull();
    expect(c.bars).toHaveLength(10);
    expect(c.today!.x).toBeCloseTo(c.x(10));
    expect(c.max).toBeGreaterThanOrEqual(1500);
  });
  it("clips a rent-sized day instead of flattening the rest", () => {
    const daily = Array.from({ length: 30 }, (_, i) => (i === 0 ? "1950" : i < 20 ? String(40 + i) : null));
    const c = chart(view({ daily }), 780, 230, 34);
    expect(c.bars[0].clipped).toBe(true);
    expect(c.bars[0].h).toBe(30);
    expect(c.bars[5].h).toBeGreaterThan(10);
  });
  it("has no dashed line when everything is confirmed", () => {
    const v = view({ cumulative_provisional: Array(30).fill("0") });
    expect(chart(v, 780, 230).total).toBeNull();
  });
});

describe("byDay", () => {
  it("groups by date and sums spending only", () => {
    const t = (date: string, amount: string, category: string) => ({ date, base_amount: amount, category }) as Txn;
    const g = byDay([t("2026-11-03", "10", "dining"), t("2026-11-03", "500", "transfer"), t("2026-11-02", "5", "coffee")], (x) => x.category !== "transfer");
    expect(g.map((d) => [d.date, d.total, d.txns.length])).toEqual([["2026-11-03", 10, 2], ["2026-11-02", 5, 1]]);
  });
});
