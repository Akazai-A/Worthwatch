import { describe, expect, it } from "vitest";
import { accountBars, columns, fhsaLeft, nextYearAdds, preview, roomStatus, stripLabels, tfsaLifetime, valueBar } from "./registered";
import type { CraLimits, RegisteredView, RoomView } from "../api";

const LIMITS: CraLimits = {
  tfsa: [[2009, "5000"], [2010, "5000"], [2011, "5000"], [2012, "5000"], [2013, "5500"], [2014, "5500"], [2015, "10000"], [2016, "5500"], [2017, "5500"], [2018, "5500"], [2019, "6000"], [2020, "6000"], [2021, "6000"], [2022, "6000"], [2023, "6500"], [2024, "7000"], [2025, "7000"], [2026, "7000"], [2027, "7000"]],
  tfsa_first_year: 2009, fhsa_first_year: 2023, fhsa_annual: "8000", fhsa_carry: "8000", fhsa_lifetime: "40000",
};

const room = (over: Partial<RoomView> = {}): RoomView => ({
  kind: "tfsa", label: "TFSA", as_of: "2026-11-06", reported: "0", added: "0", used: "0", left: "0",
  planned: "0", monthly_plan: "0", runs_out: null, over: false, estimated: false, note: null, accounts: [], ...over,
});
const reg = (over: Partial<RegisteredView> = {}): RegisteredView => ({
  kind: "tfsa", label: "TFSA", value: "142300", accounts: [], since: 2009, put_in: "109000", growth: "33300", return_pct: "30.6",
  lifetime: "109000", lifetime_left: null, closes: null, years: [], room: room(), ...over,
});

describe("registered accounts", () => {
  it("splits value into put in and growth, or shows a loss past the value", () => {
    const b = valueBar(reg())!;
    expect(b.putIn + b.growth).toBeCloseTo(100);
    expect(b.growth).toBeCloseTo((33300 / 142300) * 100);
    const down = valueBar(reg({ value: "90000", put_in: "100000" }))!;
    expect(down.growth).toBe(0);
    expect(down.putIn + down.loss).toBeCloseTo(100);
    expect(valueBar(reg({ put_in: null }))).toBeNull();
  });

  it("names where the room stands", () => {
    expect(roomStatus(reg(), 2026)).toEqual({ tone: "done", text: "Maxed out" });
    expect(roomStatus(reg({ room: room({ left: "500" }) }), 2026)?.tone).toBe("room");
    expect(roomStatus(reg({ room: room({ left: "-500", over: true }) }), 2026)?.tone).toBe("over");
    expect(roomStatus(reg({ kind: "fhsa", lifetime_left: "16000" }), 2026)?.text).toBe("Maxed for 2026");
    expect(roomStatus(reg({ kind: "fhsa", lifetime_left: "0" }), 2026)?.text).toBe("Lifetime limit reached");
    expect(roomStatus(reg({ room: null }), 2026)).toBeNull();
  });

  it("scales year columns to the biggest limit and labels the ends and 2015", () => {
    const years = LIMITS.tfsa.map(([year, limit]) => ({ year, limit, used: year < 2027 ? limit : "0", estimated: year > 2026, future: year > 2026 }));
    const r = reg({ years });
    const cols = columns(r);
    expect(cols.find((c) => c.year === 2015)?.h).toBe(1);
    expect(cols.find((c) => c.year === 2009)?.h).toBe(0.5);
    expect(cols.at(-1)).toMatchObject({ future: true, fill: 0 });
    expect(stripLabels(r, 2026)).toEqual([2009, 2015, 2026]);
    expect(nextYearAdds(r)).toBe(7000);
  });

  it("draws accounts relative to the largest", () => {
    const bars = accountBars(reg({ accounts: [{ account: "a", name: "A", value: "40000" }, { account: "b", name: "B", value: "10000" }] }));
    expect(bars.map((b) => b.pct)).toEqual([100, 25]);
  });

  it("previews the setup answers with the same rules as the server", () => {
    expect(tfsaLifetime(2009, 2026, LIMITS)).toBe(109000);
    expect(tfsaLifetime(2020, 2026, LIMITS)).toBe(45500);
    expect(preview("tfsa", 1999, { kind: "maxed" }, 2026, LIMITS)).toEqual({ since: 2009, lifetime: 109000, left: 0, putIn: 109000, error: null });
    expect(preview("tfsa", 2009, { kind: "put_in", amount: "100000" }, 2026, LIMITS).left).toBe(9000);
    expect(preview("tfsa", 2009, { kind: "left", amount: "9000" }, 2026, LIMITS).putIn).toBe(100000);
    expect(preview("tfsa", 2009, { kind: "put_in", amount: "200000" }, 2026, LIMITS).error).toMatch(/more than/);
    expect(preview("fhsa", 2024, { kind: "maxed" }, 2026, LIMITS)).toMatchObject({ lifetime: 24000, putIn: 24000, left: 0 });
    expect(preview("fhsa", 2021, { kind: "maxed" }, 2026, LIMITS).error).toMatch(/2023 to 2026/);
    expect(fhsaLeft(2024, 0, 2026, LIMITS)).toBe(16000); // only $8,000 carries
    expect(fhsaLeft(2023, 36000, 2028, LIMITS)).toBe(4000); // the lifetime limit
  });
});
