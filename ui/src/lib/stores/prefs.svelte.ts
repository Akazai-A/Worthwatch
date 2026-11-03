// Per-device preferences: hide amounts (⌘⇧H) and theme. Stored in localStorage.
import { hideAmountsWhen } from "../money";

export type Theme = "system" | "dark" | "light";

function read<T>(key: string, fallback: T): T {
  try { const v = localStorage.getItem(key); return v == null ? fallback : (JSON.parse(v) as T); } catch { return fallback; }
}
function write(key: string, v: unknown) {
  try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* private mode */ }
}

let blur = $state<boolean>(read("ww.blur", false));
let theme = $state<Theme>(read("ww.theme", "system"));
hideAmountsWhen(() => blur);

function applyTheme(t: Theme) {
  if (typeof document === "undefined") return;
  if (t === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", t);
}
applyTheme(theme);

export const prefs = {
  get blur() { return blur; },
  get theme() { return theme; },
  toggleBlur() { blur = !blur; write("ww.blur", blur); },
  setTheme(t: Theme) { theme = t; write("ww.theme", t); applyTheme(t); },
  cycleTheme() { prefs.setTheme(theme === "system" ? "dark" : theme === "dark" ? "light" : "system"); },
};
