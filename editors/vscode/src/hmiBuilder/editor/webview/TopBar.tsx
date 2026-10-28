// The top bar (§3.2): the page, Design and Live (§11), the language shown and the "longest"
// preview (§14.5 item 7), zoom, Open as text and Deploy (§16.2). It never wraps: the page title
// gives way, cut with an ellipsis and named in full on hover, and in a narrow bar the less frequent
// actions move into a More menu (§21.3 item 5).

import { useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";

import type { LanguageEntry, LiveState } from "../messages";
import { MoreAction, MoreMenu } from "./MoreMenu";
import { LanguagesDialog } from "./LanguagesDialog";

/** Below this width the bar keeps its frequent controls and puts the rest in More (§21.3 item 5). */
export const COMPACT_BELOW = 1100;
/** The room the page title keeps while the bar shows every action; with less, More takes them. */
const TITLE_ROOM = 160;
const LONGEST_TITLE = "Lengthen every text by 40 % to find the objects too small for a translation";
const LANGUAGES_TITLE = "Choose the languages your screens offer";

/**
 * Whether the bar is narrow: under 1100 px, or too narrow for every action beside a readable title.
 * The room the full bar needs is measured while it shows every action, so the bar does not switch
 * back and forth. A bar that is not laid out is not narrow.
 */
function useCompact(bar: RefObject<HTMLElement | null>, content: string): boolean {
    const [compact, setCompact] = useState(false);
    const fullWidth = useRef(0);
    useLayoutEffect(() => {
        const element = bar.current;
        if (!element) return;
        const measure = (): void => {
            const width = element.clientWidth;
            if (width <= 0) return;
            const title = element.querySelector<HTMLElement>(".hb-title");
            // In the full bar the title takes the room the actions leave.
            if (element.dataset.compact !== "true" && title)
                fullWidth.current = width + Math.max(0, Math.min(TITLE_ROOM, title.scrollWidth) - title.clientWidth);
            setCompact(width < Math.max(COMPACT_BELOW, fullWidth.current));
        };
        measure();
        const observer = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : undefined;
        observer?.observe(element);
        window.addEventListener("resize", measure);
        return () => {
            observer?.disconnect();
            window.removeEventListener("resize", measure);
        };
    }, [bar, content, compact]);
    return compact;
}

/** The screen sizes Design mode previews the page on (§4 item 5). */
export const SCREEN_SIZES: Array<{ value: string; name: string; size?: [number, number] }> = [
    { value: "design", name: "Design size" },
    { value: "1920x1080", name: "1920 × 1080", size: [1920, 1080] },
    { value: "1280x800", name: "1280 × 800", size: [1280, 800] },
    { value: "1024x768", name: "1024 × 768", size: [1024, 768] },
    { value: "1180x820", name: "Tablet landscape 1180 × 820", size: [1180, 820] },
    { value: "820x1180", name: "Tablet portrait 820 × 1180", size: [820, 1180] },
];

interface TopBarProps {
    title: string | undefined;
    zoom: number;
    onZoom: (zoom: number) => void;
    onFit: () => void;
    onOpenAsText: () => void;
    mode: "design" | "live";
    operating?: boolean;
    live: { state?: LiveState; target?: string; reason?: string };
    onMode: (mode: "design" | "live") => void;
    /** A deployment is under way (§16.2, item 8). */
    deploying: boolean;
    onDeploy: () => void;
    onCreatePlcCode?: () => void;
    onCreateHmi?: () => void;
    /** The project's languages, once known; the picker shows when there are several. */
    languages?: LanguageEntry[];
    language?: string;
    onLanguage: (language: string) => void;
    /** The languages truST ships, for the Languages dialog, and the choice made in it (§14.5). */
    shipped?: LanguageEntry[];
    onLanguages?: (languages: string[]) => void;
    longest: boolean;
    onLongest: () => void;
    /** A P&ID, which has no Live mode (§18 phase 4 item 1). */
    pidView?: boolean;
    /** The screen size previewed in Design mode, for a page (a popup has its own size). */
    screenSize?: string;
    onScreenSize: (value: string) => void;
    /** The run target's values are being recorded. */
}

const STEPS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];

/** What Live mode says about its run target (§11, item 6). */
function liveStatus(live: TopBarProps["live"], operating = false): string {
    switch (live.state) {
        case "connected":
            return `Live · ${live.target ?? "runtime"}${operating ? "" : " · operate in the runtime display"}`;
        case "none":
            return "Live · no runtime is running: start the simulator or a runtime";
        case "lost":
            return `Live · ${live.target ?? "runtime"}: connection lost${live.reason ? ` (${live.reason})` : ""}`;
        default:
            return "Live · connecting…";
    }
}

