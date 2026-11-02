import { describe, expect, it } from "vitest";
import { errorSummary, priceStatus, rateLabels } from "./market";
import type { MarketView } from "./api";

const base: MarketView = {
  price_provider: "yahoo",
  fx_provider: "bank_of_canada",
  offline: false,
  fetched_at: "2026-10-31T21:05:00Z",
  prices_as_of: "2026-10-31",
  prices_age_days: 2,
  rates: [{ currency: "USD", rate: "1.3912", date: "2026-10-31", source: "fetched" }],
  errors: {},
};

describe("priceStatus", () => {
  it("names the date when fresh", () => {
    expect(priceStatus(base)).toEqual({ text: "prices as of Oct 31", tone: "ok" });
  });
  it("warns when stale, never hides the date", () => {
    expect(priceStatus({ ...base, prices_age_days: 9 })).toEqual({ text: "prices as of Oct 31 · 9d old", tone: "warn" });
  });
  it("explains offline and off", () => {
    expect(priceStatus({ ...base, offline: true, prices_as_of: null }).text).toContain("offline");
    expect(priceStatus({ ...base, price_provider: "off", fx_provider: "off" }).text).toBe("price fetching off");
    expect(priceStatus({ ...base, prices_as_of: null }).text).toBe("no prices fetched yet");
  });
});

describe("rateLabels", () => {
  it("shows source and date for a fetched rate, manual otherwise, warns when missing", () => {
    expect(rateLabels(base, "CAD")[0]).toEqual({ text: "USD/CAD 1.3912 (Bank of Canada, Oct 31)", tone: "ok" });
    const manual = rateLabels({ ...base, rates: [{ currency: "USD", rate: "1.38", date: null, source: "manual" }] }, "CAD")[0];
    expect(manual).toEqual({ text: "USD/CAD 1.3800 (manual)", tone: "faint" });
    const missing = rateLabels({ ...base, rates: [{ currency: "EUR", rate: null, date: null, source: null }] }, "CAD")[0];
    expect(missing).toEqual({ text: "no EUR/CAD rate", tone: "warn" });
  });
});

describe("errorSummary", () => {
  it("lists up to three keys", () => {
    expect(errorSummary(base)).toBeNull();
    expect(errorSummary({ ...base, errors: { X: "e" } })).toBe("no quote for X");
    expect(errorSummary({ ...base, errors: { A: "", B: "", C: "", D: "" } })).toBe("no quotes for A, B, C and 1 more");
  });
});
