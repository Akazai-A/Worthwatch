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

/** Added vs grown (GET /api/growth): `added` is the running total since the first point
 *  of what you put in (saving, contributions, debt paid down); the rest of the move is growth. */
export interface GrowthPoint { date: string; net_worth: string; added: string }
export async function getGrowth(): Promise<GrowthPoint[]> {
  const r = await fetch("/api/growth");
  if (!r.ok) await fail(r);
  return r.json();
}

/** One account, one calendar month (GET /api/accounts/{id}/months): contribution to net worth. */
export interface MonthSplit { month: string; added: string; grown: string }
export async function getAccountMonths(id: string): Promise<MonthSplit[]> {
  const r = await fetch(`/api/accounts/${encodeURIComponent(id)}/months`);
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
  | { op: "add_event"; event: { date: string; note: string } }
  | { op: "set_recurring"; account: string; rules: Recurring[] }
  | { op: "set_room"; kind: RoomKind; room: { as_of: string; amount: string } | null }
  | { op: "record_contribution"; contribution: { date: string; account: string; amount: string; note?: string | null } }
  | { op: "set_projection"; projection: ProjectionSettings }
  | { op: "set_budget_limit"; category: string; amount: string | null }
  | { op: "link_card"; card: string; account: string | null }
  | { op: "set_txn_source"; account: string; source: TxnSource }
  | { op: "set_category_rules"; rules: CategoryRule[] }
  | { op: "set_bills"; bills: Bill[] }
  | { op: "set_feed"; feed: Feed }
  | { op: "set_game_goal"; goal: string | null }
  | { op: "play_card"; card: PlayedCard }
  | { op: "pass_card"; id: string };

// ---- Budget (GET /api/budget) ----
export type TxnSource = "statement" | "apple_pay_statement" | "apple_pay";
export const SOURCE_LABEL: Record<TxnSource, string> = { statement: "Statement", apple_pay_statement: "Apple Pay + statement", apple_pay: "Apple Pay only" };
export type FeedMode = "off" | "icloud" | "tailscale";
export interface Feed { mode: FeedMode; icloud_dir?: string | null; tailscale_url?: string | null }
export interface CategoryRule { contains: string; category: string; account?: string | null }
export interface Bill { name: string; amount: string; every?: Every; category?: string; matches?: string | null; day?: number | null }
export interface BudgetSettings {
  limits?: Record<string, string>;
  cards?: Record<string, string>;
  sources?: Record<string, TxnSource>;
  rules?: CategoryRule[];
  bills?: Bill[];
  feed?: Feed;
}
export type TxnStatus = "provisional" | "confirmed" | "posted" | "review" | "tap_only" | "unlinked";
export interface Txn {
  id: string; account: string | null; card: string | null; date: string; amount: string; currency: string;
  merchant: string; description: string | null; category: string; status: TxnStatus;
  tap_id: string | null; tap_amount: string | null; tap_currency: string | null; tap_date: string | null;
  base_amount: string | null;
}
export interface CategoryLine { category: string; label: string; spent: string; provisional: string; limit: string | null; history: string[]; count: number }
export interface BillStatus { name: string; amount: string; category: string; paid_on: string | null; paid_amount: string | null; due_day: number | null }
export interface UnlinkedCard { card: string; taps: number; last: string; total: string }
export interface AccountFeed { id: string; name: string; kind: AccountKind; group: Group; source: TxnSource; cards: string[]; last_statement: string | null; statement_rows: number }
export interface BudgetView {
  version: number; as_of: string; base_currency: string;
  month: string; days: number; day: number; spent: string; provisional: string; income: string; budget: string; pace: string; pace_by_day: string[];
  categories: CategoryLine[]; daily: (string | null)[]; cumulative: (string | null)[]; cumulative_provisional: (string | null)[];
  bills: BillStatus[]; txns: Txn[]; review: Txn[]; unlinked: UnlinkedCard[]; missing_fx: number;
  first_month: string | null; options: CategoryOption[];
  accounts: AccountFeed[]; settings: BudgetSettings;
  feed: { mode: FeedMode; last_tap: string | null; taps: number };
  store_errors: string[];
}
export interface CategoryOption { id: string; label: string }

export async function getBudget(month?: string): Promise<BudgetView> {
  const r = await fetch(`/api/budget${month ? `?month=${month}` : ""}`);
  if (!r.ok) await fail(r);
  return r.json();
}

export type Preset = "auto" | "td" | "rbc" | "cibc" | "scotiabank" | "bmo" | "tangerine" | "amex" | "simplii" | "eq" | "generic" | "ofx";
export interface ImportPreview {
  preset: Preset; preset_label: string; presets: [Preset, string][]; sign_guessed: boolean; flipped: boolean;
  rows: { date: string; amount: string; description: string; category: string; duplicate: boolean }[];
  skipped: [number, string][]; new_rows: number; duplicates: number; money_out: string; money_in: string;
  from: string | null; to: string | null;
}
export interface ImportRequest { account: string; text: string; preset: Preset; flip: boolean }
async function post<T>(url: string, body?: unknown): Promise<T> {
  const r = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!r.ok) await fail(r);
  return r.json();
}
export const previewImport = (req: ImportRequest) => post<ImportPreview>("/api/budget/import/preview", req);
export const importStatement = (req: ImportRequest) => post<{ added: number; duplicates: number }>("/api/budget/import", req);
export const setCategory = (id: string, category: string | null) => post<{ ok: boolean }>("/api/budget/category", { id, category });

