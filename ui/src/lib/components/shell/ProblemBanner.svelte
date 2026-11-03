<script lang="ts">
  import type { Diagnostic } from "../../api";
  let { problem, path }: { problem: Diagnostic; path: string } = $props();
  const file = $derived(path.split("/").pop() ?? path);
</script>

<div class="banner" role="alert">
  <span class="pill">{problem.line == null ? "file error" : `line ${problem.line}`}</span>
  <span class="text">
    <code>{file}</code> has a problem: <code>{problem.message}</code>.
    Showing the last good version. Fix the file and save; it reloads on its own. Nothing is written until then.
  </span>
</div>

<style>
  .banner { display: flex; align-items: flex-start; gap: 12px; padding: 10px 14px; border-radius: 8px; background: var(--surface); border: 1px solid var(--negative); font-size: 13px; }
  .pill { flex: 0 0 auto; font-size: 11px; padding: 2px 7px; border-radius: 10px; background: var(--negative); color: var(--bg); font-weight: 600; white-space: nowrap; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
</style>