export function TopBar({
    title,
    zoom,
    onZoom,
    onFit,
    onOpenAsText,
    mode,
    operating,
    live,
    onMode,
    deploying,
    onDeploy,
    onCreatePlcCode,
    onCreateHmi,
    languages,
    language,
    onLanguage,
    shipped,
    onLanguages,
    longest,
    onLongest,
    pidView,
    screenSize,
    onScreenSize,
}: TopBarProps) {
    const [choosingLanguages, setChoosingLanguages] = useState(false);
    const liveBar =
        mode === "live" ? (
            <div className="hb-livebar" data-part="live-bar">
                <span
                    className="hb-live-status"
                    data-part="live-status"
                    data-state={live.state ?? "connecting"}
                    role="status"
                >
                    {liveStatus(live, operating)}
                </span>
            </div>
        ) : null;
    const smaller = [...STEPS].reverse().find((step) => step < zoom - 0.001) ?? STEPS[0];
    const larger = STEPS.find((step) => step > zoom + 0.001) ?? STEPS[STEPS.length - 1];
    const bar = useRef<HTMLElement>(null);
    const name = title ?? "HMI page";
    const creates = mode === "design" && !pidView;
    const longestShown = languages !== undefined && !operating;
    const compact = useCompact(
        bar,
        [name, mode, operating, pidView, screenSize !== undefined, languages?.length ?? 0, deploying].join("|"),
    );
    // In a narrow bar: the program and page generators, the "longest" preview and Open as text.
    const more: MoreAction[] = [
        ...(creates
            ? [
                  { action: "create-plc-code", label: "Create PLC code", run: () => onCreatePlcCode?.() },
                  { action: "create-hmi", label: "Create HMI", run: () => onCreateHmi?.() },
              ]
            : []),
        ...(longestShown && onLanguages
            ? [
                  {
                      action: "languages",
                      label: "Languages…",
                      title: LANGUAGES_TITLE,
                      run: () => setChoosingLanguages(true),
                  },
              ]
            : []),
        ...(longestShown
            ? [{ action: "longest", label: "Longest", title: LONGEST_TITLE, checked: longest, run: onLongest }]
            : []),
        { action: "open-as-text", label: "Open as text", run: onOpenAsText },
    ];
    return (
        <>
            <header ref={bar} className="hb-topbar" data-compact={compact}>
                <span className="hb-title" title={name}>
                    {name}
                </span>
                <div className="hb-mode" role="group" aria-label="Mode">
                    <button
                        type="button"
                        data-action="design"
                        aria-pressed={mode === "design"}
                        onClick={() => onMode("design")}
                    >
                        Build
                    </button>
                    {pidView ? null : (
                        <button
                            type="button"
                            data-action="live"
                            aria-pressed={mode === "live"}
                            onClick={() => onMode("live")}
                        >
                            Live
                        </button>
                    )}
                </div>
                {mode === "design" && screenSize !== undefined ? (
                    <select
                        data-action="preview-size"
                        aria-label="Screen size"
                        title="Preview the page on a smaller screen, as the runtime lays it out there"
                        value={screenSize}
                        onChange={(event) => onScreenSize(event.target.value)}
                    >
                        {SCREEN_SIZES.map((entry) => (
                            <option key={entry.value} value={entry.value}>
                                {entry.name}
                            </option>
                        ))}
                    </select>
                ) : null}
                {languages && !operating && (languages.length > 1 || !compact) ? (
                    <div className="hb-languages" role="group" aria-label="Languages" data-part="languages">
                        {languages.length > 1 ? (
                            <select
                                data-part="language-picker"
                                aria-label="Language shown"
                                value={language}
                                onChange={(event) => onLanguage(event.target.value)}
                            >
                                {languages.map((entry) => (
                                    <option key={entry.tag} value={entry.tag}>
                                        {entry.name}
                                    </option>
                                ))}
                            </select>
                        ) : null}
                        {compact || !onLanguages ? null : (
                            <button
                                type="button"
                                data-action="languages"
                                title={LANGUAGES_TITLE}
                                onClick={() => setChoosingLanguages(true)}
                            >
                                Languages…
                            </button>
                        )}
                        {compact ? null : (
                            <button
                                type="button"
                                data-action="longest"
                                aria-pressed={longest}
                                title={LONGEST_TITLE}
                                onClick={onLongest}
                            >
                                Longest
                            </button>
                        )}
                    </div>
                ) : null}
                {!operating ? (
                    <div className="hb-zoom" role="group" aria-label="Zoom">
                        <button type="button" title="Zoom out" onClick={() => onZoom(smaller)}>
                            −
                        </button>
                        <span className="hb-zoom-value">{Math.round(zoom * 100)} %</span>
                        <button type="button" title="Zoom in" onClick={() => onZoom(larger)}>
                            +
                        </button>
                        <button type="button" title="Fit the page in the window" onClick={onFit}>
                            Fit
                        </button>
                    </div>
                ) : null}
                {creates && !compact ? (
                    <>
                        <button type="button" data-action="create-plc-code" onClick={onCreatePlcCode}>
                            Create PLC code
                        </button>
                        <button type="button" data-action="create-hmi" onClick={onCreateHmi}>
                            Create HMI
                        </button>
                    </>
                ) : null}
                {compact ? (
                    <MoreMenu actions={more} />
                ) : (
                    <button type="button" className="hb-secondary" data-action="open-as-text" onClick={onOpenAsText}>
                        Open as text
                    </button>
                )}
                <button
                    type="button"
                    className="hb-primary"
                    data-action="deploy"
                    disabled={deploying}
                    title="Check the HMI and send it to the selected runtime"
                    onClick={onDeploy}
                >
                    {deploying ? "Deploying…" : "Deploy"}
                </button>
            </header>
            {liveBar}
            {choosingLanguages && onLanguages ? (
                <LanguagesDialog
                    shipped={shipped ?? []}
                    languages={(languages ?? []).map((entry) => entry.tag)}
                    onApply={(next) => {
                        setChoosingLanguages(false);
                        onLanguages(next);
                    }}
                    onCancel={() => setChoosingLanguages(false)}
                />
            ) : null}
        </>
    );
}
