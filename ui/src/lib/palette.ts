// The ⌘K palette's matching: letters of the query in order, anywhere in the label,
// scored so word starts and runs beat scattered letters. Pure, so it is tested.

export type Section = "Accounts" | "Go to" | "Actions";
export interface Item {
  id: string;
  section: Section;
  label: string;
  /** Extra words that match but are shown faint (an institution, "full screen"). */
  hint?: string;
  run: () => void;
}
export interface Hit { item: Item; score: number; marks: number[] }

/** Indices in `label` that match `query` in order, and a score; null when it does not match. */
export function match(query: string, label: string): { score: number; marks: number[] } | null {
  const q = query.trim().toLowerCase();
  if (!q) return { score: 0, marks: [] };
  const l = label.toLowerCase();
  const whole = l.indexOf(q);
  if (whole >= 0) {
    const atWord = whole === 0 || /[\s›·\-/]/.test(l[whole - 1]);
    return { score: 100 + (atWord ? 50 : 0) - whole, marks: [...Array(q.length).keys()].map((i) => whole + i) };
  }
  const marks: number[] = [];
  let score = 0;
  let from = 0;
  for (const ch of q) {
    if (ch === " ") continue;
    const i = l.indexOf(ch, from);
    if (i < 0) return null;
    const prev = marks[marks.length - 1];
    if (prev != null && i === prev + 1) score += 5;
    if (i === 0 || /[\s›·\-/]/.test(l[i - 1])) score += 8;
    score -= Math.min(i - from, 10) * 0.5;
    marks.push(i);
    from = i + 1;
  }
  return { score, marks };
}

const ORDER: Section[] = ["Accounts", "Go to", "Actions"];

/** Matching items, best first within each section. With no query the sections keep a
 *  fixed order; with one, the section holding the best hit comes first, so Enter
 *  takes the best match ("wall" opens the wall view, not an account spelled w…a…l…l). */
export function rank(items: Item[], query: string, perSection = 6): Hit[] {
  const hits: Hit[] = [];
  for (const item of items) {
    const m = match(query, item.label) ?? (item.hint ? match(query, item.hint) && { score: -20, marks: [] } : null);
    if (m) hits.push({ item, score: m.score, marks: m.marks });
  }
  const groups = ORDER.map((s) =>
    hits
      .filter((h) => h.item.section === s)
      .sort((a, b) => b.score - a.score)
      .slice(0, query.trim() ? perSection : 99),
  ).filter((g) => g.length);
  if (query.trim()) groups.sort((a, b) => b[0].score - a[0].score);
  return groups.flat();
}

/** Split a label into runs for highlighting. */
export function runs(label: string, marks: number[]): { text: string; on: boolean }[] {
  const set = new Set(marks);
  const out: { text: string; on: boolean }[] = [];
  for (let i = 0; i < label.length; i++) {
    const on = set.has(i);
    const last = out[out.length - 1];
    if (last && last.on === on) last.text += label[i];
    else out.push({ text: label[i], on });
  }
  return out;
}
