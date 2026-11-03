// The one global: the current LedgerView, kept fresh by the server's event stream.
import { getView, subscribe, errorMessage, type LedgerView } from "../api";

let current = $state<LedgerView | null>(null);
let error = $state<string | null>(null);
let loading = $state(false);
let stop: (() => void) | null = null;

export const view = {
  get current() { return current; },
  get error() { return error; },
  get loading() { return loading; },
  set(v: LedgerView) { current = v; error = null; },
  async load() {
    loading = true;
    try {
      current = await getView();
      error = null;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  },
  /** Start listening for changes (hand edits, saves). Safe to call once. */
  connect() {
    if (stop) return;
    stop = subscribe((version) => {
      if (!current || version !== current.version) void view.load();
    });
  },
  disconnect() { stop?.(); stop = null; },
};
