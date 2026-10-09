// A side panel's title with the button that collapses the panel to a strip (§3.2 item 7), and the
// strip that opens it again.

interface PanelTitleProps {
    title: string;
    /** `components` or `inspector`, for the button's action name. */
    panel: string;
    onCollapse: () => void;
}

export function PanelTitle({ title, panel, onCollapse }: PanelTitleProps) {
    return (
        <div className="hb-panel-title">
            <span>{title}</span>
            <button
                type="button"
                className="hb-collapse"
                data-action={`collapse-${panel}`}
                title={`Hide ${title}`}
                aria-label={`Hide ${title}`}
                onClick={onCollapse}
            >
                {panel === "components" ? "‹" : "›"}
            </button>
        </div>
    );
}

interface PanelStripProps {
    title: string;
    panel: string;
    onOpen: () => void;
}

export function PanelStrip({ title, panel, onOpen }: PanelStripProps) {
    return (
        <aside className="hb-strip" data-part={`${panel}-strip`}>
            <button
                type="button"
                className="hb-strip-open"
                data-action={`open-${panel}`}
                title={`Show ${title}`}
                aria-label={`Show ${title}`}
                onClick={onOpen}
            >
                {panel === "components" ? "›" : "‹"}
            </button>
            <button type="button" className="hb-strip-label" tabIndex={-1} onClick={onOpen}>
                {title}
            </button>
        </aside>
    );
}
