import { ScreenEditor } from "./ScreenEditor";
import { ProposalReview } from "./ProposalReview";
// The HMI Builder webview (specification 39, §3.2, §3.3).

import "./hmiBuilder.css";

import { createRoot } from "react-dom/client";

import { App } from "./App";
import { SymbolEditor } from "./SymbolEditor";
import { vscodeApi } from "./vscodeApi";

const root = document.getElementById("root");
if (root) {
    const api = vscodeApi();
    createRoot(root).render(
        root.dataset.view === "screen" ? (
            <ScreenEditor api={api} />
        ) : root.dataset.view === "proposal" ? (
            <ProposalReview api={api} fontUrl={root.dataset.fontUrl} />
        ) : root.dataset.view === "symbol" ? (
            <SymbolEditor api={api} fontUrl={root.dataset.fontUrl} />
        ) : (
            <App api={api} fontUrl={root.dataset.fontUrl} assetBase={root.dataset.assetBase} />
        ),
    );
}
