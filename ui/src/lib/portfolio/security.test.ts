import { describe, expect, it } from "vitest";
import { insideOf, quoteSymbol, window } from "./security";
import type { Company } from "../api";

const co = (key: string, via: [string[], string][]): Company => ({
  key, name: key, sector: "Technology", slot: "tech", region: "", asset_class: "equity", value: "0", pct: "0", undisclosed: false,
  via: via.map(([path, value]) => ({ path, weight_pct: "1", value })),
});

describe("a holding's page", () => {
  it("picks the quote symbol a company trades under", () => {
    expect(quoteSymbol({ ticker: "NVDA", currency: "USD", name: "Nvidia" })).toBe("NVDA");
    expect(quoteSymbol({ ticker: "RY", currency: "CAD", name: "Royal Bank" })).toBe("RY.TO");
    expect(quoteSymbol({ ticker: "BRK.B", currency: "USD", name: "Berkshire" })).toBe("BRK-B");
    expect(quoteSymbol({ ticker: "-", currency: "USD", name: "Cash" })).toBeNull();
    expect(quoteSymbol({ name: "Unknown" })).toBeNull();
  });
  it("lists what a fund holds, through the funds inside it", () => {
    const cs = [co("NVDA", [[["VTI"], "60"], [["XEQT.TO", "XUS.TO"], "5"]]), co("RY", [[["XEQT.TO", "XIC.TO"], "15"]]), co("AAPL", [[["VTI"], "40"]])];
    expect(insideOf(cs, "VTI").map((r) => [r.c.key, r.share])).toEqual([["NVDA", 60], ["AAPL", 40]]);
    expect(insideOf(cs, "XEQT.TO").map((r) => [r.c.key, r.value, r.share])).toEqual([["RY", 15, 75], ["NVDA", 5, 25]]);
    expect(insideOf(cs, "XUS.TO").map((r) => r.c.key)).toEqual(["NVDA"]);
  });
  it("cuts the closes to a range and reads the move", () => {
    const p = [{ date: "2026-01-01", close: "100" }, { date: "2026-10-05", close: "90" }, { date: "2026-11-01", close: "110" }];
    const w = window(p, "1m")!;
    expect([w.first, w.last, w.pct, w.high, w.low]).toEqual([90, 110, (20 / 90) * 100, 110, 90]);
    expect(window(p, "1y")!.pts).toHaveLength(3);
    expect(window([], "1y")).toBeNull();
  });
});
