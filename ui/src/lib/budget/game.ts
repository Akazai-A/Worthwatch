// Budget game layout: pure, so it is tested. The board is a snaking track of day tiles
// that ends at the jar; coins are drawn from counts the server sends.
import type { GameCard, GameView, Tile } from "../api";

export interface Node { day: number; x: number; y: number }
export interface Track { nodes: Node[]; end: Node; path: string; width: number; height: number }

/** Day tiles laid out `cols` to a row, left to right then right to left, with a round
 *  turn at each row end. The node after the last day is where the month closes. */
export function track(days: number, cols: number, width: number, rowH = 90, pad = 28, top = 44): Track {
  const step = (width - 2 * pad) / (cols - 1);
  const at = (i: number): Node => {
    const r = Math.floor(i / cols);
    const c = i % cols;
    const x = pad + (r % 2 === 0 ? c : cols - 1 - c) * step;
    return { day: i + 1, x, y: top + r * rowH };
  };
  const nodes = Array.from({ length: days }, (_, i) => at(i));
  const end = at(days);
  const all = [...nodes, end];
  const turn = rowH / 2;
  const parts = [`M${all[0].x.toFixed(1)} ${all[0].y.toFixed(1)}`];
  for (let i = 1; i < all.length; i++) {
    const a = all[i - 1], b = all[i];
    if (a.y === b.y) parts.push(`L${b.x.toFixed(1)} ${b.y.toFixed(1)}`);
    else {
      // Down to the next row: a half circle out past the row's end.
      const right = a.x > width / 2;
      parts.push(`A${turn} ${turn} 0 0 ${right ? 1 : 0} ${b.x.toFixed(1)} ${b.y.toFixed(1)}`);
    }
  }
  const rows = Math.ceil((days + 1) / cols);
  return { nodes, end, path: parts.join(" "), width, height: top + (rows - 1) * rowH + 52 };
}

/** Coins piled above a tile: up to five, a row of three with two on top. */
export function pile(coins: number): { dx: number; dy: number }[] {
  const spots = [{ dx: 0, dy: 0 }, { dx: -7, dy: 1 }, { dx: 7, dy: 1 }, { dx: -3.5, dy: -6 }, { dx: 3.5, dy: -6 }];
  return spots.slice(0, Math.max(0, Math.min(5, coins)));
}

/** Coins stacked in the jar, bottom up, to `fraction` of `height` (0..1.2). Same input,
 *  same pile, so the jar doesn't jitter between renders. */
export function jarCoins(fraction: number, left: number, right: number, bottom: number, height: number): { cx: number; cy: number }[] {
  const f = Math.max(0, Math.min(1.2, fraction));
  const rows = Math.round((f * height) / 6.5);
  const out: { cx: number; cy: number }[] = [];
  const span = right - left;
  for (let r = 0; r < rows; r++) {
    const offset = r % 2 === 0 ? 0 : span / 6;
    const per = r % 2 === 0 ? 3 : 2;
    for (let i = 0; i < per; i++) {
      const jitter = ((r * 7 + i * 13) % 5) - 2;
      out.push({ cx: left + offset + (span / 3) * i + span / 6 + jitter, cy: bottom - 6 - r * 6.5 });
    }
  }
  return out;
}

/** The next payday after today in this month, as days away. */
export function paydayIn(g: Pick<GameView, "tiles" | "day">): number | null {
  const t = g.tiles.find((x) => x.payday && x.day > g.day);
  return t ? t.day - g.day : null;
}

/** Days left in the month after today. */
export const daysLeft = (g: Pick<GameView, "days" | "day">) => Math.max(0, g.days - g.day);

/** How full the jar is against its goal, for the drawing. No goal: against the month's
 *  allowance / 5, so the jar still looks like something. */
export function jarFraction(g: Pick<GameView, "jar" | "goal" | "allowance">): number {
  const target = g.goal != null && Number(g.goal) > 0 ? Number(g.goal) : Number(g.allowance) / 5 || 1;
  return Math.max(0, Number(g.jar)) / target;
}

/** One line for a tile's tooltip. */
export function tileTip(t: Tile, money: (n: number) => string, mon: string): string {
  const head = `${mon} ${t.day}`;
  const extra = [t.bills.length ? `${t.bills.join(", ")} due (doesn't count)` : "", t.payday ? "payday" : ""].filter(Boolean).join(" · ");
  const tail = extra ? ` · ${extra}` : "";
  if (t.state === "ahead") return `${head} · ${money(Number(t.allowance))} to spend${tail}`;
  if (t.state === "today") return `${head} · today · ${money(Number(t.spent))} of ${money(Number(t.allowance))}${tail}`;
  const net = Number(t.net);
  return `${head} · spent ${money(Number(t.spent))} of ${money(Number(t.allowance))} · ${net >= 0 ? `${money(net)} into the jar` : `${money(-net)} out of the jar`}${tail}`;
}

/** Cup art: how full the cup is (0..1) and whether it spilled. */
export function cup(c: Pick<GameCard, "rule" | "spent">): { fill: number; spilled: boolean } {
  if (c.rule.kind !== "cap") return { fill: 0, spilled: false };
  const cap = Number(c.rule.amount) || 1;
  const spent = Number(c.spent);
  return { fill: Math.min(1, spent / cap), spilled: spent > cap };
}

/** Days art: one square per day, done, broken or still to come. */
export function squares(c: Pick<GameCard, "days_done" | "days_total" | "status">): ("done" | "broken" | "todo")[] {
  return Array.from({ length: c.days_total }, (_, i) =>
    i < c.days_done ? "done" : i === c.days_done && c.status === "lost" ? "broken" : "todo",
  );
}

/** Which icon a card shows. */
export function glyph(category: string | null): "cup" | "bowl" | "cart" | "bag" | "ticket" | "bus" | "sun" | "coin" {
  switch (category) {
    case "coffee": return "cup";
    case "dining": return "bowl";
    case "groceries": return "cart";
    case "shopping": return "bag";
    case "entertainment": return "ticket";
    case "transport": return "bus";
    case null: return "sun";
    default: return "coin";
  }
}

export const SUITS = ["♦", "♠", "♣", "♥"];
