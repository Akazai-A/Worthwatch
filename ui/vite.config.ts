import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  server: {
    // In dev, the Rust server runs on 7777 and Vite proxies /api to it.
    proxy: { "/api": "http://127.0.0.1:7777" },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
