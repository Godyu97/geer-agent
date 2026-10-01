import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

export default defineConfig(({ mode }) => ({
  plugins: [react()],
  resolve: { alias: { "@host": fileURLToPath(new URL(`./src/hosts/${mode === "web" ? "web" : "desktop"}.ts`, import.meta.url)) } },
  build: { outDir: `dist/${mode === "web" ? "web" : "gui"}`, emptyOutDir: true },
}));
