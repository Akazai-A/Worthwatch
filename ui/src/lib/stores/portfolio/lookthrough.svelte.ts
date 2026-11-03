// The Look-through view (GET /api/lookthrough): loaded when a tab that needs it opens,
// reloaded when the ledger version moves while such a tab is open.
import { getLookthrough, errorMessage, type LookThroughView } from "../../api";

let current = $state<LookThroughView | null>(null);
let error = $state<string | null>(null);
let loading = $state(false);

export const lookthrough = {
  get current() { return current; },
  get error() { return error; },
  get loading() { return loading; },
  set(v: LookThroughView) { current = v; error = null; },
  /** Fetch if missing or older than `version`. */
  async ensure(version: number) {
    if (loading || (current && current.version >= version)) return;
    loading = true;
    try {
      current = await getLookthrough();
      error = null;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  },
};
