// The "Ask or update" drawer (chat) beside any screen: ⌘J, the rail button or #/chat open it.
let open = $state(false);

export const drawer = {
  get open() { return open; },
  show() { open = true; },
  hide() { open = false; },
  toggle() { open = !open; },
};
