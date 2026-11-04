import { describe, expect, it } from "vitest";
import { changeLine, fundSteps, minorLine, parts, presetFrom, rangeLabel } from "./changes";
import type { FundFeed, HoldingDiff } from "../api";

describe("presets", () => {
  it("count back whole months and clamp to the first known date", () => {
    expect(presetFrom("1m", "2026-03-31", null, null)).toBe("2026-02-28");
    expect(presetFrom("3m", "2026-11-02", null, null)).toBe("2026-08-02");
    expect(presetFrom("1y", "2026-11-02", "2025-01-01", null)).toBe("2025-11-02");
    expect(presetFrom("1y", "2026-11-02", "2026-05-01", null)).toBe("2026-05-01");
    expect(presetFrom("ytd", "2026-11-02", null, null)).toBe("2026-01-01");
    expect(presetFrom("checkin", "2026-11-02", null, null)).toBeUndefined();
    expect(presetFrom("custom", "2026-11-02", null, { from: "2026-02-03", to: "2026-04-05" })).toBe("2026-02-03");
  });
  it("labels a range", () => {
    expect(rangeLabel("2026-09-30", "2026-11-02", 33)).toBe("Sep 30 → Nov 2, 2026 · 33 days");
    expect(rangeLabel("2025-11-02", "2026-11-02", 365)).toBe("Nov 2, 2025 → Nov 2, 2026 · 365 days");
  });
});

describe("wording", () => {
  it("lists only the non-zero parts of the change", () => {
    const p = parts({ market: "4118", trades: "0", fx: "-12", balances: "0.001", debt: "-300", residual: "0" });
    expect(p.map((x) => x.label)).toEqual(["market", "debt grew", "FX"]);
  });
  it("words a constituent change", () => {
    expect(changeLine({ kind: "reweighted", key: "XEC", name: "X", ticker: "XEC", from_weight: "7.37", to_weight: "7.92", delta: "0.55" })).toBe("XEC 7.37% → 7.92% (+0.55 pt)");
    expect(changeLine({ kind: "added", key: "CLS", name: "Celestica", ticker: "CLS", from_weight: "0", to_weight: "0.84", delta: "0.84" })).toBe("CLS added at 0.84%");
    expect(changeLine({ kind: "removed", key: "x", name: "Algonquin Power", from_weight: "0.11", to_weight: "0", delta: "-0.11" })).toBe("Algonquin Power removed (was 0.11%)");
  });
  it("sums up the moves under the threshold", () => {
    const f: FundFeed = { symbol: "VTI", from: "2026-09-30", to: "2026-10-31", from_constituents: 3600, to_constituents: 3612, threshold: "0.5", changes: [], minor_added: 2, minor_removed: 5, minor_reweighted: 0, turnover_pct: "0.4", held: true, hidden: 0 };
    expect(minorLine(f)).toBe("3,612 names · 2 added, 5 removed under 0.5 pt · 0.4% of the fund changed hands");
  });
});

describe("fund waterfall", () => {
  const h = (symbol: string, market: string, trades: string, fx: string): HoldingDiff => ({
    symbol, currency: "CAD", accounts: [], from_units: "1", to_units: "1", from_price: "1", to_price: "1", from_price_date: null, to_price_date: null,
    from_value: "1", to_value: "1", trade_count: 0, market, trades, fx, change: String(Number(market) + Number(trades) + Number(fx)),
  });
  it("makes one step per fund that moved, biggest first, coloured by what moved it", () => {
    const steps = fundSteps([h("VFV", "500", "0", "0"), h("XEQT", "10", "14000", "0"), h("ZAG", "0", "0", "0")]);
    expect(steps.map((s) => [s.label, s.delta, s.cause])).toEqual([["XEQT", 14010, "money"], ["VFV", 500, "market"]]);
  });

});
