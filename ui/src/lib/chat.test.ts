import { describe, expect, it } from "vitest";
import { editable, editPlan, hasWork, onlySpends, providerLine, rowHint, rowTitle, spendOf } from "./chat";
import type { Plan, PlanRow2 } from "./api";

const row = (over: Partial<PlanRow2>): PlanRow2 => ({
  kind: "balance", account: "td-chq", account_name: "TD Chequing", detail: null, source: null, liability: false,
  currency: "CAD", from: "4273.2", to: "5102", delta: "828.8", price: null, effect: "828.8", ...over,
});

const plan = (): Plan => ({
  date: "2026-11-02",
  rows: [
    row({}),
    row({ account: "td-visa", account_name: "TD Visa", liability: true, from: "1240", to: "640", delta: "-600", effect: "600", source: "visa 640" }),
    row({ kind: "trade", account: "ws-tfsa", account_name: "Wealthsimple TFSA", detail: "XEQT.TO", from: "426.5", to: "406.5", delta: "-20", price: "34.87", effect: "-697.4" }),
  ],
  mutations: [
    { op: "set_balance", date: "2026-11-02", account: "td-chq", value: "5102" },
    { op: "set_balance", date: "2026-11-02", account: "td-visa", value: "640" },
    { op: "record_position_change", change: { date: "2026-11-02", account: "ws-tfsa", symbol: "XEQT.TO", units: "-20", price: null, note: null } },
  ],
  spends: [],
  rejected: [],
  net_change: "731.4",
  net_worth_after: "413681.4",
  answer: null,
});

describe("rows", () => {
  it("describe trades with their price and keep the words they came from for hover", () => {
    expect(rowHint(plan().rows[2])).toBe("sell 20 @ 34.87");
    expect(rowHint(plan().rows[1])).toBe("from “visa 640”");
  });
});

describe("editPlan", () => {
  it("rewrites a balance, its mutation and the totals", () => {
    const p = editPlan(plan(), { 1: "500" }, {}, "CAD")!;
    expect(p.rows[1].to).toBe("500");
    expect(p.rows[1].delta).toBe("-740");
    expect(p.rows[1].effect).toBe("740");
    expect(p.mutations).toContainEqual({ op: "set_balance", date: "2026-11-02", account: "td-visa", value: "500" });
    expect(p.mutations.filter((m) => m.op === "set_balance")).toHaveLength(2);
    expect(p.net_change).toBe("871.4");
    expect(p.net_worth_after).toBe("413821.4");
  });
  it("turns a corrected unit count into the trade size", () => {
    const p = editPlan(plan(), { 2: "416.5" }, {}, "CAD")!;
    const m = p.mutations[2];
    expect(m.op === "record_position_change" && m.change.units).toBe("-10");
    expect(p.rows[2].effect).toBe("-348.7");
  });
  it("drops the write when a balance is put back to what it was", () => {
    const p = editPlan(plan(), { 0: "4,273.20" }, {}, "CAD")!;
    expect(p.mutations.some((m) => m.op === "set_balance" && m.account === "td-chq")).toBe(false);
    expect(p.rows[0].delta).toBe("0");
  });
  it("refuses text that is not a number", () => {
    expect(editPlan(plan(), { 0: "lots" }, {}, "CAD")).toBeNull();
    expect(editPlan(plan(), { 0: "-5" }, {}, "CAD")).toBeNull();
  });
});

describe("providerLine", () => {
  it("says where the message goes", () => {
    expect(providerLine({ provider: "ollama", model: "llama3.1", url: null, key_present: false, offline: false }).text).toContain("nothing leaves");
    expect(providerLine({ provider: "cloud", model: "m", url: null, key_present: false, offline: false }).text).toContain("ANTHROPIC_API_KEY not set");
    expect(providerLine({ provider: "cloud", model: "m", url: null, key_present: true, offline: true }).dot).toBe("off");
  });
});

describe("purchases", () => {
  const coffee = row({ kind: "spend", account: null, account_name: "Cash", detail: "Coffee", from: null, to: "6", delta: null, effect: "0", source: "coffee 6 cash" });
  const lunch = row({ kind: "spend", account: "td-visa", account_name: "TD Visa", detail: "Lunch", from: null, to: "14", delta: null, effect: "0" });
  const spends: Plan["spends"] = [
    { date: "2026-11-02", merchant: "Coffee", amount: "6", account: null, category: "coffee" },
    { date: "2026-11-02", merchant: "Lunch", amount: "14", account: "td-visa", category: "dining" },
  ];

  it("a card of only purchases is about the budget and can be approved with no ledger change", () => {
    const p: Plan = { ...plan(), rows: [coffee, lunch], mutations: [], spends, net_change: "0" };
    expect(onlySpends(p)).toBe(true);
    expect(hasWork(p)).toBe(true);
    expect(rowTitle(coffee)).toBe("Coffee");
    expect(editable(coffee)).toBe(false);
    expect(spendOf(p, 1)?.category).toBe("dining");
  });

  it("finds the purchase behind a row when balances come first", () => {
    const p: Plan = { ...plan(), rows: [row({}), lunch], spends: [spends[1]] };
    expect(onlySpends(p)).toBe(false);
    expect(spendOf(p, 0)).toBeNull();
    expect(spendOf(p, 1)?.merchant).toBe("Lunch");
  });

  it("a plan with nothing to save can't be approved", () => {
    expect(hasWork({ ...plan(), mutations: [], spends: [] })).toBe(false);
  });
});
