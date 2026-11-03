// The add-a-purchase form, shared by Budget and Play: whether it is open, what is
// typed so far, and the last result. The open page refreshes from the server's
// version bump once an entry is saved.
import { addSpends, ApiError } from "../api";
import { todayIso } from "../money";
import { blankDraft, toSpend, type Draft } from "../spend";

let open = $state(false);
let draft = $state<Draft>(blankDraft(todayIso()));
let busy = $state(false);
let error = $state<string | null>(null);
let last = $state<{ id: string; merchant: string; amount: string } | null>(null);

export const addSpend = {
  get open() { return open; },
  get draft() { return draft; },
  get busy() { return busy; },
  get error() { return error; },
  /** The entry just saved, so the page can mark it and offer an undo. */
  get last() { return last; },
  show(preset: Partial<Draft> = {}) {
    draft = { ...blankDraft(todayIso()), ...preset };
    error = null;
    open = true;
  },
  close() { open = false; error = null; },
  toggle(preset: Partial<Draft> = {}) { if (open) this.close(); else this.show(preset); },
  /** Check and save. Keeps the form open with a message when something is wrong. */
  async submit(): Promise<boolean> {
    const s = toSpend(draft, todayIso());
    if (typeof s === "string") { error = s; return false; }
    busy = true;
    error = null;
    try {
      const { added } = await addSpends([s]);
      last = { id: added[0], merchant: s.merchant, amount: s.amount };
      open = false;
      return true;
    } catch (e) {
      error = e instanceof ApiError ? e.message : String(e);
      return false;
    } finally {
      busy = false;
    }
  },
  forget() { last = null; },
};
