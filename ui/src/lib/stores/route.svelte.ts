// Hash router: #/ (home), #/check-in, #/portfolio[/look-through|/exposure|/overlap], #/changes, #/chat, #/plan, #/wall, #/backups, #/accounts/<id>
export type PortfolioTab = "positions" | "lookthrough" | "exposure" | "overlap";
export type Route =
  | { name: "home" }
  | { name: "checkin" }
  | { name: "portfolio"; tab: PortfolioTab }
  | { name: "changes" }
  | { name: "chat" }
  | { name: "plan" }
  | { name: "wall" }
  | { name: "backups" }
  | { name: "account"; id: string };

const TAB_PATH: Record<PortfolioTab, string> = { positions: "", lookthrough: "/look-through", exposure: "/exposure", overlap: "/overlap" };

function parse(hash: string): Route {
  const p = hash.replace(/^#\/?/, "").split("/").filter(Boolean);
  if (p[0] === "check-in") return { name: "checkin" };
  if (p[0] === "portfolio") {
    const tab = (Object.keys(TAB_PATH) as PortfolioTab[]).find((t) => TAB_PATH[t] === (p[1] ? `/${p[1]}` : "")) ?? "positions";
    return { name: "portfolio", tab };
  }
  if (p[0] === "changes") return { name: "changes" };
  if (p[0] === "chat") return { name: "chat" };
  if (p[0] === "plan") return { name: "plan" };
  if (p[0] === "wall") return { name: "wall" };
  if (p[0] === "backups") return { name: "backups" };
  if (p[0] === "accounts" && p[1]) return { name: "account", id: decodeURIComponent(p[1]) };
  return { name: "home" };
}

export function hashFor(to: Route): string {
  switch (to.name) {
    case "home": return "#/";
    case "checkin": return "#/check-in";
    case "portfolio": return `#/portfolio${TAB_PATH[to.tab]}`;
    case "changes": return "#/changes";
    case "chat": return "#/chat";
    case "plan": return "#/plan";
    case "wall": return "#/wall";
    case "backups": return "#/backups";
    case "account": return `#/accounts/${encodeURIComponent(to.id)}`;
  }
}

let current = $state<Route>(parse(typeof location === "undefined" ? "" : location.hash));

if (typeof window !== "undefined") {
  window.addEventListener("hashchange", () => { current = parse(location.hash); });
}

export const route = {
  get current() { return current; },
  go(to: Route) { location.hash = hashFor(to); },
};