// ---- Budget game (GET /api/game) ----
export type CardRule =
  | { kind: "cap"; category: string; amount: string }
  | { kind: "no_spend"; category?: string | null; weekdays: number[] };
export interface PlayedCard { id: string; week: string; from?: string | null; rule: CardRule }
export type TileState = "green" | "red" | "today" | "ahead";
export interface Tile { day: number; date: string; allowance: string; spent: string; net: string; state: TileState; coins: number; bills: string[]; payday: boolean }
export type CardStatus = "offer" | "playing" | "won" | "lost";
export interface GameCard {
  id: string; week: string; from: string | null; rule: CardRule; title: string; status: CardStatus; art: "cup" | "days"; category: string | null;
  spent: string; days_done: number; days_total: number; usual: string; saved: string;
}
export interface MonthClose { jar: string; goal: string | null; hit: boolean; green: number; days: number; cards_won: number; cards_played: number; streak: number; next_goal: string | null }
export interface GameView {
  version: number; base_currency: string; as_of: string; first_month: string | null;
  month: string; days: number; day: number; goal: string | null; allowance: string; weights: string[];
  tiles: Tile[]; jar: string;
  today: { allowance: string; spent: string; left: string; taps: { merchant: string; amount: string; category: string }[] } | null;
  yesterday: Tile | null; hand: GameCard[]; offers: GameCard[]; shelf: GameCard[]; closed: MonthClose | null; suggestions: string[];
}

export async function getGame(month?: string): Promise<GameView> {
  const r = await fetch(`/api/game${month ? `?month=${month}` : ""}`);
  if (!r.ok) await fail(r);
  return r.json();
}

// ---- Settings (GET /api/settings) ----
export interface FeedStatus {
  dir: string | null; found: boolean; files: number; not_downloaded: string[];
  last_scan: string | null; last_added: string | null; errors: string[];
}
export interface SettingsView {
  mode: FeedMode; token: string; ingest_path: string; icloud_dir: string | null; icloud: FeedStatus;
  tailscale_url: string | null; tailscale_detected: boolean; can_sign: boolean; shortcut_file: string | null; day_shortcut_file: string | null;
  last_tap: string | null; taps: number;
}
export async function getSettings(): Promise<SettingsView> {
  const r = await fetch("/api/settings");
  if (!r.ok) await fail(r);
  return r.json();
}
export const rotateToken = () => post<SettingsView>("/api/settings/token");
export const makeShortcut = () => post<SettingsView>("/api/settings/shortcut");

// ---- Plan (GET /api/plan) ----
export type Every = "week" | "two_weeks" | "month" | "year";
export const EVERY_LABEL: Record<Every, string> = { week: "week", two_weeks: "2 weeks", month: "month", year: "year" };
export type RoomKind = "tfsa" | "rrsp" | "fhsa";
export interface Recurring { account: string; amount: string; every: Every; note?: string | null }
export interface RecurringView extends Recurring { name: string; liability: boolean; monthly: string | null }
export interface ProjectionSettings {
  return_pct: string;
  cash_pct: string;
  property_pct: string;
  rates?: Record<string, string>;
  target?: string | null;
}
export interface ProjPoint { date: string; net_worth: string; low: string; high: string; contributed: string }
export interface Milestone extends ProjPoint { years: number }
export interface DebtPayoff {
  account: string; name: string; balance: string; rate_pct: string; monthly_payment: string;
  payoff: string | null; months: number | null; interest: string; yearly: string[];
}
export interface ProjAccount { account: string; name: string; group: Group; rate_pct: string; monthly: string; start: string; end: string }
export interface Projection {
  start: string; start_net_worth: string; return_pct: string; band_pct: string; extra_monthly: string;
  monthly_saving: string; points: ProjPoint[]; milestones: Milestone[]; debts: DebtPayoff[];
  accounts: ProjAccount[]; target: string | null; target_date: string | null; skipped: string[];
}
export interface RoomAccount { account: string; name: string; used: string; monthly_plan: string }
export interface RoomView {
  kind: RoomKind; label: string; as_of: string; reported: string; added: string; used: string; left: string;
  planned: string; monthly_plan: string; runs_out: string | null; over: boolean; estimated: boolean;
  note: string | null; accounts: RoomAccount[];
}
export interface PlanView { projection: Projection; room: RoomView[]; recurring: RecurringView[]; settings: ProjectionSettings }
export interface PlanParams { years?: number; return_pct?: string; extra?: string; target?: string }

export async function getPlan(p: PlanParams = {}): Promise<PlanView> {
  const q = new URLSearchParams();
  if (p.years != null) q.set("years", String(p.years));
  if (p.return_pct != null) q.set("return_pct", p.return_pct);
  if (p.extra != null) q.set("extra", p.extra);
  if (p.target != null) q.set("target", p.target);
  const r = await fetch(`/api/plan${q.size ? `?${q}` : ""}`);
  if (!r.ok) await fail(r);
  return r.json();
}

// ---- Backups ----
export interface BackupInfo { name: string; bytes: number; created: string; date: string }
export async function listBackups(): Promise<BackupInfo[]> {
  const r = await fetch("/api/backups");
  if (!r.ok) await fail(r);
  return r.json();
}
export async function createBackup(): Promise<BackupInfo[]> {
  const r = await fetch("/api/backups", { method: "POST" });
  if (!r.ok) await fail(r);
  return r.json();
}

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
