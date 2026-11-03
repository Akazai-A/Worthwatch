// The chat thread for this tab: what was asked and what came back. Not persisted:
// approved changes are in the ledger, everything else is disposable. Pending MCP
// proposals live on the server and arrive with the view.
import { chat as ask, errorMessage, type ChatReply, type Plan } from "../api";

export type Status = "open" | "approved" | "cancelled" | "failed";
export interface Turn {
  id: number;
  text: string;
  reply: ChatReply | null;
  error: string | null;
  /** The plan as shown, after any edits. */
  plan: Plan | null;
  status: Status;
  outcome: string | null;
}

let turns = $state<Turn[]>([]);
let busy = $state(false);
let seq = 0;

export const chat = {
  get turns() { return turns; },
  get busy() { return busy; },
  async send(text: string, consent = false, replaceId?: number) {
    const t = text.trim();
    if (!t || busy) return;
    busy = true;
    const id = replaceId ?? ++seq;
    if (replaceId == null) turns.push({ id, text: t, reply: null, error: null, plan: null, status: "open", outcome: null });
    try {
      const reply = await ask(t, consent);
      chat.update(id, { reply, plan: reply.plan, error: null });
    } catch (e) {
      chat.update(id, { error: errorMessage(e) });
    } finally {
      busy = false;
    }
  },
  update(id: number, patch: Partial<Turn>) {
    const i = turns.findIndex((x) => x.id === id);
    if (i >= 0) turns[i] = { ...turns[i], ...patch };
  },
  clear() { turns = []; },
};
