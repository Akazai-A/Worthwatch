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
  /** How the chat panel is set up (settings.assistant). */
  assistant: AssistantView;
  /** Proposals from MCP waiting for Approve or Discard, oldest first. */
  pending: Pending[];
}

// ---- Chat (POST /api/chat) and pending proposals (MCP) ----
export interface AssistantView { provider: "ollama" | "cloud" | "shorthand"; model: string | null; url: string | null; key_present: boolean; offline: boolean }
export type RowKind = "balance" | "trade" | "price" | "new_account" | "note";
export interface PlanRow2 {
  kind: RowKind;
  account: string | null;
  account_name: string;
  detail: string | null;
  source: string | null;
  liability: boolean;
  currency: string;
  from: string | null;
  to: string | null;
  delta: string | null;
  price: string | null;
  /** Signed effect on net worth, base currency. */
  effect: string | null;
}
export interface Rejected { source: string | null; reason: string; hint: string | null }
export interface Plan {
  date: string;
  rows: PlanRow2[];
  mutations: Mutation[];
  rejected: Rejected[];
  net_change: string | null;
  net_worth_after: string | null;
  answer: string | null;
}
export interface SendPreview { url: string; model: string; prompt: { system: string; user: string } }
export interface ChatReply {
  text: string;
  provider: string;
  /** ollama | cloud | shorthand | none (preview only) */
  used: string;
  note: string | null;
  plan: Plan | null;
  preview: SendPreview | null;
}
export interface Pending { id: number; created: string; origin: string; text: string; plan: Plan }

/** Ask for a proposal. Writes nothing. `send` = consent to the cloud preview. */
export async function chat(text: string, send = false, date?: string): Promise<ChatReply> {
  const r = await fetch("/api/chat", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ text, send, date: date ?? null }),
  });
  if (!r.ok) await fail(r);
  return r.json();
}

/** Approve (applies through the one write path) or discard a pending MCP proposal. */
export async function decidePending(id: number, action: "approve" | "discard"): Promise<LedgerView> {
  const r = await fetch(`/api/pending/${id}/${action}`, { method: "POST" });
  if (!r.ok) await fail(r);
  return r.json();
}

// ---- Look-through (Portfolio › Look-through / Exposure / Overlap), from GET /api/lookthrough ----
export type AssetClass = "equity" | "fixed_income" | "cash" | "other";
export interface Via { path: string[]; weight_pct: string; value: string }
export interface Company {
  key: string;
  name: string;
  ticker?: string;
  sector: string;
  /** Colour slot: tech | financials | health | industrials | consumer | resources | communication | other | fixed */
  slot: string;
  country?: string;
  region: string;
  asset_class: AssetClass;
  currency?: string;
  value: string;
  pct: string;
  undisclosed: boolean;
  via: Via[];
}
export interface Bucket { label: string; slot?: string; value: string; pct: string }
export interface FundStatus {
  symbol: string;
  name?: string;
  value: string;
  resolved: boolean;
  as_of?: string;
  source?: string;
  constituents: number;
  children: string[];
  unresolved_children: string[];
  coverage_pct?: string;
  effective_holdings?: string;
}
export interface LookThrough {
  depth: number;
  total: string;
  as_of?: string;
  funds: FundStatus[];
  companies: Company[];
  company_count: number;
  undisclosed: string;
  unresolved: string;
  by_sector: Bucket[];
  by_region: Bucket[];
  by_country: Bucket[];
  by_asset_class: Bucket[];
  by_currency: Bucket[];
  top10_pct: string;
  effective_holdings?: string;
}
export interface SharedName { name: string; a_pct: string; b_pct: string }
export interface OverlapCell { pct: string; shared_value: string; shared: SharedName[] }
export interface Overlap { symbols: string[]; cells: (OverlapCell | null)[][]; effective_holdings: (string | null)[] }
export interface SnapshotInfo { symbol: string; name: string | null; as_of: string; source: string; constituents: number; dates: number }
export interface LookThroughView {
  version: number;
  as_of: string;
  base_currency: string;
  holdings_dir: string;
  offline: boolean;
  fetched_at: string | null;
  holdings: { symbol: string; value: string; currency: string | null }[];
  lookthrough: LookThrough;
  as_held: LookThrough;
  overlap: Overlap;
  errors: Record<string, string>;
  snapshots: SnapshotInfo[];
}

