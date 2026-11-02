import { describe, expect, it } from "vitest";
import { accountHint, bridgeSteps, changeLine, minorLine, parts, presetFrom, rangeLabel } from "./changes";
import type { AccountDiff, FundFeed } from "./api";

const acct = (over: Partial<AccountDiff>): AccountDiff => ({
  account: "a", name: "A", group: "cash", liability: false, currency: "CAD", from_value: "1", to_value: "2",
  change: "1", pct: "100", residual: "0", missing_price: false, missing_fx: false, ...over,
});

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
  it("explains an account row", () => {
    expect(accountHint(acct({ currency: "USD", market: "2653", fx: "-12" }), "CAD", [])).toBe("USD → CAD · market +$2,653 · fx −$12");
    expect(accountHint(acct({ market: "1465", trades: "742" }), "CAD", ["bought 14 XEQT.TO"])).toBe("market +$1,465 · bought 14 XEQT.TO +$742");
    expect(accountHint(acct({ from_value: null, balance: "500" }), "CAD", [])).toBe("new in this period");
  });
  it("folds small movers into one bridge step", () => {
    const many = Array.from({ length: 12 }, (_, i) => acct({ name: `A${i}`, change: String(100 - i * 5) }));
    const steps = bridgeSteps(many, 8);
    expect(steps).toHaveLength(9);
    expect(steps[8]).toEqual({ label: "4 others", delta: 60 + 55 + 50 + 45, folded: 4 });
    expect(bridgeSteps(many.slice(0, 9), 8)).toHaveLength(9);
    expect(bridgeSteps([acct({ change: "0" })])).toHaveLength(0);
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
