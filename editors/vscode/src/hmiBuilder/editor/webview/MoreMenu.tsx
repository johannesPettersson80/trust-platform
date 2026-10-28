// The top bar's More menu (specification 39, §21.3 item 5): the less frequent actions, when the bar
// is too narrow to show them. A menu button: a click, Enter, Space or the down arrow opens it; the
// arrows, Home and End move through its items; Escape closes it and gives the focus back to the
// button; Tab or a press elsewhere closes it too.

import { useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";

/** One action of the menu. */
export interface MoreAction {
    /** The `data-action` of the action's own button in a wide bar, kept on its item. */
    action: string;
    label: string;
    title?: string;
    /** A toggle shows whether it is on; a plain action has none. */
    checked?: boolean;
    run: () => void;
}

export function MoreMenu({ actions }: { actions: MoreAction[] }) {
    const [open, setOpen] = useState(false);
    const button = useRef<HTMLButtonElement>(null);
    const menu = useRef<HTMLDivElement>(null);
    const id = useId();
    const items = (): HTMLButtonElement[] =>
        Array.from(
            menu.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"], [role="menuitemcheckbox"]') ?? [],
        );
    const close = (refocus: boolean): void => {
        setOpen(false);
        if (refocus) button.current?.focus();
    };

    // The first item takes the focus as the menu opens.
    useEffect(() => {
        if (open) items()[0]?.focus();
    }, [open]);

    // A press outside the menu and its button closes it, and so does Escape wherever the focus is.
    useEffect(() => {
        if (!open) return;
        const onPress = (event: MouseEvent): void => {
            const target = event.target as Node | null;
            if (target && (menu.current?.contains(target) || button.current?.contains(target))) return;
            setOpen(false);
        };
        const onKey = (event: KeyboardEvent): void => {
            if (event.key !== "Escape") return;
            const focused = document.activeElement;
            setOpen(false);
            if (focused === button.current || (focused && menu.current?.contains(focused))) button.current?.focus();
        };
        window.addEventListener("mousedown", onPress, true);
        window.addEventListener("keydown", onKey);
        return () => {
            window.removeEventListener("mousedown", onPress, true);
            window.removeEventListener("keydown", onKey);
        };
    }, [open]);

    const onMenuKey = (event: ReactKeyboardEvent<HTMLDivElement>): void => {
        const list = items();
        const at = list.indexOf(document.activeElement as HTMLButtonElement);
        const focus = (index: number): void => list[(index + list.length) % list.length]?.focus();
        if (event.key === "Escape") {
            // The menu's own key: it goes no further, to the canvas or the editor.
            event.preventDefault();
            event.stopPropagation();
            close(true);
        } else if (event.key === "ArrowDown") {
            event.preventDefault();
            focus(at + 1);
        } else if (event.key === "ArrowUp") {
            event.preventDefault();
            focus(at < 0 ? list.length - 1 : at - 1);
        } else if (event.key === "Home") {
            event.preventDefault();
            focus(0);
        } else if (event.key === "End") {
            event.preventDefault();
            focus(list.length - 1);
        } else if (event.key === "Tab") {
            setOpen(false);
        }
    };

    return (
        <div className="hb-more" data-part="more">
            <button
                ref={button}
                type="button"
                data-action="more"
                aria-haspopup="menu"
                aria-expanded={open}
                aria-controls={open ? id : undefined}
                title="More actions"
                onClick={() => setOpen((shown) => !shown)}
                onKeyDown={(event) => {
                    if (event.key === "ArrowDown" && !open) {
                        event.preventDefault();
                        setOpen(true);
                    }
                }}
            >
                More
                <span className="hb-more-chevron" aria-hidden="true">
                    ▾
                </span>
            </button>
            {open ? (
                <div
                    ref={menu}
                    id={id}
                    role="menu"
                    aria-label="More actions"
                    data-part="more-menu"
                    className="hb-context-menu hb-more-menu"
                    onKeyDown={onMenuKey}
                >
                    {actions.map((entry) => (
                        <button
                            key={entry.action}
                            type="button"
                            role={entry.checked === undefined ? "menuitem" : "menuitemcheckbox"}
                            aria-checked={entry.checked}
                            data-action={entry.action}
                            title={entry.title}
                            tabIndex={-1}
                            onClick={() => {
                                close(true);
                                entry.run();
                            }}
                        >
                            <span>{entry.label}</span>
                            {entry.checked ? (
                                <span className="hb-more-check" aria-hidden="true">
                                    ✓
                                </span>
                            ) : null}
                        </button>
                    ))}
                </div>
            ) : null}
        </div>
    );
}
