/** What every amount shows while amounts are hidden (⌘⇧H). */
export const MASK = "*****";

let hidden: () => boolean = () => false;
/** Points the formatters at the hide-amounts preference. It is read inside each call, so a
 *  template that formats an amount re-renders when the preference flips. */
export function hideAmountsWhen(source: () => boolean): void { hidden = source; }
export function amountsHidden(): boolean { return hidden(); }

/** Replaces dollar amounts inside prose (chat answers) while amounts are hidden. */
export function maskAmounts(text: string): string {
  return hidden() ? text.replace(/[+−-]?\$\d+(?:,\d{3})*(?:\.\d+)?(?:[KkMm](?![A-Za-z]))?/g, MASK) : text;
}

export function formatMoney(value: string | number, currency = "CAD", digits = 0): string {
  if (hidden()) return MASK;
  const n = typeof value === "string" ? Number(value) : value;
  const s = new Intl.NumberFormat("en-CA", {
    style: "currency",
    currency,
    currencyDisplay: "narrowSymbol",
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(Math.abs(n));
  return n < 0 ? `−${s}` : s;
}

/** "+$1,240" / "−$640" / "$0". */
export function formatSigned(value: string | number, currency = "CAD"): string {
  if (hidden()) return MASK;
  const n = typeof value === "string" ? Number(value) : value;
  if (n > 0) return `+${formatMoney(n, currency)}`;
  return formatMoney(n, currency);
}

/** Axis ticks: $12K, $1.2M, $850. */
export function formatCompact(n: number): string {
  if (hidden()) return MASK;
  const a = Math.abs(n);
  const sign = n < 0 ? "−" : "";
  if (a >= 1_000_000) return `${sign}$${trim((a / 1_000_000).toFixed(a >= 10_000_000 ? 0 : 1))}M`;
  if (a >= 1_000) return `${sign}$${trim((a / 1_000).toFixed(a >= 100_000 ? 0 : 1))}K`;
  return `${sign}$${Math.round(a)}`;
}

function trim(s: string): string {
  return s.replace(/\.0$/, "");
}

/** Plain number with grouping and 2 decimals, for inputs and tables. */
export function formatPlain(value: string | number, digits = 2): string {
  const n = typeof value === "string" ? Number(value) : value;
  return new Intl.NumberFormat("en-CA", { minimumFractionDigits: digits, maximumFractionDigits: digits }).format(n);
}

/** Accepts "5102", "5,102.10", "18.5k", "-640", "−640", "$1,200". Returns a decimal string or null. */
export function parseAmount(input: string): string | null {
  const s = input.trim().replace(/[,$\s]/g, "").replace(/−/g, "-").toLowerCase();
  const m = /^(-?\d+(?:\.\d+)?)(k|m)?$/.exec(s);
  if (!m) return null;
  const mult = m[2] === "k" ? 1000 : m[2] === "m" ? 1_000_000 : 1;
  return (Number(m[1]) * mult).toFixed(2);
}

/** Like parseAmount but keeps up to 6 decimals (units, prices). */
export function parseNumber(input: string, decimals = 6): string | null {
  const s = input.trim().replace(/[,$\s]/g, "").replace(/−/g, "-").toLowerCase();
  const m = /^(-?\d+(?:\.\d+)?)(k|m)?$/.exec(s);
  if (!m) return null;
  const mult = m[2] === "k" ? 1000 : m[2] === "m" ? 1_000_000 : 1;
  return String(Number((Number(m[1]) * mult).toFixed(decimals)));
}

export function formatDate(iso: string, style: "short" | "long" = "short"): string {
  const [y, m, d] = iso.split("-").map(Number);
  const date = new Date(Date.UTC(y, m - 1, d));
  return new Intl.DateTimeFormat("en-CA", {
    month: "short",
    day: "numeric",
    year: style === "long" ? "numeric" : undefined,
    timeZone: "UTC",
  }).format(date);
}

export function todayIso(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
