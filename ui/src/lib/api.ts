// Thin client. Types mirror server/src/routes.rs (Decimal and dates come as strings).
export type Group = "cash" | "investments" | "property" | "debt";
export const GROUPS: Group[] = ["cash", "investments", "property", "debt"];
export const GROUP_LABEL: Record<Group, string> = {
  cash: "Cash",
  investments: "Investments",
  property: "Property",
  debt: "Debt",
};

export type AccountKind =
  | "chequing" | "savings" | "tfsa" | "rrsp" | "fhsa" | "resp" | "non_registered" | "crypto"
  | "property" | "vehicle" | "other_asset"
  | "credit_card" | "line_of_credit" | "mortgage" | "loan" | "other_liability";

export const KIND_LABEL: Record<AccountKind, string> = {
  chequing: "Chequing", savings: "Savings", tfsa: "TFSA", rrsp: "RRSP", fhsa: "FHSA", resp: "RESP",
  non_registered: "Non-registered", crypto: "Crypto", property: "Property", vehicle: "Vehicle",
  other_asset: "Other asset", credit_card: "Credit card", line_of_credit: "Line of credit",
  mortgage: "Mortgage", loan: "Loan", other_liability: "Other liability",
};

export interface Diagnostic { line: number | null; message: string }
export interface Delta { from: string; amount: string; pct: string | null }
export interface HistoryPoint { date: string; value: string }
export type Source = "manual" | "fetched";
export interface Holding {
  symbol: string;
  units: string;
  price: string | null;
  price_date: string | null;
  price_source: Source | null;
  native_value: string | null;
  value: string | null;
  book_cost: string | null;
}
export interface Position { symbol: string; units: string; book_cost?: string | null }

export interface Account {
  id: string;
  name: string;
  institution?: string | null;
  kind: AccountKind;
  currency: string;
  tags?: string[];
  positions?: Position[] | null;
}

export interface AccountView extends Account {
  group: Group;
  liability: boolean;
  value: string;
  native_value: string;
  as_of: string | null;
  days_stale: number | null;
  missing_fx: boolean;
  missing_price: boolean;
  history: HistoryPoint[];
  holdings: Holding[];
}

export interface Point {
  date: string;
  net_worth: string;
  assets: string;
  liabilities: string;
  by_group: Partial<Record<Group, string>>;
}

export interface RateView { currency: string; rate: string | null; date: string | null; source: Source | null }
export interface MarketView {
  price_provider: string;
  fx_provider: string;
  offline: boolean;
  fetched_at: string | null;
  prices_as_of: string | null;
  prices_age_days: number | null;
  rates: RateView[];
  errors: Record<string, string>;
}

export interface AccountSlice { account: string; name: string; units: string; value: string | null; book_cost: string | null; gain: string | null }
export interface SecurityRow {
  symbol: string;
  currency: string;
  units: string;
  price: string | null;
  price_date: string | null;
  price_source: Source | null;
  value: string | null;
  weight_pct: string | null;
  gain: string | null;
  gain_pct: string | null;
  day_change: string | null;
  missing_fx: boolean;
  accounts: AccountSlice[];
}
export interface Slice { key: string; label: string; value: string; pct: string }
export interface Portfolio {
  total: string;
  day_change: string | null;
  book_cost: string | null;
  gain: string | null;
  gain_pct: string | null;
  by_currency: Slice[];
  by_account: Slice[];
  securities: SecurityRow[];
  unpriced: string[];
}

export interface LedgerView {
  version: number;
  as_of: string;
  base_currency: string;
  ledger_path: string;
  problem: Diagnostic | null;
  totals: { assets: string; liabilities: string; net_worth: string; by_group: Partial<Record<Group, string>> };
  series: Point[];
  deltas: { previous: Delta | null; ytd: Delta | null; one_year: Delta | null };
  accounts: AccountView[];
  /** Rates typed in the ledger (settings.fx): the fallback. `market.rates` is what is applied. */
  manual_fx: Record<string, string>;
  events: { date: string; note: string }[];
  market: MarketView;
  portfolio: Portfolio;
}

export type Mutation =
  | { op: "upsert_account"; account: Account }
  | { op: "remove_account"; id: string }
  | { op: "set_balance"; date: string; account: string; value: string }
  | { op: "set_price"; symbol: string; date: string; price: string }
  | { op: "set_fx"; currency: string; rate: string }
  | { op: "record_position_change"; change: { date: string; account: string; symbol: string; units: string; price?: string | null; note?: string | null } }
  | { op: "set_book_cost"; account: string; symbol: string; book_cost: string | null }
  | { op: "add_event"; event: { date: string; note: string } };

export interface PlanRow {
  line: number;
  pasted_symbol: string;
  symbol: string;
  new: boolean;
  current_units: string;
  units: string;
  delta: string;
  price: string | null;
  book_cost: string | null;
}
export interface PastePlan {
  rows: PlanRow[];
  untouched: string[];
  skipped: [number, string][];
  mutations: Mutation[];
  parsed_lines: number;
  total_lines: number;
}

export class ApiError extends Error {
  constructor(message: string, public status: number, public line: number | null = null) {
    super(message);
  }
}

async function fail(r: Response): Promise<never> {
  const e = await r.json().catch(() => ({ message: r.statusText, line: null }));
  throw new ApiError(e.message ?? r.statusText, r.status, e.line ?? null);
}

export async function getView(): Promise<LedgerView> {
  const r = await fetch("/api/view");
  if (!r.ok) await fail(r);
  return r.json();
}

export async function apply(mutations: Mutation[]): Promise<LedgerView> {
  const r = await fetch("/api/apply", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(mutations),
  });
  if (!r.ok) await fail(r);
  return r.json();
}

/** currency -> rate in effect (fetched if any, else typed), for browser-side estimates only. */
export function ratesInUse(v: LedgerView): Record<string, string> {
  const out: Record<string, string> = { ...v.manual_fx };
  for (const r of v.market.rates) if (r.rate != null) out[r.currency] = r.rate;
  return out;
}

/** Fetch prices and FX now. Never fails on a bad symbol; see `market.errors` in the view. */
export async function refresh(): Promise<LedgerView> {
  const r = await fetch("/api/refresh", { method: "POST" });
  if (!r.ok) await fail(r);
  return r.json();
}

/** Parse a pasted holdings export against one account. Writes nothing. */
export async function parseHoldings(account: string, date: string, text: string): Promise<PastePlan> {
  const r = await fetch("/api/parse-holdings", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ account, date, text }),
  });
  if (!r.ok) await fail(r);
  return r.json();
}

/** Calls `onChange` with the new version whenever the ledger changes. Returns a stop function. */
export function subscribe(onChange: (version: number) => void): () => void {
  const es = new EventSource("/api/events");
  es.addEventListener("change", (e) => onChange(Number((e as MessageEvent).data)));
  return () => es.close();
}