export async function getLookthrough(): Promise<LookThroughView> {
  const r = await fetch("/api/lookthrough");
  if (!r.ok) await fail(r);
  return r.json();
}

/** Re-read data/holdings and fetch issuer files for every held fund now. Never fails on a bad fund; see `errors`. */
export async function refreshHoldings(): Promise<LookThroughView> {
  const r = await fetch("/api/refresh-holdings", { method: "POST" });
  if (!r.ok) await fail(r);
  return r.json();
}

// ---- Changes (GET /api/changes?from&to&threshold) ----
export interface AccountDiff {
  account: string;
  name: string;
  group: Group;
  liability: boolean;
  currency: string;
  from_value: string | null;
  to_value: string | null;
  /** Signed contribution to net worth: paying down debt is positive. */
  change: string;
  pct: string | null;
  balance?: string;
  market?: string;
  trades?: string;
  fx?: string;
  residual: string;
  missing_price: boolean;
  missing_fx: boolean;
}
export interface HoldingDiff {
  symbol: string;
  currency: string;
  accounts: string[];
  from_units: string;
  to_units: string;
  from_price: string | null;
  to_price: string | null;
  from_price_date: string | null;
  to_price_date: string | null;
  from_value: string | null;
  to_value: string | null;
  trade_count: number;
  market: string | null;
  trades: string | null;
  fx: string | null;
  change: string | null;
}
export interface Changes {
  from: string;
  to: string;
  days: number;
  from_net_worth: string;
  to_net_worth: string;
  change: string;
  pct: string | null;
  market: string;
  trades: string;
  fx: string;
  balances: string;
  debt: string;
  residual: string;
  accounts: AccountDiff[];
  holdings: HoldingDiff[];
  unpriced: string[];
}
export interface BucketDelta { label: string; slot?: string; from_pct: string; to_pct: string; delta: string; from_value: string; to_value: string }
export interface Drift {
  from_total: string;
  to_total: string;
  from_as_of?: string;
  to_as_of?: string;
  by_sector: BucketDelta[];
  by_region: BucketDelta[];
  by_country: BucketDelta[];
  by_asset_class: BucketDelta[];
  by_currency: BucketDelta[];
}
export type ChangeKind = "added" | "removed" | "reweighted";
export interface ConstituentChange {
  kind: ChangeKind;
  key: string;
  name: string;
  ticker?: string;
  from_weight: string;
  to_weight: string;
  /** Percentage points, signed. */
  delta: string;
  your_value?: string;
}
export interface FundFeed {
  symbol: string;
  name?: string;
  from: string;
  to: string;
  from_constituents: number;
  to_constituents: number;
  threshold: string;
  changes: ConstituentChange[];
  minor_added: number;
  minor_removed: number;
  minor_reweighted: number;
  turnover_pct: string;
  your_value?: string;
  held: boolean;
  hidden: number;
}
export interface ChangesView {
  version: number;
  as_of: string;
  base_currency: string;
  from: string;
  to: string;
  checkins: string[];
  first_date: string | null;
  threshold: string;
  changes: Changes;
  drift: Drift;
  as_held_drift: Drift;
  funds_resolved_from: number;
  funds_resolved_to: number;
  funds: FundFeed[];
  same_file: string[];
  no_earlier_file: string[];
  errors: Record<string, string>;
}
export interface ChangesParams { from?: string; to?: string; threshold?: string }

export async function getChanges(p: ChangesParams = {}): Promise<ChangesView> {
  const q = new URLSearchParams();
  if (p.from) q.set("from", p.from);
  if (p.to) q.set("to", p.to);
  if (p.threshold != null) q.set("threshold", p.threshold);
  const r = await fetch(`/api/changes${q.size ? `?${q}` : ""}`);
  if (!r.ok) await fail(r);
  return r.json();
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
