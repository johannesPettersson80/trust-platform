// The variables one binding can have, searchable (§7.1): shared by the inspector's bindings and a
// pipe's flow.

import { useState } from "react";

import type { VariableNode } from "../messages";
import { matchesSearch, pickerTree } from "../variables";
import { VariableTree } from "./VariableTree";

interface BindingPickerProps {
    variables: VariableNode[] | undefined;
    problem: string | undefined;
    offered: (node: VariableNode) => boolean;
    onPick: (node: VariableNode) => void;
}

export function BindingPicker({ variables, problem, offered, onPick }: BindingPickerProps) {
    const [search, setSearch] = useState("");
    const rows = pickerTree(
        variables ?? [],
        (node) => offered(node) && (!search.trim() || matchesSearch(node, search)),
    );
    let content;
    if (problem) content = <p className="hb-hint">Variables appear when the program compiles. {problem}</p>;
    else if (variables === undefined) content = <p className="hb-hint">Reading the program's variables…</p>;
    else if (rows.length === 0)
        content = (
            <p className="hb-hint">
                {search.trim() ? "No variable matches." : "No variable of the program fits this binding."}
            </p>
        );
    else content = <VariableTree rows={rows} unfolded pickable={offered} onPick={onPick} />;
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
