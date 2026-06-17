import React from "react";
import { createRoot } from "react-dom/client";
import { NetworkCanvasApp } from "./NetworkCanvasApp";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Root element not found");
}

createRoot(container).render(
  <React.StrictMode>
    <NetworkCanvasApp />
  </React.StrictMode>
);
