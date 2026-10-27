// The visibility binding of a selection, a group's among them (specification 39, §18 phase 5
// item 2): every selected object is shown only while the BOOL named here is TRUE; an empty field
// removes it. A group is its members, so it is set on each.

import { useState } from "react";

import type { HmiObjectModel, HmiPageModel } from "../../drawing/model";
import type { Catalogue } from "../catalogue";
import type { VariableNode } from "../messages";
import { BindingPicker } from "./BindingPicker";
import { Field } from "./Field";

interface VisibilitySectionProps {
    page: HmiPageModel;
    selection: string[];
    catalogue: Catalogue | undefined;
    variables: VariableNode[] | undefined;
    problem: string | undefined;
    readOnly: boolean;
    onCommit: (page: HmiPageModel) => void;
}

/** An object shown while `path` is TRUE, or always when it is empty. */
export function withVisibility(object: HmiObjectModel, path: string): HmiObjectModel {
    const bind = { ...object.bind };
    if (path) bind.visible = { path, direction: "read" };
    else delete bind.visible;
    return { ...object, bind };
}

export function VisibilitySection({
    page,
    selection,
    catalogue,
    variables,
    problem,
    readOnly,
    onCommit,
}: VisibilitySectionProps) {
    const [picking, setPicking] = useState(false);
    // Every component takes the same visibility binding; the first selected object's names it.
    const first = page.object.find((object) => selection.includes(object.id));
    const component = catalogue?.components.find((candidate) => candidate.type === first?.type);
    const paths = new Set(
        page.object.filter((object) => selection.includes(object.id)).map((object) => object.bind.visible?.path ?? ""),
    );
    const shared = paths.size === 1 ? [...paths][0] : "";
    const set = (path: string): void =>
        onCommit({
            ...page,
            object: page.object.map((object) =>
                selection.includes(object.id) ? withVisibility(object, path) : object,
            ),
        });
    return (
        <section className="hb-section" data-part="visibility">
            <h2>Visibility</h2>
            <div className="hb-binding" data-binding="visible">
                <Field name="bind.visible" label="Visible while" value={shared} readOnly={readOnly} onCommit={set} />
                <button
                    type="button"
                    className="hb-pick"
                    data-action="pick"
                    data-binding="visible"
                    title="Pick a variable"
                    aria-label="Pick a variable for Visible while"
                    aria-expanded={picking}
                    disabled={readOnly}
                    onClick={() => setPicking(!picking)}
                >
                    ⋯
                </button>
                {picking ? (
                    <BindingPicker
                        variables={variables}
                        problem={problem}
                        binding={{ type: first?.type ?? "", property: "visible", component }}
                        onPick={(node) => {
                            setPicking(false);
                            set(node.path);
                        }}
                    />
                ) : null}
            </div>
        </section>
    );
}
