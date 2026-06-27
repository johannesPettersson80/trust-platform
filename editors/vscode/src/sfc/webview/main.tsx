import React from "react";
import { createRoot } from "react-dom/client";
import { SfcEditor } from "./SfcEditor";
import "../../webview/theme.css";

/**
 * Entry point for the SFC editor webview
 */
const container = document.getElementById("root");

if (!container) {
  throw new Error("Root element not found");
}

const root = createRoot(container);
root.render(
  <React.StrictMode>
    <SfcEditor />
  </React.StrictMode>
);
