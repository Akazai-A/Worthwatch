import { describe, expect, it } from "vitest";
import { caveats, closestTwin, compactMoney, fundMix, overlapPairs, shortPath, familyOf, headlineParts, topBuckets, viaLabel, viaSummary } from "./lookthrough";
import type { Company, FundStatus, Overlap } from "../api";

const co = (via: string[][]): Company => ({
  key: "AAPL", name: "Apple", sector: "Technology", slot: "tech", region: "North America", asset_class: "equity",
  value: "100", pct: "1", undisclosed: false, via: via.map((path) => ({ path, weight_pct: "1", value: "1" })),
});
const fund = (symbol: string, resolved: boolean): FundStatus => ({ symbol, value: "1", resolved, constituents: 0, children: [], unresolved_children: [] });

describe("look-through wording", () => {
  it("names the path and summarises the top-level funds", () => {
    expect(viaLabel({ path: ["XEQT.TO", "XUS.TO"], weight_pct: "1", value: "1" })).toBe("XEQT.TO → XUS.TO");
    expect(viaSummary(co([["VTI"], ["XEQT.TO", "XUS.TO"], ["XEQT.TO", "ITOT"]]))).toBe("via VTI and XEQT.TO");
    expect(viaSummary(co([["VTI"]]))).toBe("via VTI");
  });
  it("compacts money", () => {
    expect(compactMoney("920.4")).toBe("$920");
    expect(compactMoney("4120")).toBe("$4.1k");
    expect(compactMoney("150000")).toBe("$150k");
    expect(compactMoney("1234567")).toBe("$1.2M");
  });
  it("builds the headline and caveats", () => {
    expect(headlineParts(2913, [fund("A", true), fund("B", true), fund("C", false)], "31.4", "210")).toEqual([
      "2,913 companies via 2 funds", "top 10 = 31.4%", "effective holdings ~210",
    ]);
    expect(caveats("4800", "228410", [fund("ZAG.TO", false)])).toEqual(["2.1% undisclosed", "ZAG.TO has no holdings file"]);
    expect(caveats("0", "100", [fund("A", true)])).toEqual([]);
  });
  it("groups slots into treemap families and folds long bucket lists", () => {
    expect(familyOf("communication")).toBe("Tech & communication");
    expect(familyOf("resources")).toBe("Industry, energy & materials");
    expect(familyOf("fixed")).toBe("Bonds & cash");
    expect(familyOf("whatever")).toBe("Other");
    const b = Array.from({ length: 10 }, (_, i) => ({ label: `c${i}`, value: "10", pct: "10.0" }));
    const t = topBuckets(b, 8);
    expect(t).toHaveLength(9);
    expect(t[8]).toEqual({ label: "2 others", value: "20.00", pct: "20.0" });
    expect(topBuckets(b.slice(0, 9), 8)).toHaveLength(9);
  });
});

describe("shortPath", () => {
  it("keeps short paths and trims long ones to the last two parts", () => {
    expect(shortPath("data/holdings")).toBe("data/holdings");
    expect(shortPath("/tmp/claude-0/x/y/seed/holdings")).toBe("…/seed/holdings");
  });
});

describe("caveats", () => {
  it("names up to three funds without a file and counts the rest", () => {
    const f = (symbol: string, resolved: boolean) => ({ symbol, resolved }) as FundStatus;
    expect(caveats("0", "100", [f("A", false)])).toEqual(["A has no holdings file"]);
    expect(caveats("0", "100", [f("A", false), f("B", false), f("C", true)])).toEqual(["A, B have no holdings file"]);
    expect(caveats("0", "100", ["A", "B", "C", "D", "E"].map((s) => f(s, false)))).toEqual(["A, B and 3 more have no holdings file"]);
  });
});

describe("funds, overlap and concentration", () => {
  const c = (key: string, slot: string, pct: string, via: [string, string][]): Company => ({
    key, name: key, sector: slot, slot, region: "", asset_class: "equity", value: "0", pct, undisclosed: false,
    via: via.map(([f, value]) => ({ path: [f], weight_pct: "1", value })),
  });
  const companies = [c("NVDA", "tech", "12", [["VTI", "60"], ["VFV", "30"]]), c("JPM", "financials", "3", [["VTI", "20"]]), c("XOM", "resources", "2", [["VTI", "20"]])];
  const overlap: Overlap = {
    symbols: ["VTI", "VFV", "ZAG"],
    cells: [
      [{ pct: "100", shared_value: "100", shared: [] }, { pct: "95", shared_value: "80", shared: [{ name: "NVDA", a_pct: "12", b_pct: "12" }] }, null],
      [{ pct: "95", shared_value: "60", shared: [] }, { pct: "100", shared_value: "60", shared: [] }, null],
      [null, null, null],
    ],
    effective_holdings: ["19", "20", null],
  };
  it("splits a fund into families by its disclosed dollars", () => {
    expect(fundMix(companies, "VTI")).toEqual([
      { family: "Tech & communication", share: 0.6 }, { family: "Financials", share: 0.2 }, { family: "Industry, energy & materials", share: 0.2 },
    ]);
    expect(fundMix(companies, "NONE")).toEqual([]);
  });
  it("lists each pair once, smaller side first, and skips funds without a file", () => {
    const p = overlapPairs(overlap);
    expect(p.map((q) => [q.a, q.b, q.pct, q.shared_value])).toEqual([["VFV", "VTI", 95, "60"]]);
    expect(closestTwin(p, "VTI")).toEqual({ other: "VFV", pct: 95 });
    expect(closestTwin(p, "ZAG")).toBeNull();
  });

});
