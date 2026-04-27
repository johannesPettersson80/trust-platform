# OSCAT OOP Example README Template

Status: implementation contract for every OSCAT OOP example README.

Every paired example under `examples/OSCAT/<slug>/` ships one README at
`examples/OSCAT/<slug>/README.md`. This file is the skeleton. Use the section
headers verbatim and the same order. Replace the angle-bracket placeholders.

The README's job is to teach *why* the OOP version is shaped the way it is,
not to repeat the OOP concepts themselves. Concepts (interface, polymorphism,
patterns) are explained once at
[`docs/guides/oop-concepts-in-st.md`](../../../guides/oop-concepts-in-st.md).
Every README links into that file rather than re-explaining concepts inline.

Note: links shown in this template prose use `../../../guides/...` because
this template lives at `docs/internal/references/OSCAT/`. Links inside the
[Skeleton](#skeleton) block use `../../../docs/guides/...` because example
READMEs live at `examples/OSCAT/<slug>/`. The two paths are correct for
their respective locations; do not change them.

## Required Sections

The README must contain these sections, in this order:

1. **Title** — `# <Machine Name> — <Pattern Name>`
2. **Machine / process** — one paragraph on what the machine does
3. **When classic is the right answer** — three bullets describing the small
   stable spec where the procedural version stays cleaner
4. **Where classic strains** — one paragraph narrating how requirements grow
   until the procedural version visibly hurts
5. **Structure** — one Mermaid `classDiagram` of the OOP shape
6. **What happens at runtime** — one Mermaid `sequenceDiagram` of the main
   scan path
7. **The keystone** — one ST snippet, ten lines or fewer, showing the
   polymorphic moment or the pattern's defining call site
8. **Patterns used** — bullet list linking to concept anchors
9. **What this demo doesn't show** — honest disclosure of limits, with
   pointers to other examples that show the missing piece
10. **When NOT to use this** — three bullets on conditions where the OOP
    shape is overkill
11. **Integration map** — Markdown table of `%I/%Q/%M` bindings, comms list,
    OPC UA exposed records
12. **Run** — exact `trust-runtime test` commands for both projects

## Style Rules

- No code block longer than ten lines anywhere in the README.
- Use Markdown tables, not ASCII box-drawing tables.
- Mermaid for both diagrams (renders in GitHub, MkDocs, and VS Code without a
  build step).
- Link to concept anchors instead of re-explaining concepts inline.
- Be honest in "what this demo doesn't show". A reader who finds a missing
  feature must be pointed at the example that has it.
- Per-section length budget: each section should be readable in under a
  minute.

## Skeleton

```markdown
# <Machine Name> — <Pattern Name>

<One sentence: what the machine does. One sentence: what the OOP version
separates that the procedural version mixes.>

## When classic is the right answer

The procedural version is `non-oop/src/Main.st` (~XX lines). Use it when:

- <smallest stable spec where classic stays tidy>
- <a downstream condition that doesn't apply at this site>
- <an audit/scaling requirement that doesn't apply>

The OOP version costs ~Nx the lines. It earns that cost only when those
conditions break.

## Where classic strains

<One paragraph. Describe a sequence of three to five concrete requirements
that would arrive over months, and where each one would land in
non-oop/src/Main.st. End at the requirement that makes the procedural
version visibly ugly. This is the why.>

## Structure

\`\`\`mermaid
classDiagram
    <Mermaid class diagram of the OOP shape: interfaces, concrete FBs that
    implement them, the project FB that composes them. Use <|.. for
    "implements", *-- for "owns".>
\`\`\`

Read top-to-bottom in `oop/src/Main.st`: <interface 1> -> its
implementations -> <interface 2> -> its implementations -> <bus or
mediator or controller> -> <project FB that wires everything>.

## What happens at runtime

\`\`\`mermaid
sequenceDiagram
    <Sequence of the main scan path: who calls whom in what order when
    the public method fires once.>
\`\`\`

## The keystone

\`\`\`st
<Ten lines or fewer. The polymorphic moment, the chain wiring, the bus
publish loop, or whatever single snippet captures why this pattern fits
this scenario. Use a comment line if needed to point at what to notice.>
\`\`\`

<One short paragraph: contrast this snippet with what the procedural
version had to do for the same step. Name the new growth direction this
pattern unlocks.>

## Patterns used

This example combines:

- [<Pattern A>](../../../docs/guides/oop-concepts-in-st.md#<anchor-a>)
- [<Pattern B>](../../../docs/guides/oop-concepts-in-st.md#<anchor-b>)

ST mechanics used:

- [Interface](../../../docs/guides/oop-concepts-in-st.md#interface) and
  [IMPLEMENTS](../../../docs/guides/oop-concepts-in-st.md#implements)
- [Polymorphism](../../../docs/guides/oop-concepts-in-st.md#polymorphism)
- [Composition](../../../docs/guides/oop-concepts-in-st.md#composition)

## What this demo doesn't show

- **<missing feature 1>** — see `<other_example>/oop` for a worked
  implementation.
- **<missing feature 2>** — pointer to where the catalog covers this.
- **<missing feature 3>** — honest note on what an integrator must add.

## When NOT to use this

- <concrete condition specific to this scenario>
- <concrete condition specific to this scenario>
- <fixed-small-machine condition>

## Integration map

| Tag | Address | Direction |
| --- | --- | --- |
| `<SymbolName>` | `<%IXn.n / %QXn.n / %IWn>` | `IN` / `OUT` |
| ... | ... | ... |

Comms (from `io.toml`): `<modbus-tcp / mqtt / ethercat>`.
OPC UA exposed records (from `runtime.toml`): `<list of names>`.

## Run

\`\`\`bash
trust-runtime test --project examples/OSCAT/<slug>/non-oop
trust-runtime test --project examples/OSCAT/<slug>/oop
\`\`\`
```

## Acceptance

A README built from this template is acceptable when:

- Every section in [Required Sections](#required-sections) is present.
- Both Mermaid diagrams render in GitHub.
- The keystone snippet is ten lines or fewer.
- Every concept link resolves to a section in `oop-concepts-in-st.md`.
- The "what this demo doesn't show" section is concrete and honest.
- The "when not to use this" section names conditions specific to this
  scenario, not generic OOP advocacy.
- The integration map matches the actual `%I/%Q/%M` bindings in
  `oop/src/Configuration.st` and the drivers in `oop/io.toml`.

A README missing any of those is incomplete and must not ship.

## Anti-patterns to avoid

- Re-explaining "what is an interface" inside the README. Link instead.
- Code blocks longer than ten lines. The example file is for that.
- ASCII box-drawing in tables. Use Markdown.
- Vague claims like "MQTT publishes alarms" without naming the struct.
- Vague comms claims with no `io.toml` backing.
- "Pattern shown" bullet lists without explanation. Replace with a structure
  diagram and the keystone snippet.
- Repeating the same closing paragraph in every README. Each "when not to
  use this" must be specific to its own scenario.
