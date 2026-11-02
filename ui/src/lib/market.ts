// Pure helpers for the market status line: what to say about price freshness.
import type { MarketView } from "./api";
import { formatDate } from "./money";

export type Tone = "ok" | "warn" | "faint";

/** Short label for "prices as of …", with a tone. Fetched prices older than `staleDays` warn. */
export function priceStatus(m: MarketView, staleDays = 4): { text: string; tone: Tone } {
  if (m.price_provider === "off" && m.fx_provider === "off") return { text: "price fetching off", tone: "faint" };
  if (!m.prices_as_of) {
    if (m.offline) return { text: "offline · manual prices only", tone: "faint" };
    return { text: "no prices fetched yet", tone: "faint" };
  }
  const age = m.prices_age_days ?? 0;
  const text = `prices as of ${formatDate(m.prices_as_of)}`;
  if (age > staleDays) return { text: `${text} · ${age}d old`, tone: "warn" };
  return { text, tone: "ok" };
}

/** "USD/CAD 1.3912 (Bank of Canada, Oct 31)" for each foreign currency in use. */
export function rateLabels(m: MarketView, base: string): { text: string; tone: Tone }[] {
  return m.rates.map((r) => {
    if (r.rate == null) return { text: `no ${r.currency}/${base} rate`, tone: "warn" };
    const rate = Number(r.rate).toFixed(4);
    const where = r.source === "fetched" ? `${providerLabel(m.fx_provider)}${r.date ? `, ${formatDate(r.date)}` : ""}` : "manual";
    return { text: `${r.currency}/${base} ${rate} (${where})`, tone: r.source === "fetched" ? "ok" : "faint" };
  });
}

export function providerLabel(id: string): string {
  switch (id) {
    case "yahoo": return "Yahoo Finance";
    case "stooq": return "Stooq";
    case "bank_of_canada": return "Bank of Canada";
    case "off": return "off";
    default: return id;
  }
}

/** One line listing what failed on the last refresh, or null. */
export function errorSummary(m: MarketView): string | null {
  const keys = Object.keys(m.errors);
  if (!keys.length) return null;
  const shown = keys.slice(0, 3).join(", ");
  return `${keys.length === 1 ? "no quote for" : "no quotes for"} ${shown}${keys.length > 3 ? ` and ${keys.length - 3} more` : ""}`;
}
