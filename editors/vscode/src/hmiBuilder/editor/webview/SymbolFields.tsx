// Binding and port definitions beside the symbol's rules (§14.4.7).
import { useState } from "react";
import type { CustomSymbolModel } from "../../drawing/symbols/custom";

export function SymbolFields({
    model,
    disabled,
    change,
}: {
    model: CustomSymbolModel;
    disabled: boolean;
    change: (model: CustomSymbolModel) => void;
}) {
    const [problem, setProblem] = useState("");
    const [editingPort, setEditingPort] = useState<string>();
    const port = model.ports.find((candidate) => candidate.name === editingPort);
    return (
        <>
            <details className="hb-section">
                <summary>Bindings · {Object.keys(model.properties).length}</summary>
                {Object.entries(model.properties).map(([name, kind]) => (
                    <div key={name} className="hb-symbol-entry">
                        <label className="hb-field">
                            <span className="hb-field-label">{name}</span>
                            <select
                                data-binding={name}
                                value={kind}
                                disabled={disabled}
                                onChange={(event) =>
                                    change({
                                        ...model,
                                        properties: { ...model.properties, [name]: event.target.value as typeof kind },
                                    })
                                }
                            >
                                <option value="boolean">Boolean</option>
                                <option value="number">Number</option>
                                <option value="text">Text</option>
                            </select>
                        </label>
                        <button
                            type="button"
                            data-action="remove-binding"
                            data-name={name}
                            disabled={disabled}
                            aria-label={`Remove binding ${name}`}
                            onClick={() => {
                                const properties = { ...model.properties };
                                delete properties[name];
                                change({ ...model, properties });
                            }}
                        >
                            Remove
                        </button>
                    </div>
                ))}
                <form
                    onSubmit={(event) => {
                        event.preventDefault();
                        if (disabled) return;
                        const form = new FormData(event.currentTarget);
                        const name = String(form.get("binding-name") ?? "").trim();
                        const kind = String(form.get("binding-kind")) as "boolean" | "number" | "text";
                        if (!name || Object.keys(model.properties).includes(name)) {
                            setProblem("Choose a new binding name.");
                            return;
                        }
                        setProblem("");
                        change({ ...model, properties: { ...model.properties, [name]: kind } });
                    }}
                >
                    <label className="hb-field">
                        <span className="hb-field-label">Name</span>
                        <input name="binding-name" disabled={disabled} />
                    </label>
                    <label className="hb-field">
                        <span className="hb-field-label">Kind</span>
                        <select name="binding-kind" disabled={disabled} defaultValue="boolean">
                            <option value="boolean">Boolean</option>
                            <option value="number">Number</option>
                            <option value="text">Text</option>
                        </select>
                    </label>
                    <button type="submit" data-action="add-binding" disabled={disabled}>
                        Add binding
                    </button>
                </form>
            </details>
            <details className="hb-section">
                <summary>Ports · {model.ports.length}</summary>
                {model.ports.map((port) => (
                    <div key={port.name} className="hb-symbol-entry">
                        <button
                            type="button"
                            data-action="edit-port"
                            data-name={port.name}
                            disabled={disabled}
                            onClick={() => setEditingPort(port.name)}
                        >
                            {port.name} · {port.at.join(", ")}
                        </button>
                        <button
                            type="button"
                            data-action="remove-port"
                            data-name={port.name}
                            disabled={disabled}
                            aria-label={`Remove port ${port.name}`}
                            onClick={() => {
                                change({
                                    ...model,
                                    ports: model.ports.filter((candidate) => candidate.name !== port.name),
                                });
                                setEditingPort(undefined);
                            }}
                        >
                            Remove
                        </button>
                    </div>
                ))}
                <form
                    key={editingPort ?? "new"}
                    onSubmit={(event) => {
                        event.preventDefault();
                        if (disabled) return;
                        const form = new FormData(event.currentTarget);
                        const name = String(form.get("port-name") ?? "").trim();
                        const at: [number, number] = [Number(form.get("port-x")), Number(form.get("port-y"))];
                        const direction = String(form.get("port-direction")) as "left" | "right" | "up" | "down";
                        const [x, y, width, height] = model.viewBox;
                        if (!name || model.ports.some((port) => port.name === name && port.name !== editingPort)) {
                            setProblem("Choose a new port name.");
                            return;
                        }
                        if (
                            !at.every(Number.isFinite) ||
                            at[0] < x ||
                            at[0] > x + width ||
                            at[1] < y ||
                            at[1] > y + height
                        ) {
                            setProblem("Place the port inside the drawing.");
                            return;
                        }
                        setProblem("");
                        const next = { name, at, direction };
                        change({
                            ...model,
                            ports: port
                                ? model.ports.map((candidate) => (candidate === port ? next : candidate))
                                : [...model.ports, next],
                        });
                        setEditingPort(undefined);
                    }}
                >
                    <label className="hb-field">
                        <span className="hb-field-label">Name</span>
                        <input name="port-name" defaultValue={port?.name ?? ""} disabled={disabled} />
                    </label>
                    <label className="hb-field">
                        <span className="hb-field-label">X</span>
                        <input
                            name="port-x"
                            type="number"
                            step="any"
                            defaultValue={port?.at[0] ?? model.viewBox[0]}
                            disabled={disabled}
                        />
                    </label>
                    <label className="hb-field">
                        <span className="hb-field-label">Y</span>
                        <input
                            name="port-y"
                            type="number"
                            step="any"
                            defaultValue={port?.at[1] ?? model.viewBox[1]}
                            disabled={disabled}
                        />
                    </label>
                    <label className="hb-field">
                        <span className="hb-field-label">Direction</span>
                        <select name="port-direction" disabled={disabled} defaultValue={port?.direction ?? "left"}>
                            {["left", "right", "up", "down"].map((direction) => (
                                <option key={direction}>{direction}</option>
                            ))}
                        </select>
                    </label>
                    <button type="submit" data-action="add-port" disabled={disabled}>
                        {port ? "Update port" : "Add port"}
                    </button>
                </form>
            </details>
            {problem && (
                <p role="alert" className="hb-hint">
                    {problem}
                </p>
            )}
        </>
    );
}
