import { useEffect, useRef, useState } from "react";
import { startDisplay, DisplayHost, SocketLike } from "../../display/runtimeDisplay";
import * as drawing from "../../drawing/runtimeBundle";
import type { ExplorationModel } from "../explorationModel";

/** The shipped operator UI with snapshot-only transport. It cannot log in or write. */
export function Exploration({
    model,
    initial,
    fontUrl,
}: {
    model: ExplorationModel;
    initial: string;
    fontUrl?: string;
}) {
    const root = useRef<HTMLDivElement>(null);
    const [address, setAddress] = useState(`/hmi/${encodeURIComponent(initial)}`);
    useEffect(() => setAddress(`/hmi/${encodeURIComponent(initial)}`), [initial]);
    useEffect(() => {
        if (!root.current) return;
        const container = document.createElement("div");
        container.style.cssText = "height:100%;width:100%;position:relative";
        root.current.append(container);
        const lifetime = new AbortController(),
            timers = new Set<ReturnType<typeof setTimeout>>(),
            frames = new Set<number>();
        const url = new URL(address, "http://preview.invalid");
        const later = (ms: number, callback: () => void): void => {
            const timer = setTimeout(() => {
                timers.delete(timer);
                if (!lifetime.signal.aborted) callback();
            }, ms);
            timers.add(timer);
        };
        const host: DisplayHost = {
            location: { pathname: url.pathname, search: url.search, host: url.host, protocol: url.protocol },
            now: () => model.nowMs,
            asset: (file) => model.assets[file] ?? "",
            reload: () => setAddress(address),
            navigate: (path) => {
                if (/^\/hmi(?:\/|\?|$)/.test(path)) setAddress(path);
            },
            schedule: (callback) => {
                const id = requestAnimationFrame(() => {
                    frames.delete(id);
                    if (!lifetime.signal.aborted) callback();
                });
                frames.add(id);
            },
            later,
            async fetchJson(path) {
                const request = new URL(path, "http://preview.invalid");
                if (request.pathname === "/api/hmi/page") {
                    const name = request.searchParams.get("name") ?? initial;
                    return {
                        status: model.pages[name] ? 200 : 404,
                        body: model.pages[name] ?? { error: "Page is outside this preview snapshot" },
                    };
                }
                if (request.pathname === "/api/hmi/popup") {
                    const key = request.searchParams.has("faceplate")
                        ? `faceplate:${request.searchParams.get("faceplate")}`
                        : (request.searchParams.get("name") ?? "");
                    return { status: model.popups[key] ? 200 : 404, body: { popup: model.popups[key] ?? null } };
                }
                if (request.pathname === "/api/hmi/session") return { status: 200, body: { user: null } };
                return { status: 200, body: { samples: [], records: [], events: [] } };
            },
            async postJson() {
                return { status: 403, body: { error: "Synthetic preview: commands and login are disabled." } };
            },
            openSocket() {
                const socket: SocketLike = {
                    onmessage: null,
                    onclose: null,
                    close() {
                        socket.onmessage = null;
                        socket.onclose = null;
                    },
                };
                later(0, () => {
                    socket.onmessage?.({ data: JSON.stringify({ cycle: 1, period_ms: 250, values: model.values }) });
                    if (model.alarms?.summary)
                        socket.onmessage?.({ data: JSON.stringify({ alarms: model.alarms.summary }) });
                });
                return socket;
            },
        };
        const follow = (event: MouseEvent) => {
            const link = (event.target as Element)?.closest?.("a[href]");
            const path = link?.getAttribute("href");
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
            timers.forEach(clearTimeout);
            frames.forEach(cancelAnimationFrame);
            container.removeEventListener("click", follow);
            container.remove();
        };
    }, [model, address, initial, fontUrl]);
    return <div className="hb-proposal-exploration" ref={root} aria-label="Explore synthetic HMI" />;
}
