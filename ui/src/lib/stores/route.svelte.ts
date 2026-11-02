// Hash router: #/ (home), #/check-in, #/portfolio, #/accounts/<id>
export type Route = { name: "home" } | { name: "checkin" } | { name: "portfolio" } | { name: "account"; id: string };

function parse(hash: string): Route {
  const p = hash.replace(/^#\/?/, "").split("/").filter(Boolean);
  if (p[0] === "check-in") return { name: "checkin" };
  if (p[0] === "portfolio") return { name: "portfolio" };
  if (p[0] === "accounts" && p[1]) return { name: "account", id: decodeURIComponent(p[1]) };
  return { name: "home" };
}

let current = $state<Route>(parse(typeof location === "undefined" ? "" : location.hash));

if (typeof window !== "undefined") {
  window.addEventListener("hashchange", () => { current = parse(location.hash); });
}

export const route = {
  get current() { return current; },
  go(to: Route) {
    location.hash =
      to.name === "home" ? "#/" : to.name === "checkin" ? "#/check-in" : to.name === "portfolio" ? "#/portfolio" : `#/accounts/${encodeURIComponent(to.id)}`;
  },
};
