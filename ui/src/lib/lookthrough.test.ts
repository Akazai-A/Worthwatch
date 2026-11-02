import { describe, expect, it } from "vitest";
import { caveats, compactMoney, familyOf, headlineParts, topBuckets, viaLabel, viaSummary } from "./lookthrough";
import type { Company, FundStatus } from "./api";

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
