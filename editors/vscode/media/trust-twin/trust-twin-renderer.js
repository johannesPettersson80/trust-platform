"use strict";
(function () {
  const script = document.currentScript;
  const wasmUri = script ? script.getAttribute("data-wasm-uri") : "";
  async function initialize() {
    if (!wasmUri || typeof WebAssembly === "undefined") {
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: false, error: "wasm unavailable" },
      }));
      return;
    }
    try {
      const response = await fetch(wasmUri);
      const bytes = await response.arrayBuffer();
      await WebAssembly.instantiate(bytes, {});
      const version = 1;
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: version === 1, renderer: "trust-twin-renderer", contract: version },
      }));
    } catch (error) {
      window.dispatchEvent(new CustomEvent("trustTwinRendererWasmReady", {
        detail: { ok: false, error: String(error && error.message ? error.message : error) },
      }));
    }
  }
  void initialize();
}());
