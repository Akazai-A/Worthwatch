// The Changes view (GET /api/changes): one period at a time, reloaded when the
// period, the threshold or the ledger version moves while the screen is open.
import { getChanges, errorMessage, type ChangesView } from "../../api";
import { type Preset } from "../../portfolio/changes";

let current = $state<ChangesView | null>(null);
let error = $state<string | null>(null);
let loading = $state(false);
let preset = $state<Preset>("checkin");
let custom = $state<{ from: string; to: string } | null>(null);
let threshold = $state<string>("0.5");
let key = "";

export const changes = {
  get current() { return current; },
  get error() { return error; },
  get loading() { return loading; },
  get preset() { return preset; },
  get custom() { return custom; },
  get threshold() { return threshold; },
  setPreset(p: Preset) { preset = p; },
  setCustom(from: string, to: string) { custom = { from, to }; preset = "custom"; },
  setThreshold(t: string) { threshold = t; },
  /** Fetch when the inputs differ from the last fetch. `from`/`to` undefined = server defaults. */
  async ensure(version: number, from: string | undefined, to: string | undefined) {
    const k = `${version}|${from ?? ""}|${to ?? ""}|${threshold}`;
    if (loading || k === key) return;
    key = k;
    loading = true;
    try {
      current = await getChanges({ from, to, threshold });
      error = null;
    } catch (e) {
      error = errorMessage(e);
      key = "";
    } finally {
      loading = false;
    }
  },
};
