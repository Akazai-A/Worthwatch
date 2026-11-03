import { describe, expect, it } from "vitest";
import { fan, milestoneSplit, niceStep, payoffLine, roomBar, roomLine, yearsBetween } from "./plan";
import { match, rank, runs, type Item } from "../palette";
import type { DebtPayoff, Milestone, RoomView } from "../api";

const room = (over: Partial<RoomView>): RoomView => ({
  kind: "tfsa", label: "TFSA", as_of: "2026-01-01", reported: "13500", added: "0", used: "5000", left: "8500",
  planned: "500", monthly_plan: "500", runs_out: null, over: false, estimated: false, note: null, accounts: [], ...over,
});

describe("plan layout", () => {
  it("splits a milestone into today, money put in and growth", () => {
    const m: Milestone = { years: 1, date: "2027-11-03", net_worth: "340000", low: "0", high: "0", contributed: "21000" };
    const s = milestoneSplit("280000", m);
    expect(s).toEqual({ today: 280000, putIn: 21000, growth: 39000, total: 340000 });
  });

  it("draws room as used, planned up to what is left, and left", () => {
    const b = roomBar(room({}));
    expect(b.room).toBe(13500);
    expect(b.used + b.planned + b.left).toBeCloseTo(100);
    expect(b.planned).toBeCloseTo((500 / 13500) * 100);
    // A plan bigger than the room left only fills what is left.
    const full = roomBar(room({ planned: "20000", left: "1000", used: "12500" }));
    expect(full.left).toBe(0);
    expect(full.used + full.planned).toBeCloseTo(100);
  });

  it("shows an over-contribution as its own segment and says what it costs", () => {
    const r = room({ used: "14500", left: "-1000", over: true });
    expect(roomBar(r).over).toBeGreaterThan(0);
    expect(roomLine(r)).toMatch(/^over by \$1K/);
    expect(roomLine(room({ runs_out: "2026-07-15", planned: "4500", left: "2000" }))).toBe("$500/mo planned · full by Jul 2026");
    expect(roomLine(room({ monthly_plan: "0" }))).toBe("no recurring rule");
  });

  it("words a debt's end", () => {
    const d: DebtPayoff = { account: "m", name: "Mortgage", balance: "1", rate_pct: "4.9", monthly_payment: "2925", payoff: "2046-01-03", months: 230, interest: "1", yearly: [] };
    expect(payoffLine(d, 20)).toBe("paid off Jan 2046");
    expect(payoffLine({ ...d, payoff: null }, 20)).toBe("not paid off within 20 years");
    expect(payoffLine({ ...d, payoff: null, monthly_payment: "0", rate_pct: "0" }, 20)).toBe("no payment rule · stays flat");
  });

  it("counts years and builds the fan from history to the horizon", () => {
    expect(yearsBetween("2026-11-03", "2041-05-03")).toBe(14.5);
    const f = fan(
      [{ date: "2026-01-01", net_worth: "100", assets: "0", liabilities: "0", by_group: {} }],
      [{ date: "2026-12-01", net_worth: "200", low: "150", high: "260", contributed: "50" }],
      "2026-11-01", "120", 1000,
    );
    expect(f.proj[0].v).toBe(120);
    expect(f.yMax).toBeGreaterThan(1000);
    expect(f.yMin).toBe(0);
  });

  it("picks round axis steps", () => {
    expect(niceStep(2_200_000)).toBe(500_000);
    expect(niceStep(90_000)).toBe(20_000);
  });
});

describe("palette matching", () => {
  const noop = () => {};
  const items: Item[] = [
    { id: "a1", section: "Accounts", label: "Wealthsimple TFSA", run: noop },
    { id: "a2", section: "Accounts", label: "TD Chequing", hint: "TD", run: noop },
    { id: "g1", section: "Go to", label: "Portfolio › Look-through", run: noop },
    { id: "x1", section: "Actions", label: "Back up now", run: noop },
  ];

  it("prefers a whole-word hit over scattered letters", () => {
    expect(match("td", "TD Chequing")!.score).toBeGreaterThan(match("td", "Wealthsimple TFSA")?.score ?? -1e9);
    expect(match("tfsa", "Wealthsimple TFSA")!.marks).toEqual([13, 14, 15, 16]);
    expect(match("zz", "TD Chequing")).toBeNull();
  });

  it("matches letters in order across words", () => {
    expect(match("lkth", "Portfolio › Look-through")).not.toBeNull();
    expect(match("bun", "Back up now")!.marks).toEqual([0, 5, 8]);
  });

  it("ranks within sections in a fixed order", () => {
    const h = rank(items, "t");
    expect(h[0].item.section).toBe("Accounts");
    expect(rank(items, "backup").map((x) => x.item.id)).toEqual(["x1"]);
    expect(rank(items, "").length).toBe(4);
    // The section with the best hit leads, so Enter takes it.
    const wall: Item[] = [...items, { id: "g2", section: "Go to", label: "Wall view", run: noop }];
    expect(rank(wall, "wall")[0].item.id).toBe("g2");
  });

  it("splits a label into highlighted runs", () => {
    expect(runs("Back up now", [0, 5, 8])).toEqual([
      { text: "B", on: true }, { text: "ack ", on: false }, { text: "u", on: true }, { text: "p ", on: false }, { text: "n", on: true }, { text: "ow", on: false },
    ]);
  });
});
