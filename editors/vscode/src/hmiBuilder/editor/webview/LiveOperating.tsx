// Live operation shares the runtime's drawing, session, confirmation and outcome UI (§11).
import { useEffect, useRef, useState } from "react";
import { startDisplay } from "../../display/runtimeDisplay";
import * as drawing from "../../drawing/runtimeBundle";
import type { HostToWebview } from "../messages";
import type { VsCodeApi } from "./vscodeApi";
import { LiveDisplayHost } from "./liveDisplayHost";
import "./liveOperating.css";

let next = 0;
export function LiveOperating({
    api,
    page,
    fontUrl,
    assetBase,
    onActive,
}: {
    api: VsCodeApi;
    page: string;
    fontUrl?: string;
    assetBase?: string;
    onActive: (active: boolean) => void;
}) {
    const root = useRef<HTMLDivElement>(null);
    const [host, setHost] = useState<LiveDisplayHost>();
    const [reason, setReason] = useState<string>();
    const [location, setLocation] = useState({ address: `/hmi/${encodeURIComponent(page)}`, revision: 0 });
    // The named connection lasts across page navigation, but never across leaving Live mode.
    useEffect(() => {
        const id = `live-${++next}`;
        const connection = new LiveDisplayHost(
            api,
            id,
            page,
            (address) => setLocation((current) => ({ address, revision: current.revision + 1 })),
            assetBase,
        );
        const receive = (event: MessageEvent<HostToWebview>): void => {
            const message = event.data;
            if (!("id" in message) || message.id !== id) return;
            if (message.type === "operateReady") {
                if (message.reason) {
                    setReason(message.reason);
                    return;
                }
                setHost(connection);
                setReason(undefined);
                onActive(true);
            } else if (message.type === "operateEnded") {
                connection.dispose();
                setReason(message.reason);
                setHost(undefined);
                onActive(false);
            } else connection.receive(message);
        };
        window.addEventListener("message", receive);
        api.postMessage({ type: "operateOpen", id, page });
        return () => {
            connection.dispose();
            window.removeEventListener("message", receive);
            api.postMessage({ type: "operateClose", id });
            setHost(undefined);
            onActive(false);
        };
    }, [api, page, assetBase, onActive]);
    // A fresh drawing gets a disposable container. Late replies can only touch its detached DOM.
    useEffect(() => {
        if (!host || !root.current) return;
        const lifetime = new AbortController();
        const container = document.createElement("div");
        container.className = "hb-live-page";
        root.current.append(container);
        const url = new URL(location.address, "http://live.invalid");
        host.location.pathname = url.pathname;
        host.location.search = url.search;
        const follow = (event: MouseEvent): void => {
            const anchor = (event.target as Element | null)?.closest?.("a[href]");
            const path = anchor?.getAttribute("href");
            if (path?.startsWith("/hmi")) {
                event.preventDefault();
                host.navigate(path);
            }
        };
        container.addEventListener("click", follow);
        void startDisplay(
            container,
            host,
            { ...drawing, tokensCss: (colors) => drawing.tokensCss(colors, fontUrl) },
            lifetime.signal,
        );
        return () => {
            lifetime.abort();
            container.removeEventListener("click", follow);
            container.remove();
        };
    }, [host, location, fontUrl]);
    return (
        <div className="hb-operating" data-active={host !== undefined}>
            {reason ? (
                <div className="hb-live-reason" role="status">
                    Read-only · {reason}
                </div>
            ) : null}
            <div ref={root} className="hb-live-display" data-part={host ? "live-operating" : undefined} />
        </div>
    );
}
