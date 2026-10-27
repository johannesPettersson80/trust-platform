// The variables one binding can have, searchable (§7.1): shared by the inspector's bindings, a
// pipe's flow and a selection's visibility. The language server decides what the binding is
// offered (the binding checks' rules, §7.6); the search narrows what is shown.

import { useState } from "react";

import type { VariableNode } from "../messages";
import { offeredRows, searched, type PickerBinding } from "../variables";
import { useRust, useRustAnswer } from "./rust";
import { VariableTree } from "./VariableTree";

interface BindingPickerProps {
    variables: VariableNode[] | undefined;
    problem: string | undefined;
    /** The binding the picker offers variables for. */
    binding: PickerBinding;
    onPick: (node: VariableNode) => void;
}

export function BindingPicker({ variables, problem, binding, onPick }: BindingPickerProps) {
    const [search, setSearch] = useState("");
    const { call } = useRust();
    const { type, property, component, direction, fields } = binding;
    // A panel's fields are offered while the program's variables are still being read (§8.7).
    const reading = variables === undefined && fields === undefined;
    const offered = useRustAnswer(
        call,
        [variables, type, property, component, direction, JSON.stringify(fields)],
        (ask) => (reading ? undefined : offeredRows(ask, variables ?? [], binding)),
        true,
    );
    const rows = offered ? searched(offered, search) : undefined;
    let content;
    if (problem) content = <p className="hb-hint">Variables appear when the program compiles. {problem}</p>;
    else if (reading || rows === undefined) content = <p className="hb-hint">Reading the program's variables…</p>;
    else if (rows.length === 0)
        content = (
            <p className="hb-hint">
                {search.trim() ? "No variable matches." : "No variable of the program fits this binding."}
            </p>
        );
    else content = <VariableTree rows={rows} unfolded onPick={onPick} />;
    return (
        <div className="hb-picker" data-part="picker">
            <input
                className="hb-search"
                name="picker-search"
                placeholder="Search variables"
                value={search}
                autoFocus
                onChange={(event) => setSearch(event.target.value)}
            />
            {content}
        </div>
    );
}
