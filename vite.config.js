import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import adapter from "@sveltejs/adapter-static";
// @ts-expect-error type error without @types/node package
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    // SvelteKit 3: la configuración (antes en svelte.config.js) se pasa al plugin.
    // Tauri no tiene servidor Node, así que se usa adapter-static en modo SPA.
    // https://v2.tauri.app/start/frontend/sveltekit/
    await sveltekit({
      preprocess: vitePreprocess(),
      adapter: adapter({ fallback: "index.html" }),
    }),
  ],

  // Opciones para `tauri dev` / `tauri build`:
  // 1. no ocultar los errores de Rust
  clearScreen: false,
  // 2. Tauri espera un puerto fijo; fallar si no está libre
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. no vigilar `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
