import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  resolve: {
    alias: {
      // Monaco's prebuilt sanitizer vendors an older copy; bind its import to the locked patch.
      "./dompurify/dompurify.js": resolve(
        import.meta.dirname,
        "node_modules/dompurify/dist/purify.es.mjs",
      ),
      // Monaco exports JavaScript subpaths; its published CSS needs an explicit asset binding.
      "monaco-editor/editor/editor.main.css?inline": resolve(
        import.meta.dirname,
        "node_modules/monaco-editor/min/vs/editor/editor.main.css",
      ) + "?inline",
    },
  },
  build: {
    outDir: resolve(import.meta.dirname, "../../src/web/ui/assets"),
    emptyOutDir: false,
    sourcemap: false,
    target: "es2020",
    cssCodeSplit: false,
    lib: {
      entry: resolve(import.meta.dirname, "src/monaco-bundle.ts"),
      formats: ["es"],
    },
    rolldownOptions: {
      output: {
        entryFileNames: "ide-monaco.20260215.js",
        assetFileNames: "ide-monaco.20260215.[ext]",
        codeSplitting: false,
      },
    },
  },
});
