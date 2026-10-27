// A selected pipe in the inspector (§6): the ports it joins, its line style, the flow it shows
// and its bends.

import { useState } from "react";

import type { HmiPageModel, HmiPipeModel } from "../../drawing/model";
import type { VariableNode } from "../messages";
import { BindingPicker } from "./BindingPicker";
import { Field } from "./Field";

interface PipeSectionProps {
    page: HmiPageModel;
    index: number;
    paused: boolean;
    variables: VariableNode[] | undefined;
    variablesProblem: string | undefined;
    onCommit: (page: HmiPageModel) => void;
}

export function PipeSection({ page, index, paused, variables, variablesProblem, onCommit }: PipeSectionProps) {
    const [picking, setPicking] = useState(false);
    const pipe = page.pipe[index];
    const replace = (next: HmiPipeModel): void =>
        onCommit({ ...page, pipe: page.pipe.map((candidate, at) => (at === index ? next : candidate)) });
    /** An end as the operator reads it: the object's tag, or its id, and the port. */
    const end = (text: string): string => {
        const id = text.slice(0, text.lastIndexOf("."));
        const object = page.object.find((candidate) => candidate.id === id);
        return `${object?.tag ?? id} ${text.slice(text.lastIndexOf(".") + 1)}`;
    };
    const bindFlow = (path: string): void => {
        const { bind: _old, ...rest } = pipe;
        replace(path ? { ...rest, bind: { flow: { path, direction: "read" } } } : rest);
    };
    return (
        <section className="hb-section" data-part="pipe-section">
            <p className="hb-pipe-ends" data-part="pipe-ends">
                {end(pipe.from)} → {end(pipe.to)}
            </p>
            <label className="hb-field">
                <span className="hb-field-label">Line</span>
                <select
                    name="pipe-style"
                    value={pipe.style ?? "process"}
                    disabled={paused}
                    onChange={(event) => {
                        const { style: _old, ...rest } = pipe;
                        replace(event.target.value === "signal" ? { ...rest, style: "signal" } : rest);
                    }}
                >
                    <option value="process">Process line</option>
                    <option value="signal">Signal line</option>
                </select>
            </label>
            <div className="hb-binding" data-binding="flow">
                <Field
                    name="bind.flow"
                    label="Flow"
                    value={pipe.bind?.flow?.path}
                    readOnly={paused}
                    onCommit={bindFlow}
                />
                <button
                    type="button"
                    className="hb-pick"
                    data-action="pick"
                    data-binding="flow"
                    title="Pick a variable"
                    aria-label="Pick a variable for Flow"
                    aria-expanded={picking}
                    disabled={paused}
                    onClick={() => setPicking(!picking)}
                >
                    ⋯
                </button>
                {picking ? (
                    <BindingPicker
                        variables={variables}
                        problem={variablesProblem}
                        binding={{ type: "pipe", property: "flow" }}
                        onPick={(node) => {
                            setPicking(false);
                            bindFlow(node.path);
                        }}
                    />
                ) : null}
            </div>
            <p className="hb-hint">
                The pipe shows flow while this value is TRUE or not zero, backwards while it is negative.
            </p>
            {(pipe.waypoints ?? []).length > 0 ? (
                <button
                    type="button"
                    data-action="straighten"
                    disabled={paused}
                    onClick={() => {
                        const { waypoints: _old, ...rest } = pipe;
                        replace(rest);
                    }}
                >
                    Straighten
                </button>
            ) : null}
        </section>
    );
}
