// The project's own colours (specification 39, §14.1 item 2): each token of the screen's theme in
// groups, with its colour, a reset to the default for the ones the project sets, and the colour
// warnings of the quality checks (item 4) beside their tokens.

import { useEffect, useRef, useState } from "react";

import { TOKEN_THEMES } from "../../drawing/tokens";
import type { ScreenModel } from "../messages";
import type { QualityWarning } from "../quality";

const GROUPS: Array<[string, string[]]> = [
    ["Surfaces", ["page", "surface", "raised", "border", "line"]],
    ["Text", ["text", "muted"]],
    ["Data", ["accent", "accent-2", "accent-3", "pen-1", "pen-2", "pen-3", "pen-4"]],
    ["States", ["unknown", "stale", "pending"]],
    ["Alarms", ["warning", "alarm", "alarm-high", "alarm-medium", "alarm-low"]],
];

/** A colour as the picker takes it: `#rrggbb`, without an alpha. */
function pickerColour(hex: string): string {
    const digits = hex.replace("#", "");
    const full = digits.length === 3 ? [...digits].map((digit) => digit + digit).join("") : digits.slice(0, 6);
    return `#${full.toLowerCase()}`;
}

/** A colour well that reports a colour once it is chosen, not while it is dragged. */
function ColourInput({
    token,
    value,
    readOnly,
    onPick,
}: {
    token: string;
    value: string;
    readOnly: boolean;
    onPick: (value: string) => void;
}) {
    const input = useRef<HTMLInputElement>(null);
    const pick = useRef(onPick);
    pick.current = onPick;
    useEffect(() => {
        const element = input.current;
        if (!element) return undefined;
        const changed = (): void => pick.current(element.value);
        element.addEventListener("change", changed);
        return () => element.removeEventListener("change", changed);
    }, []);
    useEffect(() => {
        if (input.current && input.current.value !== value) input.current.value = value;
    }, [value]);
    return (
        <input
            ref={input}
            type="color"
            name={`colour-${token}`}
            defaultValue={value}
            disabled={readOnly}
            aria-label={token}
        />
    );
}

interface ColoursSectionProps {
    screen: ScreenModel;
    warnings: QualityWarning[];
    readOnly: boolean;
    onColour: (theme: "dark" | "light", token: string, value: string | undefined) => void;
}

export function ColoursSection({ screen, warnings, readOnly, onColour }: ColoursSectionProps) {
    const theme = screen.theme;
    const own = screen.colors?.[theme] ?? {};
    const defaults = TOKEN_THEMES[theme];
    const warned = warnings.filter((warning) => warning.colour?.theme === theme);
    // Open while a colour has a warning; otherwise as the user leaves it.
    const [open, setOpen] = useState(warned.length > 0);
    useEffect(() => {
        if (warned.length > 0) setOpen(true);
    }, [warned.length]);
    return (
        <details
            className="hb-section hb-colours"
            data-part="colours"
            open={open}
            onToggle={(event) => setOpen((event.currentTarget as HTMLDetailsElement).open)}
        >
            <summary>
                <h2>Colours</h2>
                <span className="hb-hint">{theme} theme</span>
            </summary>
            {GROUPS.map(([group, tokens]) => (
                <div key={group} className="hb-colour-group">
                    <h3>{group}</h3>
                    {tokens.map((token) => (
                        <div key={token} className="hb-colour-row">
                            <ColourInput
                                token={token}
                                value={pickerColour(own[token] ?? defaults[token])}
                                readOnly={readOnly}
                                onPick={(value) => onColour(theme, token, value)}
                            />
                            <span className="hb-colour-name">
                                {token}
                                <span className="hb-colour-value">{(own[token] ?? defaults[token]).toLowerCase()}</span>
                            </span>
                            {own[token] ? (
                                <button
                                    type="button"
                                    className="hb-secondary hb-colour-reset"
                                    data-action="reset-colour"
                                    data-token={token}
                                    disabled={readOnly}
                                    onClick={() => onColour(theme, token, undefined)}
                                >
                                    Reset
                                </button>
                            ) : null}
                            {warned
                                .filter((warning) => warning.colour?.token === token)
                                .map((warning, index) => (
                                    <div
                                        key={index}
                                        className="hb-colour-warning"
                                        data-part="colour-warning"
                                        data-token={token}
                                    >
                                        {warning.message}
                                    </div>
                                ))}
                        </div>
                    ))}
                </div>
            ))}
        </details>
    );
}
