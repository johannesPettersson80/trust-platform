import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  build: {
    outDir: resolve(import.meta.dirname, "../../src/web/ui/assets"),
    emptyOutDir: false,
    sourcemap: false,
    target: "es2020",
    lib: {
      entry: resolve(import.meta.dirname, "src/codemirror-bundle.ts"),
      formats: ["es"],
    },
    rolldownOptions: {
      output: {
        entryFileNames: "ide-codemirror.20260215.js",
        codeSplitting: false,
      },
    },
  },
});
