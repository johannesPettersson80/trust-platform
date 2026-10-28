import { useEffect, useState } from "react";
import { Field } from "./Field";
import { LanguageChoices } from "./LanguageChoices";

import type { VsCodeApi } from "./vscodeApi";
import type { LanguageEntry, ScreenDocumentModel as Model, PageEntry } from "../messages";
import { ColoursSection } from "./ColoursSection";

export function ScreenEditor({ api }: { api: VsCodeApi }) {
    const [pages, setPages] = useState<PageEntry[]>([]);
    const [shipped, setShipped] = useState<LanguageEntry[]>([]);
    const [users, setUsers] = useState<Array<{ name: string; role: string }>>([]);
    const [model, setModel] = useState<Model>();
    const [error, setError] = useState<string>();
    useEffect(() => {
        const receive = (event: MessageEvent): void => {
            if (event.data?.type === "screenDocument") {
                setModel(event.data.model);
                setPages(event.data.pages ?? []);
                setShipped(event.data.shipped ?? []);
                setUsers(event.data.users ?? []);
                setError(undefined);
            }
            if (event.data?.type === "screenError") setError(event.data.message);
        };
        window.addEventListener("message", receive);
        api.postMessage({ type: "ready" });
        return () => window.removeEventListener("message", receive);
    }, [api]);
    const change = (changes: Partial<Model>): void => api.postMessage({ type: "editScreen", changes });
    const dimension = (index: 0 | 1, value: string): void => {
        if (!model) return;
        const pixels = Number(value);
        if (!Number.isInteger(pixels) || pixels < 1 || pixels > 16384) {
            setError("Screen dimensions must be between 1 and 16384 pixels.");
            return;
        }
        const size: [number, number] = [...model.size];
        size[index] = pixels;
        change({ size });
    };
    return (
        <main className="hb-screen-settings" style={{ padding: 24, maxWidth: 640 }}>
            <h1>Screen settings</h1>
            <p>Shared by every page in this HMI.</p>
            <button type="button" className="hb-secondary" onClick={() => api.postMessage({ type: "openAsText" })}>
                Open as text
            </button>
            {error ? <p role="alert">{error}</p> : null}
            {model ? (
                <>
                    <section className="hb-section">
                        <h2>Screen size</h2>
                        <div className="hb-pair">
                            <Field
                                name="screen-width"
                                label="Width (px)"
                                value={model.size[0]}
                                numeric
                                onCommit={(value) => dimension(0, value)}
                            />
                            <Field
                                name="screen-height"
                                label="Height (px)"
                                value={model.size[1]}
                                numeric
                                onCommit={(value) => dimension(1, value)}
                            />
                        </div>
                    </section>
                    {model.top_bar ? (
                        <section className="hb-section">
                            <h2>Top bar</h2>
                            <Field
                                name="screen-title"
                                label="Title"
                                value={model.top_bar.title}
                                onCommit={(title) => change({ top_bar: { ...model.top_bar, title } })}
                            />
                        </section>
                    ) : null}
                    <section className="hb-section">
                        <h2>Appearance</h2>
                        <label className="hb-field">
                            <span>Operator theme</span>
                            <select
                                name="screen-theme"
                                value={model.theme}
                                onChange={(event) => change({ theme: event.target.value as Model["theme"] })}
                            >
                                <option value="dark">Dark</option>
                                <option value="light">Light</option>
                            </select>
                        </label>
                    </section>
                    <section className="hb-section">
                        <h2>Start page</h2>
                        <label className="hb-field">
                            <span>Open this page first</span>
                            <select
                                name="screen-start-page"
                                value={model.start_page ?? ""}
                                onChange={(event) => change({ start_page: event.target.value })}
                            >
                                <option value="">First page in navigation</option>
                                {pages.map((page) => (
                                    <option key={page.name} value={page.name}>
                                        {page.title}
                                    </option>
                                ))}
                            </select>
                        </label>
                    </section>
                    <ColoursSection
                        screen={{ size: model.size, theme: model.theme, colors: model.colors }}
                        warnings={[]}
                        readOnly={false}
                        onColour={(theme, token, value) => {
                            const colors = { ...model.colors, [theme]: { ...model.colors?.[theme] } };
                            if (value) colors[theme]![token] = value;
                            else delete colors[theme]![token];
                            change({ colors });
                        }}
                    />
                    <section className="hb-section">
                        <h2>Languages</h2>
                        <p className="hb-hint">
                            Operators switch between these on the running screen; your texts are written in the default.
                        </p>
                        <LanguageChoices
                            shipped={shipped}
                            chosen={model.languages}
                            onChange={(languages) => change({ languages })}
                        />
                    </section>
                    <section className="hb-section" data-part="users">
                        <h2>Users</h2>
                        <p className="hb-hint">
                            Who may log in to the runtime display. Operators operate equipment; Engineers also change
                            settings.
                        </p>
                        {users.length === 0 ? (
                            <p className="hb-hint">No users yet: nobody can operate the runtime display.</p>
                        ) : (
                            <ul className="hb-users">
                                {users.map((user) => (
                                    <li key={user.name} data-user={user.name}>
                                        <span>{user.name}</span>
                                        <span className="hb-hint">
                                            {user.role.charAt(0).toUpperCase() + user.role.slice(1)}
                                        </span>
                                        <button
                                            type="button"
                                            className="hb-secondary"
                                            data-action="remove-user"
                                            onClick={() => api.postMessage({ type: "removeUser", name: user.name })}
                                        >
                                            Remove
                                        </button>
                                    </li>
                                ))}
                            </ul>
                        )}
                        <button
                            type="button"
                            className="hb-secondary"
                            data-action="add-user"
                            onClick={() => api.postMessage({ type: "addUser" })}
                        >
                            Add user…
                        </button>
                    </section>
                    <details className="hb-section">
                        <summary>Existing navigation</summary>
                        <label className="hb-field">
                            <span>Navigation</span>
                            <select
                                name="screen-navigation"
                                value={model.navigation.style}
                                onChange={(event) =>
                                    change({
                                        navigation: {
                                            ...model.navigation,
                                            style: event.target.value as Model["navigation"]["style"],
                                            position: "left",
                                        },
                                    })
                                }
                            >
                                <option value="none">None</option>
                                <option value="side">Sidebar</option>
                                <option value="tabs">Tabs</option>
                            </select>
                        </label>
                    </details>
                </>
            ) : (
                <p>Reading screen settings…</p>
            )}
        </main>
    );
}
