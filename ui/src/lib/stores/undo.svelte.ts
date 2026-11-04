// One Undo at a time, for removals that save at once (rules, bills, card links).
// `offer` keeps the mutations that put things back; the bar under the screen applies them.
import { apply, errorMessage, type Mutation } from "../api";
import { view } from "./view.svelte";

let current = $state<{ label: string; restore: Mutation[] } | null>(null);
let error = $state<string | null>(null);
let timer: ReturnType<typeof setTimeout> | null = null;

export const undo = {
  get current() { return current; },
  get error() { return error; },
  offer(label: string, restore: Mutation[]) {
    current = { label, restore };
    error = null;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => (current = null), 8000);
  },
  dismiss() { current = null; },
  async run() {
    if (!current) return;
    const { restore } = current;
    current = null;
    try { view.set(await apply(restore)); } catch (e) { error = errorMessage(e); }
  },
};
