import { describe, expect, it } from "vitest";
import { cup, glyph, jarCoins, jarFraction, paydayIn, pile, squares, tileTip, track } from "./game";
import type { Tile } from "../api";

describe("track", () => {
  it("snakes ten to a row and ends one step past the last day", () => {
    const t = track(30, 10, 760);
    expect(t.nodes).toHaveLength(30);
    expect(t.nodes[0].x).toBeLessThan(t.nodes[9].x);
    expect(t.nodes[10].y).toBeGreaterThan(t.nodes[9].y);
    expect(t.nodes[10].x).toBeCloseTo(t.nodes[9].x); // turns under itself
    expect(t.nodes[19].x).toBeCloseTo(t.nodes[0].x);
    expect(t.end.y).toBeGreaterThan(t.nodes[29].y); // 31st spot starts row four
    expect(t.path.match(/A/g)).toHaveLength(3);
    expect(Math.min(...t.nodes.map((n) => n.x))).toBeGreaterThanOrEqual(28);
  });
  it("fits a phone at five a row", () => {
    const t = track(31, 5, 350);
    expect(Math.max(...t.nodes.map((n) => n.x))).toBeLessThanOrEqual(350);
    expect(new Set(t.nodes.map((n) => n.y)).size).toBe(7);
  });
});

describe("coins", () => {
  it("piles up to five", () => {
    expect(pile(0)).toHaveLength(0);
    expect(pile(3)).toHaveLength(3);
    expect(pile(9)).toHaveLength(5);
  });
  it("fills the jar in alternating rows, the same every time", () => {
    const a = jarCoins(0.5, 40, 140, 180, 120);
    expect(a.length).toBeGreaterThan(10);
    expect(a).toEqual(jarCoins(0.5, 40, 140, 180, 120));
    expect(jarCoins(0, 40, 140, 180, 120)).toHaveLength(0);
    expect(Math.min(...jarCoins(2, 40, 140, 180, 120).map((c) => c.cy))).toBeGreaterThan(180 - 1.25 * 120);
  });
  it("measures the jar against the goal", () => {
    expect(jarFraction({ jar: "300", goal: "600", allowance: "2400" })).toBe(0.5);
    expect(jarFraction({ jar: "-40", goal: "600", allowance: "2400" })).toBe(0);
    expect(jarFraction({ jar: "240", goal: null, allowance: "2400" })).toBe(0.5);
  });
});

const tile = (over: Partial<Tile>): Tile => ({ day: 1, date: "2026-11-01", allowance: "82", spent: "0", net: "0", state: "ahead", coins: 0, bills: [], payday: false, ...over });

describe("tiles", () => {
  const money = (n: number) => `$${Math.round(n)}`;
  it("say what went in or out of the jar", () => {
    expect(tileTip(tile({ state: "green", spent: "47", net: "35", day: 12 }), money, "Nov")).toBe("Nov 12 · spent $47 of $82 · $35 into the jar");
    expect(tileTip(tile({ state: "red", spent: "100", net: "-18" }), money, "Nov")).toBe("Nov 1 · spent $100 of $82 · $18 out of the jar");
    expect(tileTip(tile({ bills: ["Rent"] }), money, "Nov")).toBe("Nov 1 · $82 to spend · Rent due (doesn't count)");
  });
  it("find the next payday", () => {
    const tiles = [tile({ day: 15, payday: true }), tile({ day: 30, payday: true })];
    expect(paydayIn({ tiles, day: 18 })).toBe(12);
    expect(paydayIn({ tiles, day: 30 })).toBeNull();
  });
});

describe("cards", () => {
  it("fill the cup and spill past the cap", () => {
    expect(cup({ rule: { kind: "cap", category: "coffee", amount: "25" }, spent: "18.75" })).toEqual({ fill: 0.75, spilled: false });
    expect(cup({ rule: { kind: "cap", category: "coffee", amount: "25" }, spent: "30" })).toEqual({ fill: 1, spilled: true });
  });
  it("tick off days and mark the one that broke it", () => {
    expect(squares({ days_done: 3, days_total: 5, status: "playing" })).toEqual(["done", "done", "done", "todo", "todo"]);
    expect(squares({ days_done: 1, days_total: 5, status: "lost" })).toEqual(["done", "broken", "todo", "todo", "todo"]);
  });
  it("pick an icon per category", () => {
    expect(glyph("coffee")).toBe("cup");
    expect(glyph(null)).toBe("sun");
    expect(glyph("pets")).toBe("coin");
  });
});
