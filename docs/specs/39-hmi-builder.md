# HMI Builder

> **Rebuild, 7 October 2026:** equipment comes from the native [Plant library](trust-plant-library.md); sections 8–10 and §21 hold the agreed contract (one page format, operator panels, settings, Create PLC code, Create HMI, runtime fixes). §21 supersedes any earlier statement it contradicts. Catalogue detail: [hmi-component-catalogue.md](hmi-component-catalogue.md).

This specification defines the truST HMI Builder: a drag-and-drop editor in VS Code with which
customers build the operator screens of their truST projects, and the runtime behaviour of the
screens they build. It replaces the legacy HMI pages, their file format, their look and the
VS Code HMI preview, all of which are removed (§17.2). Old pages are not converted; the shipped
examples are rebuilt with the builder.

This is a truST product contract. IEC 61131-3 does not define HMIs, so this specification
creates neither an IEC requirement nor an IEC deviation. It keeps the decision of
`docs/PRODUCT_DECISIONS.md` (2026-07-26) that runtime HMI schema, values, writes, alarms,
trends and events are one typed product contract, with read-only operation and explicit write
permission authoritative. When this specification ships, the HMI sections of specifications 19,
22 and 25 are updated to point here. The behaviour of the shipped equipment function blocks is
owned by a separate specification (§8.2).

Normative words `shall`, `shall not`, `should` and `may` have their usual requirements
meanings. Examples are explanatory.

## 1. Scope

In scope:
- the editor, pages and screen layout;
- components and symbols, and process graphics with pipes;
- bindings to program variables, including members of function block instances;
- the shipped equipment pairs (function block and faceplate);
- generating pages from the program, and program declarations from pages;
- live mode;
- authorization and command handling;
- alarms, the look and the asset pipeline;
- the TOML file format, deployment and the runtime display.

Out of scope: 3D views, scripts inside pages, several people editing one page at the same time,
long-term history storage.

## 2. Terms

- **Page**: one operator screen.
- **Screen layout**: the top bar, navigation and theme shared by all pages of a project.
- **Popup**: a window opened from a page; a faceplate is a popup bound to one equipment instance.
- **Object**: anything placed on a page. It has a stable id, an optional tag, a type, a position,
  a size, properties, bindings, visualizations and events.
- **Component**: an object type from the component catalogue (value, gauge, button, pump, ...).
- **Symbol**: the graphic of a process component: two-dimensional and based on its P&ID symbol,
  with named parts, the moving part the equipment has, and ports (§14.2).
- **Port**: a connection point of a symbol (inlet, outlet, ...).
- **Pipe**: a connection between two ports, drawn by the editor along a computed route.
- **Equipment pair**: a function block type and the faceplate and symbol that belong to it.
- **Binding**: the link from an object property to a program variable or member.
- **Command**: an operator action that writes to the program: set, reset, toggle, pulse, write a
  value, acknowledge an alarm.
- **Runtime write policy**: the runtime's configured maximum of what may be written, and by which
  roles.
- **HMI bundle**: everything deployed together: screen layout, pages, popups, assets, faceplates,
  pair definitions and the page permissions, identified by a version.
- **Live mode**: the page running inside the editor with values from a simulator or a runtime.
- **Value states**:
  - **commanded**: what the HMI or the program asked for;
  - **confirmed**: what a measurement or feedback reports;
  - **estimated**: what a model or estimator calculates;
  - **unknown**: what cannot be known now, for example after a lost connection.
- **Stale**: a value whose last update is older than its freshness limit.

## 3. The editor

### 3.1 Documents

1. Page and popup files under `hmi/` shall open in the HMI Builder, implemented as a VS Code
   `CustomTextEditorProvider` on the TOML text document. **Open as text** shall remain available.
2. Every edit in the builder shall be a text edit of that document. Undo, redo, save, the dirty
   state and Git therefore behave as for any text file.
3. A file open in the builder and in a text editor at the same time (split view) shall stay
   consistent. A text edit appears on the canvas, and a canvas edit appears in the text.
4. When the file changes on disk while the builder holds unsaved edits, VS Code's normal
   conflict handling applies. The builder shall never silently discard either version.
5. After a VS Code restart, unsaved builder edits shall be restored with the text document
   (hot exit).
6. When the text is temporarily invalid, the canvas shows the last valid state and a banner with
   the errors. Canvas editing is paused until the text is valid again.
7. A save without changes shall produce a byte-identical file.
8. The truST side bar shall offer an **HMI** entry that lists the pages and creates new ones.

### 3.2 Layout

1. **Top bar**: page name, zoom, undo, redo, **Design / Live**, **Deploy**.
2. **Left panel** with tabs:
   - **Components**: search, categories, a picture preview of each component and its variants.
   - **Layers**: the objects of the page in z-order, with lock and hide.

   Pages are files, organised in VS Code's Explorer (decided with the rebuild of 7 October 2026,
   which removed the builder's duplicate Pages panel and page switcher): **New HMI Page** on a
   folder's context menu or in the command palette, rename, duplicate and delete as files (a
   rename updates the links to the page), and editor tabs switch between pages. A page's place in
   the navigation (order, icon, group, hidden) is set in the inspector with nothing selected.
   - **Assets**: images and customer symbols.
3. **Canvas**: the page at its design size, with zoom, pan, fit and a snap grid.
4. **Right inspector**, contextual to the selection, with tabs:
   - **Design**: General, Position, Appearance.
   - **Bindings**: each bindable property with its variable.
   - **Events**: click and other actions.
   - **Animations**: visualization rules (§5.3).
5. **Bottom panel**, collapsible, with tabs **Tags** (program variables to drag), **Alarms**,
   **Variables** (live values) and **Runtime diagnostics**.
6. **First view:** a new user sees Components, the canvas and the inspector. Pages, Layers and
   Assets are one click away, and the bottom panel is closed. Each user's panel arrangement is
   remembered.
7. **Arrangement:**
   - the left panel (200–480 px, 264 at first) and the inspector (240–520 px, 300 at first) are
     resized by dragging their inner edge, and each collapses to a 40 px strip that opens it again
     with one click;
   - the bottom panel opens and closes from its tab and is resized by dragging its top edge
     (120 px up to half the editor's height, 220 at first);
   - while the editor is too narrow for a canvas of 400 px beside the open panels, the side
     panels show as strips; widening the editor brings the user's arrangement back;
   - the arrangement is remembered for each user, the same in every page, and survives a
     restart.

### 3.3 Editing

1. Drag a component from the left panel onto the canvas to create an object. Dropping a program
   variable or an equipment instance creates the matching component (§7.1, §9).
2. Select by click, Shift-click and box selection. Move by dragging and with the arrow keys,
   resize by handles, and rotate process symbols in 90° steps.
3. Snap to the grid and to the edges and centres of other objects, with alignment guides.
4. Group and ungroup, align and distribute, bring forward and send back. Copy, paste and duplicate
   within and between pages, and delete.
5. **Grid and keys:** the snap grid is 8 px. Dragging and resizing snap to it; holding Alt places
   freely, in whole pixels. The arrow keys move the selection one grid step, with Alt one pixel.
   An object is at least one grid step wide and high.
6. **New objects** get their type's default size from the component catalogue and the first free
   id of the form `<type>-<n>`, for example `value-1` (§5.1). A component clicked instead of
   dragged is placed at the centre of the visible part of the page.
7. **Clicking empty canvas** clears the selection; Delete and Backspace delete it.
8. **Editing like a design tool** (planned with the user on 3 October 2026, Phase 5):
   - **Selection:** click; Shift-click adds or removes; a box dragged on empty canvas selects
     the objects wholly inside it; Ctrl+A selects all; Escape clears.
   - **Copy and paste:** Ctrl+C, Ctrl+X and Ctrl+V, also between pages, through the clipboard. A
     copy keeps every property and binding and gets a new id. Pipes between copied objects come
     along; pipes to objects not copied are dropped. Ctrl+D duplicates; after a copy is moved,
     the next Ctrl+D repeats that step. **Smart numbering:** a copied binding's last number is
     moved to the next one when that variable exists in the program (`TT101` to `TT102`,
     `P101.Running` to `P102.Running`); otherwise it stays as it was. An instrument keeps what it
     measures only when that was copied with it.
   - **Align and distribute:** with two or more objects selected, align left, centre, right, top,
     middle or bottom to the selection's outer edges; with three or more, distribute horizontally
     or vertically with equal gaps. While dragging, guides snap to other objects' edges and
     centres and show equal gaps.
   - **Groups:** Ctrl+G groups the selection and Ctrl+Shift+G ungroups. A group is invisible:
     each member records `group = "<name>"` in the page file. Selecting a member selects its
     group; a double-click edits one member. The visible group box (§5) is a separate component.
   - **Layer order:** bring forward, send backward, bring to front, send to back (Ctrl+],
     Ctrl+[, and with Shift); the order is the objects' order in the page file.
   - **Pages** are organised in VS Code's Explorer (§3.2 item 2). **Layers panel:** the objects front to back;
     a click selects, dragging reorders; the eye hides an object in the editor only, never in the
     runtime; the lock, saved in the file as `locked = true`, stops it being selected or moved on
     the canvas.
   - **Zoom and pan:** Ctrl+wheel and pinch zoom at the pointer; Space-drag and middle-drag pan;
     keys fit the page, zoom to the selection and show 100 %. On first open, fit with a minimum
     of 75% and allow scrolling; the user chose readability over a tiny whole-page view on
     4 October 2026. A saved zoom takes precedence, including a smaller one. Explicit Fit still
     fits the entire page without the 75% minimum.
   - **Context menu:** a right-click anywhere on the canvas: cut, copy, paste, duplicate, delete,
     group, layer order, align, lock, and on a symbol, open its faceplate (of the project or one
     of its libraries, in the HMI Builder). An object right-clicked outside the selection becomes
     the selection. Near the window's edge the menu opens inward.
   - **Variable picker:** while a run target is connected, each variable shows its current value.
     The Tags panel and a binding's picker read only the variables they show (at most 1 000, with
     `hmi.read`, §11 item 6) while they are open. A value is written TRUE or FALSE, as a number, a
     STRING or WSTRING in quotes (so an empty one shows), a time or date as the runtime writes it,
     and "?" when the running program lacks the variable or its value is unknown.
   - **Live mode controls:** user decision, 4 October 2026: controls operate the run target
     through a named user's HMI login, under that user's runtime role and the existing write
     policy. Keep the existing confirmations and show a banner naming the runtime, user and
     role. The editor's administrative control token does not replace this HMI session.

### 3.4 One drawing module

The canvas shall draw pages with the same drawing module that the runtime uses (§17.3). Editing
aids (selection, handles, guides, grid) are drawn on a separate layer above it. A page shall look
the same in the editor, in Live mode and in the runtime.

The canvas takes the design size, theme, symbol style and project colours from
`hmi/screen.toml`; without it, 1920×1080 in the dark theme with flat symbols. Without a connected
source (§11), the canvas draws each bound value as a design sample so a page looks finished while
it is designed: a number at 60 % of its range (of 0–100 without one), a state as on, a text as
its variable's name, and tables empty. Equipment shows its normal state: a pump
running, a valve open, and no fault, manual or out of service (decided on 2 October 2026 while
the user was away, approved by the user on 3 October 2026). Design samples never leave the editor.
Trends in Design mode show deterministic sample history over their configured window, ending
at the sample reading. Explicit object or pen ranges supply the shared numeric sample before
unscaled consumers of that path, so the trace remains on scale. A visible “Design sample data” notice identifies it. Placed faceplates
use the same preview history. Live mode never fills missing history with samples.

The drawing module is given the page, the design size, and for each bound path its live value:
the value, its state (confirmed, commanded with its target, estimated, or unknown) and, when
stale, its age. Trends also get their recent samples, tables get structured values (arrays and
structures), an event log gets the recent events, and an alarm list gets the alarms with their
text, priority, time, and whether each is active and acknowledged. From these alone it returns
the drawing, coloured only through the design tokens (§14.1). It does not fetch data, so the
editor, Live mode and the runtime feed it the same way.

### 3.5 Usability (phase 1)

1. **Keyboard:** every editing action shall work from the keyboard: add a component through
   search, select next and previous, move, resize, delete, open the inspector. Decided on
   2 October 2026 while the user was away, approved by the user on 3 October 2026: `/` goes to the component search, where
   Enter adds the first match at the centre of the visible page; Tab and Shift+Tab select the
   next and the previous object in z-order; the arrow keys move the selection by one grid step
   and Shift+arrow resizes it (right and down grow, left and up shrink), each with Alt for one
   pixel; Delete removes it; Enter goes to the inspector's first field; Escape clears the
   selection.
2. **Runtime controls:** touch targets of at least 44 × 44 px and text of at least 14 px at the
   design size.
3. **Status cues:** a status is never shown by colour alone; text or shape always carries it too. A
   button shows its command's outcome by a sign at its right end as well as its outline:
   … pending, ✓ applied or confirmed, ! refused or not confirmed, ? unknown.
4. **Scaling:** runtime pages scale uniformly to fit a screen at least their design size; on a
   smaller screen they adapt (§4.5).
5. **Themes:**
   - the editor shell uses truST's existing webview design tokens and follows the VS Code theme;
   - the canvas previews the runtime theme selected in `screen.toml`;
   - both are defined in the one design system of §14.1.

## 4. Pages, screen layout and popups

1. `hmi/screen.toml` holds the screen layout:
   - design size (default 1920×1080);
   - theme (light or dark);
   - the project's own colours, if any (§14.1);
   - symbol style: `flat`, the two-dimensional symbols of §14.2;
   - top bar: title, logo, clock, alarm counter, logged-in user, page title;
   - navigation: side bar left or right, tabs on top, or none.
   Templates provide complete screen layouts to start from.
2. Pages are free: a page is not tied to a program, and its objects may bind to variables of any
   program.
3. Navigation is built from the page list: order, title, icon, hidden pages, page groups.
4. Popups are designed like pages, with their own size and title. They open from events (§5.4),
   are modal or pinnable, and close by button or Escape.
5. Adaptive display (phase 3):
   - process graphics scale uniformly;
   - layout containers (rows, columns, grids) rearrange their content for smaller screens;
   - the editor previews 1920×1080, 1280×800, 1024×768, and tablet in portrait and landscape.

   Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026:
   - **When.** A page area at least the design size shows the page scaled uniformly to fit, as
     before (§3.5 item 4). A page area smaller than the design size in either direction shows
     the page at scale 1, so its texts and touch targets keep their sizes (§3.5 item 2): when the
     area is narrower than the design width, the page is laid out again at the area's width;
     when the page is then taller than the area, it scrolls vertically.
   - **Blocks.** For that layout the page is divided into blocks: each panel or group inside no
     other panel or group, with the objects lying wholly inside it; each other display, control
     and text on its own; and the equipment, lines and images in no panel or group, with the
     pipes, as one block: the process graphic.
   - **Placing blocks.** In reading order, each block keeps its design position when it lies
     within the width and overlaps no block already placed. Reading order goes by bands, top to
     bottom: blocks whose heights overlap, directly or through others, form a band; within a
     band by columns, left to right: blocks whose widths overlap form a column; within a column
     top to bottom. So a design's columns stay together when they come to lie one under the
     other.
     Otherwise it moves left as far as it must to fit, but not past the page's left margin (the
     leftmost block's design left edge). If it then overlaps a block already placed, it starts
     at the left margin instead, as a new row would, and moves down until it overlaps none,
     16 px below the blocks above it.
   - **A block wider than the width** less the margins: an arranged panel or group (next item)
     keeps its objects' sizes and rearranges them; any other block, the process graphic
     included, scales uniformly to fit: its objects' positions and sizes and its pipes' bends
     scale, its texts keep their size. A block holding controls scales no further than keeps
     each control at least 44 × 44 px; if it is still too wide, the page scrolls sideways.
   - **Arranged containers.** A panel or group may set `arrange`: `"row"`, `"column"` or
     `"grid"`, and `gap`, the space between its objects in pixels (default 16). At its design
     width nothing moves. Narrower, it places its objects in reading order inside its padding
     (16 px, below a panel's title): a row from left to right, starting a new row when the next
     object does not fit; a column one under the other; a grid in cells the size of its largest
     object, as many columns as fit. An object wider than the container's inner width shrinks
     to it, in proportion, as a scaled block does. The container grows downward to hold them.
   - **Previews.** In Design mode the top bar offers the screen sizes: the design size,
     1920 × 1080, 1280 × 800, 1024 × 768, tablet landscape 1180 × 820 and tablet portrait
     820 × 1180. The canvas then shows the page as the runtime lays it out in that screen's page
     area (the screen less its top bar and navigation, item 7), read-only, until the design size
     is chosen again.

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026:

6. **Page list.** A page file may set, after its title:
   - `order`: a whole number; pages are listed by order, then by file name, and a page without
     an order comes after the ordered ones;
   - `icon`: one of the shipped icons `overview`, `process`, `trend`, `alarm`, `settings`,
     `pump`, `valve`, `tank`, `motor`, `heat`, `flow`, `power`, `list` and `info`;
   - `group`: a group name; the side bar lists a group's pages under its name, and tabs keep a
     group's pages together;
   - `hidden = true`: the page is left out of the navigation; page buttons still reach it.
7. **Runtime layout.** The screen's `size` is the design size of the page area. The runtime
   draws the top bar (56 px high) above it and the navigation beside it: a side bar (232 px wide,
   on the left or the right) or tabs (48 px high, under the top bar). The page scales uniformly
   to fit the space that remains (§3.5). The navigation appears when at least two pages are not
   hidden. Each template chooses a size that fits its panel together with its top bar and
   navigation.
8. **Top bar.** It shows what `[top_bar]` turns on: the title and the logo (an image under
   `hmi/assets/`), the clock (local time with seconds), the alarm counter and the newest alarm
   (§13.6), the logged-in user with Log in or Log out, and the page title. Without a top bar,
   the session shows at the top right. Refined on 7 October 2026, when a new screen kept its
   folder's name: a new screen's title is its project folder's name in words (`room-heating` is
   "Room heating"), and the screen settings edit it.
   While the runtime runs a simulation, a **SIMULATION** badge shows in the top bar, or at the top
   right with the session when there is no top bar, whatever `[top_bar]` turns on. It adds the
   speed when that is not 1× ("SIMULATION ×10"), and when the speed is above 1× and the achieved
   speed is more than 5 % below it, both ("SIMULATION ×7 of ×10"; specification 46, §6.1). The
   runtime gives these words: it decides the speed and whether the achieved speed is behind
   (specification 35, B9.2), and its status answer carries the same speed as the words
   `simulation_speed_text` (annex E1.3). The live stream sends
   `{"simulation": {"enabled", "time_scale", "achieved_time_scale", "speed_configured", "speed_achieved"}}`
   after the session message and whenever it changes (§11.1). `speed_configured` is the configured
   speed when it is above 1×, else `null`; `speed_achieved` is the achieved speed as text, to one
   decimal rounded down, when it is behind, else `null`. The page shows those words and translates
   only the phrase "{achieved} of {configured}" for its language. The badge is never hidden by a
   page setting.
9. **Popups.** A popup file `hmi/popups/<name>.toml` has `format`, `title`, `size` (its design
   size), optionally `pinnable = true`, and objects and pipes as a page has. An object opens it
   with `[object.click] popup = "<name>"`, and a page button goes to its page with
   `[object.click] page = "<name>"`.
   - A popup opens over the page with its title and a close button. Its values and commands work
     as on a page (§11.1, §12); a command names `popup:<name>` as its page.
   - It is modal: the page behind is dimmed and takes no presses. A pinnable popup has a pin
     button; a pinned popup stays open beside the page, and the page can be used again.
   - It closes with its close button or Escape.
10. **Templates.** The editor's New HMI Screen action creates `hmi/screen.toml` and a first page
    from a template:
    - **Operator panel:** for a 1920 × 1080 panel, a top bar with title, clock, alarms and user,
      and a side bar on the left; page size 1688 × 1024.
    - **Overview:** for a 1280 × 800 panel, a top bar with title, clock and user, and tabs; page
      size 1280 × 696.
    - **Single page:** 1024 × 768, without top bar or navigation.

    Unless the program already calls `Hmi.Display`, it also adds the HMI library, a `Screens`
    program that calls it and its task, so the new screen shows live values (§18 phase 5).

## 5. Objects and components

### 5.1 Object

An object has:
- `id`: stable and unique within its file. It is created once, short and readable (for example
  `pump-1`), and never changes when the object is moved, renamed or rebound.
- `tag`: the displayed equipment tag (for example `P101`), editable and separate from the id. By
  default it is taken from the bound instance's name. Two objects may show the same equipment, for
  example on two pages.
- `type`, and for symbols the variant.
- Position and size in whole pixels on the grid, and rotation.
- Properties: text, unit, range, decimals, style; for an image, `image`: a project path under
  `hmi/assets/` (no absolute path, no `..`), checked when the file is read; for a trend,
  `span`: the time it shows, in whole seconds (default 300).
- For a control, its permissions and confirmation (§12.8): `role` (`operator` or `engineer`),
  `confirm` (a question), `action` (`set`, `reset` or `toggle`, for a button), `pulse` (seconds,
  for a pulse button), `expect` (a boolean, number or text) and `timeout` (seconds). `pulse` and
  `timeout` are positive and finite.
- For an instrument, `attach`: the equipment or pipe it measures (§6, item 5).
- For a placed faceplate (type `faceplate`), `faceplate`: the faceplate it shows, and
  `instance`: the instance it is bound to (§8.9 item 3). Both are required on a placed faceplate
  and an error on any other object, each at its line.
- For a panel or group, `arrange` (`row`, `column` or `grid`) and `gap` (whole pixels): how it
  rearranges its objects on a smaller screen (§4, item 5).
- Bindings, visualizations and events.

Text objects come in three sizes, chosen by their variant: `heading`, `body` (the default) and
`caption`. A page button names its page as its click target (§5.4).

### 5.2 Component catalogue

The catalogue shall cover at least every plant, instrument and control structure used in the
control course (specification 38); §5.5 maps the course to the components.

Phase 1:
- **Displays**:
  - text, image, value with unit and quality (§5.3);
  - gauge, bar, split bar (two outputs from one request);
  - trend with several pens (for example PV, SP and MV) and limit bands;
  - status indicator, table (also for arrays and matrices), event log, alarm list.
- **Controls**: button, pulse button, switch, selector, Manual/Auto/Track selector, slider,
  numeric input, setpoint entry with limits.
- **Layout**: panel, group, page button, line.

Phase 2, the process and motion equipment used in the control course:
- **Thermal**:
  - cold room with door, refrigeration unit;
  - electric heater with water vessel, cooler;
  - jacketed vessel (vessel and jacket), coupled vessels with a heat-transfer wall;
  - heat draw-off.
- **Liquid**:
  - open tank, gravity-drained tank (bottom outlet), tank cascade;
  - transfer pump (fixed and variable speed), dosing pump;
  - on/off valve, control valve with positioner and stem position;
  - supply line (regulated or fixed pressure), flow junction and mixing point.
- **Motion**:
  - motor, servo motor with drive, shaft, drum with hanging load;
  - flexible coupling with load, encoder;
  - rotary arm (a joint working against gravity);
  - axis position indicator (actual and target angle, following error).
- **Instrumentation**: temperature probe, level, flow, pressure, position and angle, speed,
  valve stem position, redundant sensor pair with disagreement indication, and the loop
  controller of the PID loop and controller pairs (§8.1).
- **Communication**: network link status (connection, packet age, hold, fallback).

Phase 3:
- **Process**: vessels, heat exchangers, filters, cyclones, crushers, conveyors, screens.
- **Instrumentation**: weight.
- **Electrical**: motors, VFDs, contactors, breakers, drives.

Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026: the conveyor, the motor and the
weigh scale came with phase 2 (§8.8); phase 3 adds these, grouped under **Process** and
**Electrical** in the Components panel (§5.6 item 1). Their shapes follow ISO 10628-2 (process)
and IEC 60617 (electrical); a machine's flags (`fault`, `manual`, `out-of-service`) work as the
pump's do (§5.3 item 4). Electrical symbols have no ports: a single-line diagram joins them with
lines.

| Type | Ports | Bindings | Shows |
|---|---|---|---|
| `vessel` | `inlet` top, `outlet` bottom | `level` | a closed vessel with dished heads, filled to its level, which the word gives |
| `heat-exchanger` | `inlet`, `outlet` (product, left and right), `medium-inlet` top, `medium-outlet` bottom | `temperature`, `fault` | the shell with the medium's tube coil through it; the word gives the product's temperature, or Fault |
| `filter` | `inlet`, `outlet` | `differential-pressure`, `clogged`, `fault` | the housing with its filter medium across it; clogged, the medium is drawn in the warning colour and the word says Clogged, else it gives the differential pressure |
| `cyclone` | `inlet` (tangential, top left), `gas-outlet` top, `solids-outlet` bottom | `differential-pressure`, `fault` | the cylinder and its cone; the word gives the differential pressure, or Fault |
| `crusher` | `inlet` top, `outlet` bottom | `running`, `fault`, `manual`, `out-of-service`, `load` | the housing and its hammer rotor, which turns while it runs; running, the word gives its load in percent |
| `screen` | `inlet` top left, `oversize` right, `undersize` bottom | `running`, `fault`, `manual`, `out-of-service` | the inclined deck with its mesh and its exciter, whose weight circles while it runs |
| `vfd` | none | `running`, `fault`, `manual`, `out-of-service`, `frequency` | the converter (a square with its diagonal, AC in, AC out); running, the output is drawn in the moving colour and the word gives the frequency with the object's unit |
| `drive` | none | `ready`, `running`, `fault`, `manual`, `out-of-service`, `speed` | the converter with DC in and AC out; the word says Fault, Running with the speed, Ready, Not ready or Stopped, in that order |
| `contactor` | none | `closed`, `fault` | the main contact with its contactor arc, open or closed, drawn in the moving colour while closed; the word says Open or Closed |
| `breaker` | none | `closed`, `tripped` | the main contact with its breaker cross; tripped, the contact stands open and the word says Tripped as a fault |
| `device` | none | `running`, `fault`, `manual`, `out-of-service` | a plain unit with a control panel's lines, its lamp lit in the moving colour while it runs: the picture of a customer's own function block until they choose their own (§21.9) |

### 5.3 States, data quality and visualizations

1. **Value states.** The display shall distinguish:
   - **confirmed**: shown normally;
   - **commanded** but not yet confirmed: shown as pending, with the target and a transition
     marker;
   - **estimated**: marked as estimated, for example with an "est." label;
   - **unknown**: shown as unknown (a "?" and a neutral hatch), never as the last good state.
2. **Stale data.** A value that is stale keeps its last value with a stale marking and its age.
   Each binding has a freshness limit, by default three expected update periods.
3. **Animations.** An animation (rotation, flow, blinking state) represents a current
   observation only. It shall stop while its data is stale, unknown or merely commanded.
4. **Symbol states.** Symbols have states: running, stopped, fault, manual, out of service; open,
   closed, moving; level fill. Each state is driven by the bindings named in its visualization
   rule. Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026, for the symbols of
   milestone 2A (§14.2):
   - **Pump** (`running`, `fault`, `manual`, `out-of-service`, all `BOOL`): the impeller turns
     while `running` is TRUE, also during a fault, and stands still otherwise. The state word is
     Fault when `fault` is TRUE, else Running or Stopped.
   - **On/off valve** (`open`, `closed`, `fault`, `manual`, `out-of-service`, all `BOOL`): with
     both feedbacks bound, open or closed by the one that is TRUE, moving when neither is, and a
     fault when both are; with one feedback bound, the other end is its opposite. Closed: the
     body is filled and the ball's bore lies across the pipe. Open: the body is hollow and the
     bore lies in line. Moving or both feedbacks: the bore stands at 45°. The state word is Fault
     when `fault` is TRUE or both feedbacks are, else Open, Closed or Moving.
   - **Control valve** (`position`, a number in percent of travel, 0 closed and 100 open;
     `fault`, `manual`, `out-of-service`, all `BOOL`): the plug stands at the position, on its
     seat at 0 and lifted by its full travel at 100. The body is filled at 0 and hollow above it.
     The state word is Fault when `fault` is TRUE, else the position, for example "62 %".
   - Fault shows its word in the alarm colour and the "!" sign beside it; manual shows "M"; out
     of service shows "OOS" and dims the symbol.
   - When a binding that decides the state is unknown, the moving part is not drawn, the body
     shows the unknown hatch, the state word is "?" and nothing turns. A stale binding keeps the
     last state with the stale outline and its age as a sign beside the state word; nothing turns
     (item 3).
   Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026, for the symbols of milestone
   2C (§5.2 phase 2), each drawn in the same style (§14.2):
   - **Liquid:** the open tank and the gravity-drained tank (`level`, filled from the bottom to
     its share of the object's range in the accent of live values, the word giving the level),
     the tank cascade (`upper-level`, `lower-level`), the dosing pump (`running` and the pump's
     flags; its piston strokes while it runs; `rate` as its word while it runs), the supply line
     (`pressure`; variant `regulated` draws its pressure regulator), the mixing point (`ratio`),
     the pump's variant `variable-speed` (the ISO 14617 arrow of variability behind the casing,
     `speed` in percent as its word while it runs) and the control valve's variant
     `positioner` (its box on the stem; `demand` as a pointer beside the plug; the word
     "62 % → 70 %" while position and demand differ).
   - **Instrumentation:** the temperature, level, flow, pressure, position and speed sensors
     (an ISA-5.1 bubble with TT, LT, FT, PT, ZT or ST; `value` as the word, a sensor `fault`) and
     the redundant sensor pair (bubbles A and B; `a` and `b` as the word; `disagree` makes the
     word "Disagree" a fault, with a crossed equals sign between the bubbles). An instrument's
     tag and word stand beside its bubble, which takes at most half the object's width.
   - **Thermal:** the cold room (`door-open` swings its door open, with its arc; `temperature`),
     the refrigeration unit and the cooler (their fans turn while `running`), the electric heater
     with its water vessel (`power` lights its element and makes heat rise; `temperature`;
     `level` in percent), the jacketed vessel (`level` in percent, `temperature`), the coupled
     vessels (`temperature-1`, `temperature-2`; arrows through their wall from the warmer side
     while they differ by a hundredth of the range) and the heat draw-off (`power`; heat rises
     from its radiator while it draws).
   - **Motion:** the motor (decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026: the
     IEC circle with M, `running` and its flags, its rotor's mark circling while it runs), the
     servo motor with its drive (`running` and its flags, `speed` as its word
     while it runs, `angle` placing its rotor's mark, which circles while it runs without one),
     the shaft (`angle`, `speed`), the drum (`angle` turns it, `position` lowers its load by its
     share of the range), the flexible coupling (`motor-angle`, `load-angle`), the encoder and the
     rotary arm (`angle`) and the axis position indicator (`actual` and `target` needles, the
     target's pointer on the rim; the word "87.5 ° → 90.0 °" while they differ). Angles count
     from straight up, clockwise; the rotary arm hangs straight down at 0°.
   - **Communication:** the network link (`connected`, `age`, `hold`, `fallback`,
     `out-of-service`): connected, its packets travel and its word gives their age; lost, the line
     is broken and it is a fault; holding, a pause stands in the line; fallen back to local
     control, a turning arrow over the local station; switched off on purpose (not connected and
     out of service), it says Off, its line whole and still, with no fault (decided on 3 October
     2026 while the user was away, approved by the user on 3 October 2026: a link switched off showed as lost).
5. **Visualization rules** map bindings to symbol parts: rotate a part, colour by state, fill a
   level, show a value, blink while an alarm is unacknowledged. Decided on 3 October 2026 while
   the user was away, approved by the user on 3 October 2026: a symbol declares its rules, each naming a part and a binding:
   `spin` (it turns while the binding is TRUE), `stroke` (it moves to and fro while TRUE),
   `active` (it shows working while TRUE or above zero), `angle` (it stands at the number's
   angle), `level` (filled to the number's share of the object's range, or of a range the rule
   gives), `travel` (moved by that share), `blink` (while TRUE), `show` (a number with the
   object's decimals and unit), `alarm` (in the alarm colour while TRUE) and `equals` (shown only
   while the number equals the rule's `value`: one of several state pictures, such as stopped,
   filling and mixing for an INT state; decided on 7 October 2026 with the user). Turning, moving
   to and fro and blinking show a current observation only (item 3).
6. In Design mode a state switcher shows each state, including stale and unknown, without a
   connected source. Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026: the
   switcher is the inspector's Preview row for the selected object: Sample (the design samples),
   the symbol's own states, then Stale and Unknown; other components offer Commanded,
   Estimated, Stale and Unknown. A state that needs a binding the object lacks is offered
   disabled and names it. The preview changes only what the editor draws: it is never saved, and
   Live mode ignores it. A symbol's design sample is its own sample state.
7. **Labels** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): an object's
   label is drawn by every component that takes one; a bar and a split bar draw it above their
   bars, a horizontal bar above its track. Text that does not fit is cut (§14.1, item 4).
8. **Durations** (decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026): a duration
   reads in words of its parts, for example `1 h 2 min 3.5 s` for `T#1h2m3s500ms` and `0 s` for
   `T#0s`, with the language's decimal sign. The runtime sends it as an IEC literal (§11.1).
   Refined on 7 October 2026, when a sequence's step time read `2 min 54.353 s`: an object's
   `decimals` round a duration's seconds, carrying a minute they round up to (`2 min 54 s` with 0
   decimals; `T#1m59s600ms` reads `2 min`), and without `decimals` it reads exactly as before.

### 5.4 Events

A click shall be able to:
- open a faceplate or popup;
- go to a page;
- send a command: set, reset, toggle or pulse a BOOL, or write a value.

Every command follows §12. An object with a click takes it anywhere in its box, also where its
symbol leaves the box empty (decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026).

### 5.5 Control course coverage

| Course content | Components and faceplates |
| --- | --- |
| Cold room: ON/OFF, hysteresis, dwell, commissioning | cold room with door, refrigeration unit, temperature |
| Water heater: P, PI, saturation, anti-windup, identification, tuning, filtering, timing, initialization, transfer, parameter updates, measurement validity | electric heater with water vessel, heat draw-off, temperature probe, PID loop faceplate, trend with PV/SP/MV, value quality |
| Valve-fed heater: actuator tracking, valve characteristics, stiction | control valve with positioner and stem position, supply line, temperature |
| Shaft and drum: position control, PD, PID, derivative filter, discrete and own PID laws, servo commissioning | servo motor with drive, shaft, drum with hanging load, encoder, axis position indicator, PID loop faceplate |
| Tanks: volume balance, level control, gravity outflow, linearization, gain scheduling | open tank with inlet and outlet pumps, gravity-drained tank, level, PID loop faceplate |
| Classical design: poles, root locus, frequency response, sensitivity, loop shaping, margins, resonance, sampling | PID loop faceplate and trends; flexible coupling with load |
| Industrial structures: trajectory feedforward, feedforward, cascade, ratio, split range, override, Smith prediction | two linked PID loop faceplates (a cascade, §10.5); heater and cooler; jacketed vessel; mixing line with dosing pump |
| Multivariable: coupled vessels, relative gain, PID group, decoupling, design selection | coupled vessels, PID loop faceplates |
| State space: state models, controllability, observability, state feedback, observers, Kalman, LQR, LQG, MPC | tank cascade |
| Nonlinear, adaptive and robust control: Lyapunov, feedback linearization, adaptive, robust | rotary arm with gravity compensation, PID loop faceplate |
| Networked and fault-aware control, loop diagnosis | network link status, redundant sensor pair |

The course's other controller structures (ON/OFF, ratio, split range, override, feedforward,
state feedback, estimators, MPC and the others) are shown with the general components: values,
setpoints, mode selectors, statuses and trends bound to the lesson's own variables. A coverage
test shall list every plant type and every faceplate the course uses and fail when one is
missing.

### 5.6 Component definitions

1. Each component type is defined once, in a catalogue shipped with the runtime. A definition
   gives:
   - its category (display, control, layout or equipment) and the phase that introduces it;
   - its default size and its variants;
   - for equipment, the heading of §5.2 it is listed under (liquid, thermal, motion,
     instruments or communication), which groups it in the editor's Components panel (decided
     on 3 October 2026 while the user was away, approved by the user on 3 October 2026);
   - its bindable properties, each with the kinds of value it accepts (number, boolean, text,
     time, enumeration, structure, array, or any single value) and the directions it allows.
2. The runtime, the language server and the editor read this one catalogue; the drawing module
   draws every type it defines. The editor offers the types the drawing module draws, so a type
   that a later phase brings in appears when its drawing does (decided on 2 October 2026 while
   the user was away, approved by the user on 3 October 2026).

### 5.7 Data displays

1. **Trend.** Every binding other than `high-limit` and `low-limit` is a pen, in the order of
   the binding names. The trend shows the last `span` seconds up to now, labelled relative to
   now (for example "−5 min" to "now").
   - Pens are drawn in the pen colours `pen-1` to `pen-4` (§14.1); from the fifth pen on, the
     colours repeat dashed.
   - Number pens share the value axis: the object's range when both ends are set, otherwise the
     samples and limits in view with a margin. A sample outside the axis is drawn at its edge. Compact plots reduce the number of axis
     labels to keep at least 20 design pixels between labels; the scale and traces do not change.
   - A boolean pen is a step line in its own lane under the plot, high while TRUE.
   - A limit is a dashed line in the warning colour with a faint band beyond it. A limit whose
     value is unknown, or outside the object's range, is not drawn.
   - The legend shows each pen's name and current value in its state (§5.3): "?" when unknown,
     the age when stale, "est." when estimated. A trace ends at its last sample; it is never
     drawn on to now.
   - **Pens in the editor** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): the
     inspector's Add pen takes a variable, typed or picked, and adds it as a pen named after the
     variable's last part, with a number when that name is taken (`Level`, `Level2`). Clearing a
     pen's binding removes the pen.
     Refined on 7 October 2026, when two transmitters' pens both read "Value" in the legend: the
     pen is named as its legend shows it, the instance's tag for its value (`Main.TT101.Value` is
     "TT101"), the tag and the member in words for another member (`Main.TIC101.Setpoint` is
     "TIC101 setpoint"), and a program's own variable in words (`Simulation.WindowOpening` is
     "Window opening"); a taken name gets a number after a space ("TT101 2").
   - **History** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): when the
     runtime's historian is enabled, a trend on the runtime display opens with its pens' samples
     of the last `span` seconds and then adds the stream's samples. The display asks
     `GET /api/hmi/history?path=…&path=…&span=<seconds>` (Viewer role; at most 16 paths, `span`
     1 to 86 400, default 300), which answers `{ "enabled", "samples": { "<path>": [[time_ms,
     value], …] } }`: oldest first, at most 5 000 per path, numbers as numbers and booleans as
     0 or 1. A path the historian does not record under the same name has no samples, and
     without the historian the trend starts empty as before. (The historian names array elements
     by position from 0 and does not record inherited members, so their trends start empty.)
2. **Table.** A structure is one row per field, with its name and value. An array is one row
   per element, labelled with its index; an array of structures has one column per field; a
   two-dimensional array is a grid with row and column indices. Indices count from the declared
   lower bound. A value nested deeper is shown by its size, for example `[10]` for an array and
   `{4}` for a structure with four fields. Rows that do not fit end with a "+N more" row; an
   empty array shows "Empty". An unknown value shows "?" over the neutral hatch; stale and
   estimated values are marked as on other values (§5.3). Decided on 3 October 2026 while the
   user was away, approved by the user on 3 October 2026:
   - a `BOOL` is a mark, lit while TRUE, never the words TRUE and FALSE;
   - field names read as words, as options do (`FirstOut` reads "First out", §5.8);
   - with column titles, each column takes the width its longest title or cell needs, and a
     column of texts reads from the left, so a condition's text is shown whole beside its marks;
   - the rows at the end of an array of structures that hold nothing (empty texts, FALSE and 0)
     are left out and counted in an "Empty rows: N" row, so a fixed array of 16 conditions with
     three in use shows those three.
3. **Event log and alarm list** are drawn with the alarm and event sources (§13.6, §13.7).
4. **Alarm list** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026):
   - tabs for Active, Shelved, Suppressed and Out of service, each with its count; Active is open
     first;
   - a row per alarm in the order of §13.6, item 3: the priority mark, blinking while the alarm is
     unacknowledged and its data fresh; the message; and the time it became active, its path and
     its state (Unacknowledged, Acknowledged, or Cleared, unacknowledged);
   - an unacknowledged row has an Acknowledge button. Selecting a row opens the alarm: its path,
     priority, state, time, first out and how often it was raised, and the actions its state
     allows (specification 40, §4): acknowledge; shelve for 15 minutes or 1, 4, 8 or 24 hours;
     unshelve; reset, for a latching alarm; a comment; and, for an Engineer, out of service and
     back in service. Without a session an action asks to log in, as controls do. The runtime
     decides these actions: each alarm in the list it sends carries the actions its state allows
     the receiving session (`actions`), and the list carries the shelving durations and the longest
     comment (`shelve_for`, `comment_max_chars`). The display offers exactly these, and the runtime
     refuses any other action (specification 40, §4);
   - rows that do not fit are shown in pages, with "1–6 of 9" and buttons for the previous and
     the next page; an empty tab says so, for example "No active alarms";
   - in the editor, without a connected source, the list is empty.
5. **Event log** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026):
   - a row per record, newest first: what happened (Raised, Cleared, Acknowledged, Shelved,
     Unshelved, Suppressed, Unsuppressed, Out of service, In service, Reset or Comment; for a
     command its outcome: Confirmed, Applied, Refused or Timed out) with the alarm's message or
     what the command asked, and below it the time, the path, the user and the detail (a
     comment's text, a shelving's seconds, a refusal's reason);
   - an alarm's record carries its priority's mark, a command's record a mark of its outcome;
   - rows that do not fit are shown in pages, as in the alarm list;
   - it shows the records the runtime keeps when the page opens (specification 40, §5.1) and
     those the stream adds; in the editor it is empty ("No events yet").

### 5.8 Controls

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026.

Every control sends its commands as §12 describes. The switch, selector, mode selector and
slider act on the value they show, so while it is unknown or stale they send nothing and say
that the value is not known.
1. **Setpoint entry and numeric input** ask for a number within their limits.
2. **Button and pulse button** send their action (§12.8).
3. **Switch** (`value`, a BOOL) shows On or Off as a sliding knob and the word; a click writes
   the opposite of the value shown. Its track has a control's usual size: at most 32 px high,
   centred in the object, and about twice as wide, so a toggle in a tall row stays in proportion
   to its 14 px label; the whole object still takes the click (refined on 7 October 2026, when a
   56 px row drew a 106 px toggle). Its `options` may name its two states' words, off first: a
   PID loop's Manual switch reads Auto or Manual (`options = ["Auto", "Manual"]`); the track
   widens to its longer word (decided on 7 October 2026).
4. **Selector** (`value`, an enumeration or a number) shows the current choice; a click opens
   the list of its `options` and the chosen one is written:
   - for an enumeration, each option is one of its value names and is written by name;
   - for a number, the options are labels and option *n* writes *n*, counting from 0.

   `options` is required, and the binding checks report an option that is not a value of the
   bound enumeration.
5. **Mode selector** (`mode`, an enumeration) shows its modes side by side, the active one lit;
   a click on a mode writes it by name. Its modes are `Manual`, `Auto` and `Track`, as
   `Control.PIDMode` has, unless `options` lists the modes of the bound enumeration, for example
   `Auto`, `Manual`, `OutOfService` for the equipment library's devices; the binding checks
   hold them against the enumeration. A value name
   is shown as words in the display's language: `OutOfService` reads "Out of service" (decided
   on 3 October 2026 while the user was away, approved by the user on 3 October 2026).
6. **Slider** (`value`, a number; variants `horizontal` and `vertical`) runs from `min` to
   `max` (default 0 to 100). Dragging moves a preview; the value is written once, on release,
   rounded to `decimals`, and the slider shows the program's value again when the command ends.

For every control, the runtime checks the value against the control on the page: a switch
takes TRUE or FALSE, a selector and a mode selector one of their options, and a number stays
within the control's limits.

### 5.9 Operator displays (Phase 5)

Planned with the user on 3 October 2026.

1. **Trends:**
   - a trend opens filled from the runtime's history, which is on by default and keeps 24 hours
     of each trended value; longer history goes through the OpenOT database;
     - decided on 4 October 2026 while the user was away (to be approved): the runtime keeps, in
       memory, every value a trend of the active bundle's pages and popups shows (its pens, not
       its limit bands), read at each call of `Hmi.Display` whether or not a screen is open: a
       sample a second for the last hour, then each minute's lowest and highest value for 24
       hours, about 100 kB a value. The values are taken again when a bundle is activated
       (start, deployment, rollback). `GET /api/hmi/history` answers from it, else from the
       historian for other values, and says `"enabled": true`. A faceplate's trend, bound per
       instance, is not kept yet;
   - range buttons for 5 min, 1 h, 8 h and 24 h, and a chosen from and to;
   - a box dragged on the plot zooms into its period, the wheel zooms the time axis, a sideways
     drag pans; zooming or panning pauses the live scroll until **Back to live**;
   - a cursor line gives each pen's value at its time; a pen's legend entry hides and shows it,
     and each pen has its own scale, automatic or fixed;
   - the visible range exports as CSV, and **Expand** shows the trend at full page size.
   - Decided on 4 October 2026 while the user was away (to be approved): the trend's top row
     holds, right-aligned, Back to live while paused, the range chips 5 min, 1 h, 8 h and 24 h
     (the one matching the window marked), and two icon chips, Export and Expand; where they do
     not fit, the range chips go first. A drag on the plot draws a box and zooms into its period
     on release; a tap places the cursor instead; the wheel zooms the time axis by a fifth at the
     pointer (10 s to 24 h); a drag along the row under the plot pans; each pauses unless the
     window reaches now. While the pointer is over the plot the cursor follows it and the legend
     reads each pen's last sample at the cursor's time ("?" before its first). A legend entry's
     click hides its pen, dimmed in the legend until clicked again. Export saves
     `<object id>-<yyyymmdd>-<hhmmss>.csv`: a `time` column in ISO 8601 UTC and a column per shown
     pen, each holding its last value. Expand draws the trend over the page at the design size,
     sharing its window, cursor and hidden pens, with a close button. Over a span longer than
     10 minutes the browser keeps one live sample a second.
   - **Pen scales (4 October 2026 completion):** a numeric pen may put `range = [min, max]`
     on its binding table. Both ends must be finite and the maximum greater than the minimum.
     The inspector offers Automatic or Fixed for each pen; Fixed edits the two ends together.
     A binding range takes precedence over the trend object's existing `min` and `max`.
     Without either fixed range, pens of one unit scale together to their visible samples, so a
     measurement and its setpoint are read against each other, and a pen of another or no unit
     scales to its own, each with the normal automatic margin (refined and approved by the user on
     7 October 2026: a PID trend drew its measurement and setpoint on top of each other). The
     value axis uses the first visible numeric pen's range and colour;
     hiding that pen makes the next visible pen own the axis. Limit bands use this same axis.
     Boolean pens keep their own digital lanes. The CSV continues to contain original values.
2. **Alarm list:**
   - columns for priority, time, tag, message, the value against its limit when the alarm was
     raised (for example "Level 2.83 m > 2.80 m"), and state; a column heading sorts by it, the
     default being priority then time;
   - a search box, and quick filters for priority, equipment and unacknowledged alarms;
   - **sound** on the display (EEMUA 191 practice): a High alarm sounds until it is acknowledged
     or silenced, a Medium alarm once, a Low alarm not at all; **Silence** stops the sound
     without acknowledging. The runtime says how each alarm sounds (`sound`: `continuous`, `once`
     or `none`); the display plays it, and keeps only which raises were heard and silenced;
   - User decision, 4 October 2026: a row's click opens its alarm details. **Show equipment**
     in those details opens the page holding the alarm's equipment, with that equipment
     highlighted for five seconds. Prefer its symbol or placed faceplate, using its instance
     or member bindings; search the current page first, then the other pages in navigation
     order, with hidden pages last. The action is
     disabled with an explanation if no accessible page shows the equipment or lookup fails.
     Looking up or navigating to equipment never acknowledges an alarm or writes a variable.
   - Decided on 4 October 2026 while the user was away (to be approved):
     - under the tabs a row of chips: Search, High, Medium, Low, Unacknowledged and Equipment,
       then Clear while a filter is set, and "3 of 9" at the end; the search keeps the rows whose
       message, tag or path holds the text in any case, and Search and Equipment ask in a dialog
       (the equipment of the open tab's alarms, by tag);
     - the column headings above the rows; the tag is the name of the alarm's equipment
       instance (§13.6 item 9), empty for an alarm a program holds directly; a narrow list leaves
       out the tag, then the time, then the state, and always keeps the priority and the message;
     - the default order stays §13.6 item 3's (unacknowledged first, then by priority, then
       newest first, which is priority then time within each); a heading sorts by its column in
       that column's own order (highest priority, newest, or from A), the same heading again
       reverses it, and the order stays when the tab changes;
     - the display sounds where its top bar shows the alarms: Silence appears beside the alarm
       counter while a High alarm sounds; a Medium alarm sounds once for each raise. A browser
       plays sound only after the page was first touched or clicked.
     - the value against its limit: the runtime reads a limit alarm's `Value`, `Limit` and
       `Direction` (from the first part of the instance that has them, so a block extending
       `LimitAlarm` has them too) at each `Hmi.Display` call and keeps the reading of the call
       that raised the alarm, the first call's for an alarm already active; a row carries it as
       `raised: { value, limit, high }`. The list shows it in a Value column, "91.5 > 90.0", the
       two numbers with the same decimals, at most three; the message column already names what
       is measured, and the unit follows item 4. The column is there only while a row of the tab
       has a reading, and a narrow list leaves it out first.
       For an equipment limit alarm, the raised reading also records its physical value's
       `source` path and the block's `unit` at that instant. Later unit changes must not relabel
       the historical reading. The source path selects the project's decimal default; without
       one, the existing shared precision of at most three places applies.
3. **Ranges on bars and gauges:** a quiet band for the normal range and ticks in the alarm
   colours at the limits, taken from the alarms of the bound block (for example an
   `AnalogInput`'s limits), or set on the object where no block gives them.
   - Manual limits use `limits = { low-low = 5, low = 10, high = 90, high-high = 95 }`.
     Every key is optional; present values must be finite and ordered from low-low to high-high.
     The normal band lies between low and high; a missing end uses the outer limit on that
     side, then the display end when neither limit exists.
     Low/high ticks use the Medium alarm colour; low-low/high-high use High. Limits outside the
     display range are not drawn. These marks describe thresholds and never raise or acknowledge
     an alarm; the program remains the alarm authority.
   - Live equipment metadata accompanies its numeric value in the same `Hmi.Display` snapshot.
     `Equipment.AnalogInput.Value` (including derived measurement blocks) takes its unit and
     engineering range from that instance, and limits/priorities from its four actual limit
     alarm instances. `Equipment.Tank.Level` takes its range from zero to `MaxLevel` and its
     four limit alarms. Percent, totals, raw inputs and unrelated scalars do not inherit those
     physical-value limits. The runtime never creates alarms from drawing metadata.
     A changed unit or limit is a value-stream change even when the numeric value is unchanged.
     Nonfinite or reversed equipment thresholds are omitted. Known equipment metadata owns
     its alarm marks; manual limits are the fallback for values without that contract.
4. **Number formats** (simplified with the user the same day: no table of settings): one rule
   gives every component the same decimals for a value; an object bound to a variable that
   another object of the project already shows takes that object's unit and decimals; a block
   that carries a unit, such as `AnalogInput`, gives it. An object may still set its own decimals
   and unit in the inspector, as today.
   - Inherited project formats are read-only presentation metadata; they never rewrite another
     object's authored settings. Unit and decimals are inherited independently. If several
     explicit settings exist, the first in file-name then object-id order supplies each default;
     an object's own setting still overrides it. Binding paths use IEC case-insensitive matching.
     Boolean equipment-state bindings do not acquire a numeric unit from their symbol.
     A trend pen's explicit binding unit takes precedence over its trend's shared unit when
     choosing that pen's default, matching the legend. The editor refreshes metadata from
     unsaved page/popup text; runtime page and popup answers carry the deployed metadata.
   - A loop controller compares one quantity: its measurement (`pv`), the value of the
     instrument it reads (`measures`) and its setpoint (`sp`) each take the unit and decimals
     they lack from the others, in that order, so `TIC101` reads `42.2 → 45.0 °C` like `TT101`
     beside it, and its panel and trend read the same. The controller shows the unit once.
5. **Controls and roles:** a control the signed-in user's role may not use is drawn dimmed with
   a lock, says which role may use it, and cannot be pressed.
   Dim its decorative surface, not its reading, label or role badge. Combining a signed-out
   state with a role lock must not compound opacity on text; disabled button text uses the
   theme's normal readable ink against its subdued surface.
   - The runtime's page, popup and faceplate answers give each control's lowest role as `roles`,
     by object id: the write policy's role for what it writes, raised by the control's own `role`
     (§12.8 item 4), or `null` where the policy does not let it write. Objects that write nothing
     are not listed.
   - The lock names the role, Operator or Engineer, or reads Locked where no role may use the
     control; a press says which role the control needs and sends nothing. Without a session
     every control stays read-only as before (§12.4).

## 6. Process graphics

1. Each symbol declares its ports with position and direction. The shipped symbols' design size, label
   place and ports are one table in the runtime (`crates/trust-runtime/src/hmi/symbol_geometry.json`):
   the language server's routes read it, and the editor's drawing reads the module generated from it
   (`npm run generate:symbol-geometry`), so that both meet the same ports. A project symbol declares
   its own in its file (§14.4).
2. A pipe connects two ports, or a port and a junction on another pipe. The file stores the
   connection, not the drawn line.
3. The route is computed: orthogonal, around objects, with few bends. It shall be recomputed
   when a connected object moves. Manual waypoints are stored only when the user bends a pipe.
4. Dropping an inline component (valve, flow meter) onto a pipe splits it into two pipes joined
   through the component.
5. Instruments attach to an equipment or a pipe; the attachment is stored as a reference.
   Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026: an instrument object has
   `attach = "<object id>"` or `attach = "pipe:<n>"`. The target is on the same page, is not the
   instrument itself, and is equipment that does not itself attach, or a pipe; each problem is
   reported at the `attach` line. The attachment is drawn under the objects as a thin orthogonal
   line from the instrument's bubble: straight onto the symbol's facing side or the pipe when the
   bubble stands over, under or beside it, else to the symbol's middle height and across into its
   facing side. In the editor the inspector's "Attached to" lists the page's equipment by tag
   and its pipes by their ends, and a selected instrument has a handle under its bubble to drag
   onto what it measures. When pipes are deleted or split, attachments follow their pipes; an
   attachment goes with its pipe or its equipment.

   **A loop controller's signal lines** (agreed with the user on 7 October 2026, when a controller
   stood alone beside its process): a loop controller shows its loop as on a P&ID, with two dashed
   signal lines routed as pipes are, around the other objects. `measures = "<object id>"` draws
   one from the instrument whose measurement it reads to the controller's bubble, and
   `acts_on = "<object id>"` one from the bubble to the equipment it acts on (its heater or
   valve). The lines follow the controller's configuration: Connected equipment in the inspector
   (the measurement, and the controlled equipment, the one that drives an input) sets both, as does
   picking them on the page, and selecting a controller whose lines differ from its configuration
   brings them in line; a line goes to the page object that shows the configured instance, and
   there is none when the page does not show it. So the picture never contradicts the PLC code
   Create PLC code writes from the same configuration. The checks report a line to an object that
   is not on the page or is the controller itself, signal lines on anything but a loop
   controller, a `measures` that names no instrument and an `acts_on` that names no equipment.
6. A pipe shows flow only from a binding that reports flow: a flow measurement, or a confirmed
   flow signal of the program. It never infers flow from a pump or valve state; a running pump
   can have no flow through a closed valve. Without such a binding the pipe is drawn without flow
   animation. An estimated flow is drawn as estimated (§5.3).
7. Line styles: process line and signal line.
8. **Layers:** pipes and instruments' lines lie under the objects, so a symbol covers their ends,
   and over layout panels, so a pipe stays visible where it crosses a panel used as an area of
   the plant, such as a room (refined on 7 October 2026, when a room panel hid the fan-coil's
   pipes).

## 7. Bindings

1. **Variable picker:**
   - a searchable tree of programs, function block instances and their members, with types and,
     when a source is connected, live values, supplied by the language server;
   - it opens from each binding in the inspector, and as the **Tags** tab of the bottom panel
     (§3.2);
   - for a binding, only variables of compatible type and direction are offered. The checks of
     item 6 decide, so the picker never offers a variable the checks would refuse;
   - a variable can be dragged onto a binding, or onto the canvas to create a component of the
     matching type:
     - a boolean creates a **Status**, a structure or an array a **Table**, and any other single
       value a **Value**;
     - the new object is placed where it is dropped (§3.3), bound to the variable and labelled
       with the last part of its path, for example `Running` or `Pumps[2]`;
     - program and function block instances create nothing (their equipment symbols are §8),
       and the editor says to drag one of their members;
   - a variable dropped on a binding it cannot have leaves the binding unchanged and says why;
   - the tree refreshes from edited declarations, including unsaved source and pending design
     instances. Unrelated executable-code errors do not prevent drawing or selecting equipment.
     No compilation or PLC execution is part of loading the picker.
2. **Paths:**
   - bindings use runtime instance paths, for example `FeedArea.P101.Running`;
   - paths reach members of function block instances, structure fields and array elements
     (`FeedArea.Pumps[2].Running`);
   - global variables are not required.
3. **Runtime support (phase 1 foundation):**
   - the runtime shall expose nested members (function block instance members, structure
     fields, array elements) as readable points with their type;
   - it shall expose them as writable points where the runtime write policy allows (§12.2);
   - the resolver for these member paths comes with its own tests.
4. **Direction:** each binding is `read`, `write` or `read-write`. A control binding that writes
   shall name a writable target.
5. **Units:** a binding may declare its engineering unit, shown with the value. Scaling raw
   values to engineering units is the program's work, for example in the Plant library's
   `FB_AnalogInput` (specification `trust-plant-library` §7.1); the HMI shows and writes engineering values as the program
   holds them (decided with the user on 3 October 2026, when a linear scaling in the HMI was
   dropped from Phase 5).
6. **Checks:** each of the following shall be an error on the canvas and in the Problems view:
   - a component type the catalogue does not define (§5.6);
   - a binding to a property the component does not have;
   - a path that does not resolve in the program;
   - a variable whose kind of value the property does not accept;
   - a direction the property does not allow;
   - a binding that writes to a target that cannot be written: a constant or a function block
     output.

   Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026: the editor checks every page
   and popup file of a project when it opens or changes and when a source file of the project is
   saved; each problem is an error at its binding's line, and a file that does not load shows its
   errors. On the canvas an object with a problem is outlined in the alarm colour with "!", and
   its problems show when the pointer rests on it; the inspector lists the selected object's
   problems under Problems, each with its fix where the editor has one (refined on 7 October
   2026). Continuous checks use declarations and component definitions, without constructing or
   executing a runtime. Explicit Build and deployment retain full program checks.
7. **Edited and deployed programs:**
   - the editor checks bindings against the edited program;
   - Deploy checks them against the running program (§16.2);
   - a binding that is valid in the edited program but missing in the running program is shown as
     "not in running program", in the editor's Live mode and in the runtime.
8. **Rename:** renaming a variable or instance in the program shall update the bindings in the
   HMI files, and Find All References shall include them. After a rename the editor shall list
   any binding it could not update.
   Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026:
   - the language server's rename edits, beside the program, every binding of the project's
     page and popup files whose path names what is renamed: a member of a program, function
     block or class, matched by the type that declares it (renaming `Running` of `Pump` changes
     `Main.P101.Running` when `P101` is a `Pump`, also when `Pump` inherits it); a structure
     field, by its structure type; a global variable or a configured program instance, as the
     first part of a path; and a program, when its instance carries its name. The instance an
     equipment object shows (`instance = "Main.P101"`, §21.1) follows in the same way, and so does
     the object's displayed `tag` while it is the instance's own name (`P101`, not `P-101`);
     Find All References lists the instances, not the tags (refined on 7 October 2026, when a
     renamed pump left its object pointing at the old instance). A program
     instance that a configuration names (`PROGRAM Tank WITH Fast : TankLevelControl`) is of its
     program's type, as a function block instance is of its function block's type (IEC 61131-3,
     §6.6.4 and §6.8, Table 62 feature 6a), so renaming `Level` of `TankLevelControl` changes
     `Tank.Level`;
   - only the renamed part of a path changes; the rest of the file keeps every byte;
   - Find All References lists those bindings, each at its line;
   - a file that does not load, or a binding whose path does not resolve before the rename, is
     not changed. The binding checks list every binding that does not resolve in the Problems
     view and on the canvas (item 6), so a binding the rename could not update is listed there.
9. **Tags of other runtimes** (planned with the user on 3 October 2026, Phase 5): the variable
   picker also lists the variables that the project's other runtimes publish on the runtime mesh
   (`[runtime.mesh] publish`), grouped under each runtime's name, and one is picked like a
   variable of the program, as `<runtime>:<path>`, for example `PLC2:Main.Tank.Level`:
   - the editor checks such a binding against what that runtime publishes, with its type;
     the typed publish catalogue lists existing configured paths only, preserving local
     constant/output write restrictions. Publishing a container does not advertise its
     unlisted members as separate tags; each separately bound remote path must be published.
     Existing mesh catalogue entries carry additive `variable` metadata (the variable-tree
     shape). Entries without that metadata are not offered as typed HMI tags; metadata is read
     from the running resource when queried, and an unavailable resource returns a query error.
   - the runtime serving the page subscribes to it over the mesh by itself, with no setting in
     either `runtime.toml` beyond the publish list the other runtime already has;
   - while that runtime is unreachable or stops publishing the variable, the value is stale with
     its age, then unknown;
   - Remote operation uses the existing mesh authentication configuration. A configured token
     must be enforced by peer authentication; requesting mesh TLS requires the runtime's
     certificate material and an actual TLS transport, never a plaintext fallback.
   - **writes** (the user, the same day: "we need write also"): a control bound to such a variable
     sends its command over the authenticated mesh to the runtime that owns the variable, with
     the signed-in user's name and role. That runtime decides by its own write policy, the one
     its own HMI uses (§12.8); nothing is set up in either runtime beyond it. The command follows
     §12 as a local one does: applied, confirmed by the published value, refused or timed out,
     never sent twice. While that runtime is unreachable the control refuses at once, saying so,
     and nothing is queued. Both runtimes record the command in their audit logs, the serving
     one who pressed it, the owning one what was applied and from where.

## 8. Plant components

### 8.1 Supplied components

The [Plant library](trust-plant-library.md) ships, in `libraries/plant/hmi/`, one component
definition (`pairs/<name>.toml`) per block and one panel (`panels/<name>.toml`) per family:
run (motor, pump, fan, blower, agitator, conveyor), drive (variable-speed motor, pump and fan,
dosing pump), heater, reversing motor, two-speed motor, valve (on/off valve, damper), motor
valve, cylinder, position (control valve, modulating damper), measurement (analog, level,
pressure, temperature and analyzer inputs), flow, digital (digital input, level and pressure
switches), totaliser, counter, weigh scale, tank, PID loop, cascade, on/off controller,
conditions (interlocks, permissives, protection), group (duty/standby, lead/lag, staged),
alarm manager, sequence and sequence manager. The Comm library ships the `link` panel
(specification 42, §5). Customers use a definition as it is, copy and adapt it in their project,
or write definitions for their own function blocks through the same files.

### 8.2 Behaviour

The blocks' behaviour is owned by the [Plant library specification](trust-plant-library.md):
modes and authority (§3), common device behaviour (§4), each block (§6–§9) and settings (§10).

### 8.3 The PID loop

1. The PID panel operates a loop written as ordinary code around the real `Control.PID`
   (§10.5, Plant §8.1): initialization, tracking, bumpless transfer and whole-record parameter
   updates (specification 45) are the code's, run through the PID's own methods.
2. Operators use, in the runtime browser: the Manual/Auto switch, the setpoint, the manual output
   (in Manual) and the PV/SP/MV trend. The panel writes the loop's variables through the write
   rules Create PLC code adds (§12.2).
3. Tuning is in the Engineer-only Settings dialog, on the loop's parameter record, the same record
   Control Studio tunes. **Open in Control Studio** is an engineering action, available only in
   VS Code (§8.8).

### 8.4 Operator panel pattern

Agreed with the user on 7 October 2026. Every supplied equipment panel opens as a popup:

1. **Title:** the instance's tag (`P101` of `Main.P101`, or the object's tag), with the
   component's title after it ("P101 — Pump").
2. **State:** the state words (Running, Stopped, Open, Fault, …), and one message line under the
   commands: the device's `ReasonText` in plain words (why it is blocked, else why a command
   would be refused). The line is tucked into the commands' gap, so when it is empty the panel
   shows a section break between commands and values, not an empty row.
3. **Mode:** an Auto/Manual selector for Operators. Out of service and Return to service are
   Engineer buttons in the maintenance row, each lit only while it can act (out of service while
   in service, and back).
4. **Commands:** Start/Stop (Open/Close, Extend/Retract, …) are enabled in Manual. In Auto they
   are greyed with "Auto: switch to Manual" and out of service with "Out of service" — the
   device's `CommandText` through each button's `interlock` permit (§12.6). A start or open is
   also greyed, with the reason, while the device is blocked. Stop uses the `stop` button
   variant, visibly distinct from Start. Reset is enabled while the device has a fault.
5. **Values and counters** follow without empty bands: the panel's size is its content.
6. **Settings:** a header button for Engineers only; Viewers and Operators do not see it (§21.4).
7. There is no "take control", owner or request number anywhere on an operator screen.

### 8.5 Component definition

A component definition `pairs/<name>.toml` (`format = 1`) names:

| Key | Meaning |
|---|---|
| `title` | The palette name ("Pump"). |
| `block` | The function block type with its namespace (`Plant.FB_Pump`). Create HMI also offers it for blocks derived from it unless a closer definition exists. |
| `faceplate` | The panel it opens. |
| `symbol`, `variants` | The symbol type and the variants it may take. |
| `[fields]` | Panel fields whose member has another name (`Output = "HeaterOn"`). |
| `[bind]`, `[quality]` | Symbol properties and their members, and the quality member of each. |
| `[[settings]]` | `member`, `title`, `unit`, `min`, `max`, `step` or `choices`, `help`: the settings metadata (Plant §10), read by the inspector, the Settings dialog and the runtime's validation. |
| `[[io]]` | Hardware terminals: `member`, `title`, `direction` (`input`/`output`), `required` (default TRUE) or `required_when` (a BOOL input such as `HasFeedback`). A `required_when` terminal is needed while its member is TRUE, taking the block's default when the instance does not set it; connecting it sets the member TRUE in the generated declaration unless the design sets it (refined on 7 October 2026, when a connected valve feedback was ignored). |
| `[[dependencies]]` | References to other instances: `member`, `title`, `interface` (`Plant.IMeasurement`), `required`. A member may repeat, one entry per interface it can drive (a controller's `Output` drives `AutoPosition`, `AutoPower` or `AutoSpeed`); the chosen instance's interface selects the entry (refined on 7 October 2026). |
| `[program]` | `position` (`measurements`, `controllers`, `actuators`: call order), `initial` (instance initial values), `cycle` (task period range, ms). |

One parser in `trust-runtime` reads definitions for the runtime, the language server and
`hmi check`.

**Members beside the instance** (decided with the user on 7 October 2026). A member written
`{instance}_…` names a variable beside the instance, named after it: `{instance}_Setpoint` of
`Main.TIC101` is `Main.TIC101_Setpoint`, and `{instance}_Parameters.ProportionalGain` is a field
of `Main.TIC101_Parameters`. Every key above that names a member may use it: fields, bindings,
quality, settings, dependencies, `drives` and initial values; `plant.toml` keys follow. The link
check (§8.6) checks such a member where an instance is bound, against the variables the program
declares, not against the block. The Plant library's PID controller (`block = "Control.PID"`)
uses it for a loop written as ordinary code (§10.5).

### 8.6 Link check

A field whose member the block does not have, or whose member holds a kind of value the field
does not accept, is an error at the field's line in the definition and at the block's name in
its ST file; a definition whose block type does not exist is an error at its `block` line; a
setting whose member is not a public variable of the block, or whose limits do not fit its type,
is an error at that setting's line.

### 8.7 Panel files

1. **A panel file** `panels/<name>.toml` is a popup file (§4.9) with an optional `description`,
   `[fields.<Name>]` with `accepts`, and objects that bind fields as `"$<Name>"`. A panel may
   also declare `settings = true` (it offers the Settings button for its instance) and
   `[panel]` with `scope`, `pages`, `dock` and `order` when it is a sidebar docked on pages
   (§21.1).
2. **Where they live:** a project's own in `hmi/panels/` and `hmi/pairs/`; a library's in
   `<library>/hmi/panels/` and `<library>/hmi/pairs/`. A project file of the same name replaces
   a library's, so a copy can be adapted. The HMI bundle (§16.4) holds the project's files and
   its libraries' panels, definitions and symbols (§14.4 item 1) under the same paths.
3. **Opening:** an object with `instance` and `faceplate` opens that panel for that instance
   when clicked, unless it has its own `[object.click]`; `[object.click] faceplate` with
   `instance` does the same for any object. The panel's fields become the instance's members
   through the definition whose block is the instance's type, else the
   panel's only definition, else the fields' own names.
4. **In the runtime display** a panel opens as a popup named by the object that opens it,
   `faceplate:<page>/<object id>` (or `faceplate:popup:<name>/<object id>`); the runtime
   resolves panel, instance and members from that object, so a panel opens only for what the
   project's pages open it for.
5. **In the editor** a panel opens in the HMI Builder as a popup does, with design samples for
   its fields. The Problems view shows problems of panels and definitions at their lines.
6. **Header, marks and units** keep the rules decided on 3 October 2026: the opening object's
   tag titles the popup; its symbol takes the opening object's type, variant and tag; an object
   bound to a variable the opening object shows takes its unit and decimals when it has none. A
   trend's pens may show values of different units, so a trend takes the unit pen by pen: only a
   pen bound to a variable the opening object shows takes its unit.

### 8.8 Open in Control Studio

**Open in Control Studio** is an editor inspector action on placed PID panels. It opens the
existing validated project loop for that equipment and the selected runtime: the loop whose
controller is the panel's `Control.PID` instance (or a block inside it); several matching
controllers need a chooser; missing or invalid configurations explain the blocker. It never
changes the selected target or invents signal bindings.

### 8.9 Panels on a page

A panel may stand on a page bound to an instance: one object of type `faceplate` with
`faceplate = "<name>"` and `instance = "<path>"`, under a slim header with the instance's tag.
Any panel whose fields the instance's block has may be placed (§8.6 decides the fit; the
language server lists fitting instances). It takes the panel's size plus the 44 px header and
scales proportionally. A panel the project and its libraries lack, an instance that is not a
function block instance, or a missing field is an error at its line in the editor,
`trust-runtime hmi check` and deployment checks.

## 9. Program to HMI

**Create HMI** is an explicit editor and command-palette action.

1. It reads the PLC declarations through the language server's existing index and matches every
   function block instance declared in a program to the nearest installed component definition
   (Plant §12.1), including customer definitions.
2. It adds one equipment object per unshown instance to an overview page: the definition's
   symbol, `block`, `instance` and `faceplate`, arranged in rows by `program.position`
   (measurements, controllers, actuators), using the page writer of §15, so the file is the
   canonical format every reader accepts.
3. Existing views and hand-arranged pages are untouched; a repeat without new instances creates
   nothing. Unsupported blocks are listed by name. It does not change PLC source, compile, run,
   deploy or infer pipes and process connections.
4. It answers within 2 s for a project of 200 instances on the builder's reference machine:
   it uses the already-built declaration index and never compiles the project or runs a scan.

## 10. HMI to program

### 10.1 Explicit action

Draw and configure equipment first. **Create PLC code** reads all project pages, including
unsaved edits, and the project's `plant.toml` (§10.4). Drawing, selection, configuration and
save never trigger it.

### 10.2 What it writes

For each equipment instance shown on any page and not yet declared, it writes:
1. **one declaration** in the instance's program, inside the region marked
   `(* truST plant: declarations sha256:<hex> *)` … `(* end of truST plant declarations *)`
   in a `VAR` block of its own, with the instance initialiser holding the settings chosen in the
   inspector and the definition's `[program] initial` values that differ from the block's
   defaults: `P101 : Plant.FB_Pump := (StartTimeout := T#10s);`;
2. **one call** inside `(* truST plant: calls sha256:<hex> *)` … `(* end of truST plant calls *)`
   at the end of the program body, in measurements → controllers → actuators order (an instance
   taken as an input is called first, one whose output drives another before it), with its
   connected I/O and dependencies: `P101(RunningFeedback := PumpRunning, RunCommand => PumpRun);`
   and, for a dependency with `drives`, the assignment after the call
   (`FV101.AutoPosition := LIC101.Output;`).

Nothing else for a block: no helper variables, initialisation blocks, registration, identities or
"ready" flags. The program sets `Auto…` inputs in its own code (`P101.AutoRun := …;`). A PID loop
is not a block call but the ordinary code of §10.5.

### 10.3 Ownership and transactions

1. The regions are owned by the generator. Their begin marker carries the SHA-256 of the
   region's generated text. A region whose text no longer has that checksum is accepted only
   when it is exactly what the generator writes for the instances it declares, as after
   renaming one of them; otherwise it was edited by hand and is refused with its location,
   never overwritten. Calls of one kind follow in name order, so a rename can move a call: the
   calls region compares statement by statement, in any order, and is written again in the
   generator's order (refined on 7 October 2026, when renaming `FanProtection` to `Heating`
   was refused). Instances declared by hand outside the regions are reused, never
   redeclared. A change in the inspector of an instance the regions declare writes them again
   in the same edit, so the declaration stays the truth.
2. The change is one undoable WorkspaceEdit; repeating it is idempotent; deleting a view never
   deletes PLC code; a source change during preparation invalidates the proposal and keeps the
   user's work. The files it changes or creates are saved, as are those of an equipment change
   in the inspector or in Devices & Connections, except a file with unsaved changes of the
   user's own, which stays unsaved for the user to review (refined on 7 October 2026: a change
   left unsaved in `plant.toml`, which has no editor, was lost when VS Code closed).
3. Missing required hardware terminals and dependencies are listed with the equipment and page;
   generation never invents addresses, process logic or task periods. A project without a task
   asks for its control cycle.

### 10.5 PID loops as ordinary code

Decided with the user on 7 October 2026: a PID loop is ordinary ST around the Control library's
real `Control.PID` (specification 45), run through `Configure`, `Initialize` and `Update` as the
control course runs it, with no block around it, so Control Studio tunes it as it tunes every
course loop (Plant §8.1). For a controller `Main.TIC101` (`block = "Control.PID"`) measuring
`Main.TT101` and driving `Main.EH101`, Create PLC code writes:

1. **in the declarations region** `TIC101 : Control.PID;` and the variables rebuilt after a
   restart, `TIC101_Input : Control.PIDInput;`, `TIC101_Start : Control.PIDInitialState;` and
   `TIC101_Initialized : BOOL;`;
2. **in a retained declarations region** `(* truST plant: retained declarations sha256:<hex> *)`
   … `(* end of truST plant retained declarations *)`, inside a `VAR RETAIN` block after the
   declarations' block: `TIC101_Parameters : Control.PIDParameters := (…)`, the one parameter
   record (the definition's initial values, then the design's settings
   `{instance}_Parameters.<field>`), `TIC101_Manual : BOOL := TRUE;` (the operator's mode),
   `TIC101_Setpoint`, `TIC101_ManualOutput` and `TIC101_Output`;
3. **in the calls region**, in the controllers' place: configure the PID from
   `TIC101_Parameters`; pass the setpoint, the manual output and the cycle time (the period of
   the program's task, else the cycle chosen for a new task); while `TT101.GetQuality()` is Good
   take `TT101.GetValue()` as the measurement, initialise the PID from it and from the present
   output when it has not started, and select Manual or Auto by `TIC101_Manual`; otherwise hold
   the output in Manual and clear `TIC101_Initialized`, so the loop starts again without a bump
   from the next good measurement; update the PID into `TIC101_Output`; in Auto let the manual
   output follow the output; then, as for any dependency with `drives`,
   `EH101.AutoPower := TIC101_Output;`;
4. **a cascade** when the controller's output is connected to another controller: the drives
   assignment is that loop's setpoint, `TIC201_Setpoint := TIC200_Output;`, and while the inner
   loop is in Manual the outer loop tracks its measurement (`PIDMode#Track`), so closing the
   cascade is bumpless;
5. **the `USING Control;` and `USING Plant;` directives** the code's enumeration values need,
   beside the file's own `USING` directives before the program (else just before it), when the
   program does not see those namespaces yet: the only lines Create PLC code writes outside its
   regions;
6. **the write rules** the loop's panel needs (§12.2): `Main.TIC101_Manual`, `Main.TIC101_Setpoint`
   within the measurement's `RangeLow`…`RangeHigh` and `Main.TIC101_ManualOutput` within the
   output limits, for the operator, added to `runtime.toml` unless it has them.

The loop's variables belong to it: they are never planned as instances of their own, and the
regions' ownership (§10.3) covers the retained region too.

### 10.4 The project's equipment design (`plant.toml`)

`plant.toml` (format 2) holds the design choices shared by all views of an instance and not yet
in PLC code:

```toml
format = 2

[equipment."Main.P101"]
block = "Plant.FB_Pump"
settings = { StartTimeout = "T#10s" }
signals = { RunCommand = "Main.PumpRun", RunningFeedback = "Main.PumpRunning" }

[equipment."Main.T101"]
block = "Plant.FB_Tank"
dependencies = { LevelInstrument = "Main.LT101" }
```

It holds no generated code, no numeric identities and no copy of the PLC source. Hardware
signals are chosen in Devices & Connections (§21.3); settings and dependencies in the
inspector. Once an instance is declared, its declaration is the truth: the inspector shows the
declaration's real values.

## 11. Live mode

1. **Design / Live** switches the canvas between editing and running the page.
2. Sources: the project's simulator and a connected runtime or PLC. Live mode always shows the
   real run target (decided with the user on 7 October 2026: the earlier test values and
   recordings are removed; design samples cover a page without a target, item 6 says when none
   runs).
3. In Design mode objects may show current values from a connected source.
4. Commands in Live mode follow §12 exactly as in the runtime.
5. When the connection is lost:
   - values turn stale (§5.3);
   - pending commands turn unknown (§12.5);
   - no command is resent after reconnection.
6. Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026:
   - **Source:** the run target the extension has selected, the simulator or a runtime, reached
     through its control endpoint. Live mode reads the page's bound paths with the control request
     `hmi.read` (`{ "paths": […] }`, Viewer role, at most 1 000 paths) every 250 ms; the answer has
     the stream's cycle count, values and missing paths (§11.1), and values turn stale as there.
   - **Commands (phase 1):** Live mode observes. Controls show their values but send nothing, and
     the editor says to operate in the runtime display. Item 4 needs the HMI session of a named
     user (§12.8, item 2), which the control endpoint does not carry; it applies once the editor
     can open such a session on the runtime.
   - **Phase 5 commands (user decision, 4 October 2026):** the editor opens a named HMI session
     on the selected run target. Commands use the runtime display's authorization, confirmation,
     outcomes and audit path. Login secrets and sessions are not written to project files.
     Leaving the target, expiry or disconnect disables
     operating controls; reconnect never resends a command. The shared display has an abortable
     lifetime: ending it closes its stream, stops delayed redraws/reconnects, removes its global
     input handlers and prevents late requests or replies from reviving the old display.
     The extension host retains the named session cookie only in memory for this editor and
     target. Only HMI read, login, logout, command and alarm endpoints are bridged; arbitrary
     addresses, redirects, deployment and control endpoints are refused. The runtime's access
     token supplies transport access, never the named operator's identity. Each request verifies
     the selected target. Target changes invalidate the session before another command can go
     out. An in-flight response from an ended connection cannot restore its cookie.
     Operating uses the deployed bundle, with its normal confirmations. The project must be
     saved and match that bundle before opening the operating view. A simulator or target with
     no HMI web endpoint remains read-only with an explanation. In the operating view, design
     zoom/translation preview and editing panels are hidden; Design remains available above the
     runtime display.
   - Without a running target, Live mode says so and every value is unknown.
### 11.1 Runtime display

Milestone 1A shows pages read-only in the browser; commands follow in 1B (§12) and bundles in
1C (§16).

1. **Pages.** When the project has `hmi/screen.toml`, the runtime serves its pages at `/hmi`:
   each page at `/hmi/<page>`, where `<page>` is the page's file name under `hmi/pages/` without
   `.toml`, and `/hmi` shows the first page by name. Without `hmi/screen.toml`, `/hmi` says that
   the project has no HMI. The runtime reads the files when a page is opened, so a saved change
   shows on the next load; a file that does not load shows its errors (file, line and message)
   instead of the page. The files are the active bundle's (§16.4).
2. **Live values.** A page subscribes to the paths its objects and pipes bind, over a websocket. The
   runtime first sends the expected update period, the larger of its scan period and 250 ms; then,
   after each call of `Hmi.Display` (§18 phase 5), at most four times a second, the runtime's
   cycle count with the values that changed since the page's last update: every subscribed
   path's value in the first update, then only those that changed, and none when nothing changed,
   so the cycle count alone shows that the program runs. When the calls come less often, it sends their
   measured period as the new expected update period. A page that has just opened gets its first
   values at the next call:
   - a path that does not resolve in the running program, holds a reference, or has more than
     20 000 elements is unknown; the paths that do not resolve are also listed (`missing`) in the
     first update and whenever that list changes, and their objects are marked "Not in running
     program" (§16.3);
   - an array carries the declared lower bound of each dimension; a structure or function block
     instance carries its members by name, in their declared order; a real that is not finite, and an integer too large
     for a JSON number, are sent as text;
   - time and date values are sent as IEC literals (for example `T#1s500ms`), and an
     enumerated value by its name;
   - a value located at an input of a link that exchanges no data carries `staleMs`, the time
     since that link's last data, and shows as stale while the stream itself is fresh
     (specification 42, §5 item 4);
   - when the cycle count has not advanced, or no message has arrived, for three expected update
     periods, every value is stale and shows its age (§5.3);
   - **redraw** (decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026): after the first
     drawing, an update changes the drawn page in place, only where its new drawing differs from
     the last: a new value sets a text, and an unchanged object, a turning symbol included, keeps
     its nodes and its animation.
3. **Trends** draw the samples received since the page was opened; the runtime keeps trend
   history from §13 on.
4. **Access:** viewing pages and subscribing to values is a read of the running program and
   needs the Viewer role.

## 12. Authorization and command handling

### 12.1 Designing and operating are separate

Editing HMI files in VS Code grants no right to operate equipment. Adding a control to a page
changes nothing at runtime until the page is deployed (§16), and deployment is itself an
authorized change.

### 12.2 Runtime policy and page permissions

1. **Runtime write policy:**
   - the runtime holds the write policy: which points may ever be written, by which roles, and
     within which limits;
   - it is deny-by-default and part of the runtime configuration;
   - it changes only through an authorized deployment.
2. **Page permissions:** each control declares its roles, confirmation and limits.
   - Page permissions can only narrow the runtime policy, never widen it.
   - The effective permission is the intersection of both.
3. **Controls outside the policy:** a control that needs more than the policy allows is an error
   in the editor and is refused at deployment.
   Refined on 7 October 2026, when a created settings page was refused by its own project: the
   editor offers the rule a control needs. **Allow writing**, beside the problem in the inspector
   and as the quick fix of the problem in the page file, appends to the project's `runtime.toml`
   a `[[runtime.web.hmi.write]]` rule for the point with the control's role, Engineer when it names
   none, and its `min` and `max` when it has them; several controls of the page writing the point
   give the lowest role they ask and the widest limits. Create HMI Settings Page adds the rules of
   the controls it creates, and Create PLC code those of the PID loops it writes (§10.5 item 6).
   A point `runtime.toml` already names is left as it is, a
   `runtime.toml` with unsaved changes is left unsaved, and saving `runtime.toml` checks the
   project's pages again. The policy stays explicit: each rule is in the file, deployed only
   through an authorized deployment.
4. **Every request checked:** the runtime authorizes every command against the policy, the
   active HMI bundle, the session's role and the limits.

### 12.3 Roles

1. **Viewer**: reads values.
2. **Operator**: sends the commands that the policy and the pages permit to Operators.
3. **Engineer**: additionally sends engineering commands (for example tuning parameters).
4. **Deploy permission**: deploying an HMI bundle requires a deploy permission granted to
   Engineers by default.

Today every HMI write requires the Engineer role. Allowing Operator commands is a deliberate
backend change, delivered with its own tests in milestone 1B.

### 12.4 Sessions

1. The runtime HMI requires login when any control exists. The user and role are shown in the
   top bar.
2. Sessions expire after an idle time and after an absolute time, both configured in the runtime.
3. On expiry the page stays visible read-only, the controls are disabled at once, and a login
   prompt appears. Pending commands are not resent.

### 12.5 Command lifecycle

1. **Identity:** every command carries a unique command id and the version of the HMI bundle
   that produced it.
2. **Outcomes,** shown on the control and in the event log:
   - **pending**: sent, not yet decided;
   - **refused**: by permission, policy, limits, interlock or an out-of-date bundle, with the
     reason;
   - **applied**: the runtime wrote it to the program in a scan cycle;
   - **confirmed**: the feedback named by the control (for example Running becomes TRUE) reached
     the expected state within the control's confirmation time;
   - **timed out**: not applied, or not confirmed, within its time limit;
   - **unknown**: the connection was lost before the outcome was known; never assumed successful.
3. **Duplicates:** a repeated request with the same command id is executed at most once.
4. **No replay:** after a reconnection, commands are never resent automatically.
5. **Readback:** a write-value command shows the value read back from the program, not the value
   that was requested.

### 12.6 Pulses and interlocks

1. **Pulses:** a pulse command is ended by the runtime or the program after its duration, even if
   the browser disappears.
2. **Interlocks:** interlocks are enforced in the PLC program. The HMI shows interlock state and
   reasons, and never relies on the browser to enforce them. A command blocked by an interlock is
   reported as refused, with the reason.

### 12.7 Audit log

Every command shall be logged with:
- time;
- user and session;
- command id;
- target point;
- old value and requested value;
- outcome;
- HMI bundle version.

### 12.8 Configuration and requests

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026.

1. **Users, sessions and policy** are runtime configuration in `runtime.toml`. They change only
   through an authorized deployment:

   ```toml
   [runtime.web.hmi]
   idle_minutes = 15      # a session ends after this long without a command or interaction
   session_hours = 12     # and after this long in any case

   [[runtime.web.hmi.user]]
   name = "anna"
   role = "operator"      # viewer, operator or engineer
   password = "pbkdf2-sha256:600000:<salt>:<hash>"   # from `trust-runtime hmi-password`

   [[runtime.web.hmi.write]]
   path = "Loop.Setpoint" # a runtime instance path; "Loop.Tuning.*" covers every member below it
   role = "operator"      # the lowest role that may write it
   min = 0.0              # limits for numbers, optional
   max = 120.0
   ```

   - Roles are ordered: an Engineer may do what an Operator may.
   - A password is stored only as a PBKDF2-HMAC-SHA256 hash with its iteration count and salt.
   - A path no `write` entry covers cannot be written.
   - **In the editor** (refined on 7 October 2026, when a new project's display had nobody who
     could log in): Screen settings list the users with their roles. **Add user…** (also
     **HMI: Add User** in the command palette) asks the name, which no other user may have
     whatever its case, the role, and a password of at least 8 characters, and appends the user
     with the hash `trust-runtime hmi-password` makes; the password itself is never written.
     **Remove** asks first and deletes the user's table. The rest of `runtime.toml` stays as it
     is, and the file is saved unless it has unsaved changes of the user's own. Write rules are
     added from the controls that need them (§12.2 item 3).
2. **Reading and commanding:**
   - pages, values and the login request pass the runtime's web authentication with the Viewer
     role, as today;
   - every command also needs an HMI session of a named user, so the audit log names a person.
     This holds even where the web authentication grants more, for example a local browser.
3. **Login:**
   - `POST /api/hmi/login` with `{ "user", "password" }` starts a session. It returns the user,
     the role and both expiry times, and sets an `HttpOnly`, `SameSite=Strict` session cookie;
   - a wrong user and a wrong password give the same answer;
   - after five failed attempts a user is refused for a minute;
   - `POST /api/hmi/logout` ends the session, and `GET /api/hmi/session` reports it, or
     `"session": null` when there is none;
   - the value stream reports the session's end within one period.
4. **Controls** declare in the page file:
   - `role`: the lowest role, which can only raise the policy's;
   - `confirm`: a question asked before sending;
   - `min` and `max`: entry limits, which can only narrow the policy's;
   - for buttons `action` (`set`, `reset` or `toggle`), for pulse buttons `pulse` (seconds);
   - for selectors and mode selectors `options`, the choices offered, and for a switch its two
     states' words (§5.8);
   - `expect` (the feedback's expected value) and `timeout` (seconds, default 5) for
     confirmation;
   - the bindable properties `feedback` (what confirms the command), `interlock` (a BOOL
     permissive) and `interlock-reason` (its text). The binding checks check them like every
     other binding.
5. **Commands:** `POST /api/hmi/command` with
   `{ "id", "bundle", "page", "object", "action", "value" }`.
   - The runtime finds the target from the active page file, never from the request.
   - It decides refused, applied (with the value read back) or timed out at the program's next
     call of `Hmi.Display` (§18 phase 5) and answers with that outcome; a command the program has
     not taken within the time limit is withdrawn, so it is never applied later.
   - Confirmation by the feedback, a pulse's end and every later outcome reach the browser on the
     value stream as `{ "command": { "id", "outcome", "readback"?, "reason"? } }`.
   - A command id is remembered for ten minutes; a repeated id gets the first answer without a
     second write.
   - `action` is `write` with a `value` (setpoint entry, numeric input, slider, switch), the
     button's `action` (`set`, `reset` or `toggle`), or `pulse` (pulse button).
   - The target is the control's writing binding. A command needs a session, a permission that
     covers it (§12.2), a value within the limits that fits the target's type, and a permissive
     `interlock`. Otherwise it is refused with the reason.
   - Confirmation: by `feedback` reaching `expect` (for buttons TRUE after `set` and `pulse`,
     FALSE after `reset`). For a written value without `feedback`, the target reads back the
     requested value in a later scan. Either must happen within `timeout`, or the command times
     out with the value read back.
   - `bundle` is the version of the HMI files the page was drawn from, given with the page data.
     A command from another version is refused: "the page is out of date; reload it" (1C makes
     this the deployed bundle's version).
6. **Audit:** each command's record (§12.7) goes to the runtime's audit log, the same log as the
   control protocol's audit records.

## 13. Alarms and events

Alarm management follows ISA-18.2 (IEC 62682) and EEMUA 191: an alarm is a condition that needs an
operator to act; anything that needs no action is an event.

### 13.1 Alarms are function blocks

1. **One way to define an alarm:** every alarm is an instance of a shipped alarm function block in
   the program. The HMI files hold no alarm definitions. When an engineer adds an alarm to a
   variable in the builder, the builder adds the instance to the program (§10).
2. **The `Alarm` library,** shipped like `Control`, is object-oriented where it pays:
   - a **base block** holds the lifecycle (§13.2), the alarm's message and priority, and writes
     its records (§13.4);
   - **detection blocks extend the base:** digital (a condition is TRUE), limit (a value above or
     below a limit, with deadband; high-high and high are two instances with their own
     priorities), deviation (two values differ by more than a limit) and discrepancy (a command
     and its feedback disagree for longer than a time, for example failed to start or to open);
   - **own detection:** a customer block that extends the base gets the whole lifecycle, the
     records and the display;
   - normal use stays plain: the program calls the block every cycle with its inputs and reads its
     outputs. Methods serve one-off program actions: `Acknowledge` for a local push button and
     `Reset` for a latched alarm.
3. **Detection** runs in the scan cycle, with on- and off-delays and, for limits, a deadband.
4. **Program use:** the outputs (active, unacknowledged) drive horns, beacons and interlocks without
   any HMI.
5. **Equipment** blocks hold their alarms as members, for example a pump's failed-to-start alarm,
   so an object shows the alarms of its equipment (§13.6).
6. The block interfaces are defined in a separate alarm library specification, approved before
   implementation (as §8.2 does for equipment).

### 13.2 Lifecycle

1. **States (ISA-18.2):** normal; active and unacknowledged; active and acknowledged; cleared but
   unacknowledged. An alarm that clears before it is acknowledged stays listed until it is
   acknowledged.
2. **Shelved:** an operator takes an alarm out of the list for a set time; it returns by itself
   when the time ends.
3. **Suppressed by design:** the program states that an alarm does not apply, for example while
   its unit is stopped.
4. **Out of service:** an alarm removed for maintenance until it is put back.
5. **Latched:** an alarm configured to latch stays active until it is reset, even when its
   condition clears.
6. **First out:** of the alarms in one first-out group, the one that became active first is marked
   first out until the group is reset.
7. The state lives in the block instance; with `RETAIN` it survives a warm restart.

### 13.3 Priorities

1. Three priorities, **High, Medium and Low**, as ISA-18.2 and EEMUA 191 recommend: most alarms
   Low, few High.
2. A trip is not a priority: the alarm that caused it is a High alarm marked first out, and the
   trip itself is an event.
3. Information is never an alarm; it is an event (§13.7).

### 13.4 Records

1. The base block writes every transition as an OpenOT condition record: raised, cleared,
   acknowledged, shelved, unshelved, suppressed, out of service, in service, reset and comment,
   with time, priority and, for operator actions, the user.
2. An alarm's identity in the records is its instance path, for example `P1.TankLevelHigh`. OpenOT
   annotations therefore work inside function blocks, per instance.
3. The OpenOT records are the alarm history; the display keeps no second history.

### 13.5 Operator actions

1. Acknowledge, shelve (with a duration), unshelve and comment are commands (§12.5) and require
   the Operator role; out of service and in service require the Engineer role. All are in the
   audit log (§12.7).
2. The runtime passes the request to the alarm instance, which applies it in the next scan; the
   outcome is shown like any command outcome.
3. The requests for the alarm list, the records and the operator actions are specified in
   specification 40, §5.1.

### 13.6 Display

1. The runtime lists every instance of a block that extends the alarm base, so customer alarms
   appear without registration.
2. The top bar shows the alarm counter and the newest alarm.
3. The alarm list shows unacknowledged alarms first, then by priority, then newest first, with
   acknowledge. Shelved, suppressed and out-of-service alarms are listed separately with their
   counts, never hidden without a trace.
4. Objects show the alarms of their equipment.
5. Priority is shown by colour and shape. Unacknowledged alarms blink, and stop blinking while
   their data is stale (§5.3).
6. **Marks** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): High is a diamond
   in `alarm-high` (red), Medium a triangle in `alarm-medium` (orange) and Low a circle in
   `alarm-low` (yellow), each with "!"; lists also name the priority. A project may set these
   colour tokens like any other (§14.1).
7. **Top bar:** the counter says how many alarms annunciate ("No alarms", "1 alarm",
   "3 alarms"), in the mark and colour of the highest priority among them. Its priority mark
   blinks once a second while any of them is unacknowledged; the number and badge stay fully
   visible throughout. Beside it the newest alarm shows its mark, message
   and time. Shelved, suppressed and out-of-service alarms follow as a note with their counts, for
   example "2 shelved · 1 out of service".
8. **Stale:** while the value stream is closed or its updates are stale (§5.3, item 2), the
   alarm counter, the newest alarm, the alarm list and the equipment marks are marked stale, and
   nothing blinks.
9. **Equipment** (item 4; decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): an
   alarm's equipment is the function block instance nearest above it that holds it, for example
   `Tank.P1` for `Tank.P1.FailedToStart`; an alarm a program holds directly has none. An object
   with a binding below that instance (`Tank.P1.Running`) shows, at its top right, the mark of
   the highest priority among its equipment's annunciating alarms, with their number when there
   are several; it blinks while one of them is unacknowledged. Selecting the mark opens that
   alarm, as the alarm list does. Trend headers reserve room for that mark and its count;
   no range, Export or Expand action may overlap it, including narrow layouts and multi-digit counts.

### 13.7 Events

1. Events need no action and have no acknowledge.
2. The event log shows the OpenOT records (alarm transitions, messages, operator actions, batch and
   recipe events) and the command outcomes of the audit log, newest first.
3. A program raises an information event with an OpenOT message record.

### 13.8 Historian alert rules

Historian alert rules stay a separate data notification, for example a webhook. They are not
alarms: not in the alarm list and never acknowledged.

### 13.9 Alarm tutorial

When the alarm handling is implemented, a separate alarm tutorial shows it in a real project:
adding alarms to a program, priorities, acknowledge and shelve in the runtime display, first out
on a trip, equipment alarms, and own detection by extending the base.

## 14. Look and assets

### 14.1 Design system

1. **Tokens:** one design system (colours, typography, spacing) with light and dark themes. The
   editor shell, the canvas and the runtime take their tokens from it (§3.5). Decided on
   2 October 2026 while the user was away, approved by the user on 3 October 2026:
   - the editor shell takes truST's shared webview tokens (`--trust-*` in
     `editors/vscode/src/webview/theme.css`), which follow the VS Code theme, like every other
     truST webview;
   - the canvas and the runtime take the HMI tokens (`--hmi-*`) of the screen's theme;
   - the shipped HMI tokens meet the contrast of item 4 in both themes, for every text the
     drawing draws in every state (values, unknown, stale, commanded, refused, missing, alarms).
2. **Project colours:** a project may set its own colours in `hmi/screen.toml`, in a
   `[colors.dark]` and a `[colors.light]` table: any colour token by name, as a hex colour
   (`#rgb`, `#rrggbb` or `#rrggbbaa`). A token the project does not set keeps truST's default,
   so the shipped palette is the default look. An unknown token name or a value that is not a hex
   colour is an error with its line. The editor offers a colour picker for each token, with a
   reset to the default. Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026: the
   inspector's Colours section, shown when no object is selected, lists the tokens of the
   screen's theme in groups (surfaces, text, data, states, alarms), each with its colour. Choosing
   a colour writes it to `[colors.<theme>]` of `hmi/screen.toml`; Reset removes the project's
   colour, so the default applies again. The section shows the colour warnings of item 4 beside
   their tokens.
3. **Modern look:** a dark-first design with a near-black page, a step-lighter surface for
   cards and rounded shapes, and a clean light theme. Live values are drawn in one vivid accent
   (gradients and a soft glow allowed); "on" states light up. Warnings, stale or unknown data and
   alarms each have their own colour, never the accent's, so they stand out at a glance. No grey
   palette for the data itself. Trend pens have their own colours, `pen-1` to `pen-4`, none of
   them the warning or alarm colour. Alarm priorities have `alarm-high`, `alarm-medium` and
   `alarm-low` (§13.6).
4. **Quality checks:** the editor flags low contrast, too many colours, missing units, unbound
   objects, touch targets below 44 px, and text below 14 px. Decided on 2 October 2026 while the
   user was away, approved by the user on 3 October 2026:
   - The checks are warnings. They show in the Problems view as warnings (source `truST HMI`)
     and in the editor, but never stop a deployment.
   - The language server decides every warning, its threshold and its sentence
     (`trust-lsp.hmiQuality`). The editor's drawing reports only what the drawing knows: each
     text it cut to fit, in each of the project's languages, and each text of the component
     specimen with what lies behind it; the program's variable types and the alarm colours'
     distances are read and computed by the server.
   - **Touch targets:** a control, a page button, or any object with a click (§5.4) that is
     narrower or lower than 44 px at the design size is flagged at its line.
   - **Missing units:** a value, gauge, bar, split bar, slider, numeric input or setpoint entry
     with a REAL or LREAL variable bound and no `unit` is flagged at its line. `unit = ""` states
     that the number has none.
   - **Unbound objects:** an object whose component takes bindings but has none is flagged at
     its line: it shows nothing in the runtime.
   - **Low contrast:** each text the drawing draws, in every state, against what lies behind it,
     with the project's colours: at least 4.5:1, or 3:1 for text of at least 24 px, or 18.66 px
     and bold (WCAG 2.1, 1.4.3). A sign, a text of symbols only such as `?`, `!`, `…` or `●`, is
     a graphic and needs 3:1 (WCAG 2.1, 1.4.11). The controls of a read-only page are inactive
     and need none. A linear gradient counts where the text lies on it; a radial glow, such as
     the page's, counts at its strongest, since text can lie anywhere on a page. Only project colours can fail, so each
     failing pair is flagged at the line of the project colour in `hmi/screen.toml`.
   - **Colours that look alike:** a project colour for a token other than `warning`, `stale`
     (which shares the warning's hue by design) and the alarm colours within 0.1 in OKLab of the
     warning colour, the alarm colour or an alarm priority colour is flagged at its line, since
     data could then be taken for an alarm (item 3).
   - **Too many colours:** in phase 1 no object chooses its own colour; a page shows only the
     palette's roles (the accent, four pens and a trend's warning bands). The check starts with
     the first colours an object can choose (phase 2).
   - **Text below 14 px:** the drawing draws no words or numbers below 14 px at the design size
     (§3.5 item 2) in any state, so no page can break the rule; a drawing test holds it. Signs
     inside marks are graphics and may be smaller. Text that does not fit is cut with `…`
     instead, and an object whose label or text is cut is flagged at its line, so it can be
     made wider or the text shorter.

### 14.2 Symbols

Decided with the user on 2 October 2026 (§20, decision 7).

1. **Shapes:** each symbol keeps the shape engineers know from P&IDs (ISO 10628-2 and ISO 14617;
   ISA-5.1 for valves, actuators and instruments), so it reads without training, and draws it in
   a modern way.
2. **Moving parts:** what moves in the plant moves in the symbol. A running pump's impeller
   turns; a valve's closing element stands where the valve is: across the pipe when closed, in
   line with it when open, and between when it moves or when a control valve is partly open.
3. **State:** shown by the moving part, by the valve body (filled when closed and hollow
   otherwise, as on P&IDs) and by the state word under the symbol. No state rests on colour
   alone; normal states have no colour of their own, and no part of a symbol glows.
4. **Faceplates** show the symbol of their equipment.
5. Design rules, decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026:
   - **one grid, one line:** symbols are drawn on a 4 px grid at their design size with a 2 px
     line, round joins and round caps, the line staying 2 px at any size;
   - **calm tones from the tokens (§14.1):** the body in `raised`, the lines in `muted`, the
     moving part in `text` (a stopped impeller in `muted`), so a project's colours and both themes
     apply;
   - **the label:** the tag in 14 px semibold and the state word in 14 px under it, centred
     under the symbol. Fault "!", manual "M", out of service "OOS" and a stale value's age are
     signs beside the state word, each on its own ground with an edge in `muted` (the fault sign
     in `alarm`), never on the symbol; a stale value's outline surrounds the symbol only;
   - a symbol keeps its proportions in any size, centred above its label;
   - **motion:** an impeller turns once every 2 s, steadily across redraws. When the viewer's
     system asks for reduced motion, nothing turns and the state word carries the state.

### 14.3 Symbol library

1. **Definitions:** each symbol is defined once in the drawing module: its parts on the grid, its
   moving part, its ports and its label. Decided on 2 October 2026 while the user was away, approved
   by the user on 3 October 2026: one module per symbol under `drawing/symbols/`, all sharing one style module; the
   shapes are checked against Equinor's engineering symbols (MIT licence), which follow the same
   standards.
2. **Proof before the library:** one pump, one on/off valve and one control valve first:
   - every state;
   - both themes;
   - three sizes;
   - shown in the editor and in the runtime.
   The user approves them before the rest of the library is made.
3. **Review:** every new or changed symbol is reviewed as drawn in every state, in both themes and
   at three sizes, beside professional references, and approved before release.

### 14.4 Customer symbols

Phase 3: customers import an SVG, name its parts, and define its ports and visualizations
(**My Symbols**).

Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026:

1. **Files.** A symbol of the project is two files: `hmi/symbols/<name>.svg`, its drawing, and
   `hmi/symbols/<name>.toml`, its definition; `<name>` has letters, digits, `-` and `_`. A page
   places it as an object of type `my:<name>`. The files are part of the HMI bundle (§16). A
   library ships symbols the same way, in `<library>/hmi/symbols/`, beside its panels and pairs
   (§8.7 item 2), so a customer's library can carry its function blocks with their own pictures;
   the bundle holds them with the project's, and a project file of the same name takes a
   library's place.
2. **Definition:** `format = 1`; `title`, its name in the Components panel; `size`, a new
   object's size; `[properties]`, each binding it takes with its kind (`boolean`, `number` or
   `text`); `[[port]]` entries, each with `name`, `at` in the drawing's units and `direction`
   (`left`, `right`, `up` or `down`); and `[[part]]` entries, each naming an element of the
   drawing by its `id` and driving it in one of the ways of §5.3 item 5: `spin` (turns while
   TRUE), `stroke` (moves to and fro while TRUE), `angle` (stands at the value, in degrees),
   `level` (filled from the bottom to the value's share of `range`, by default the object's
   range), `travel` (moved by that share of `move = [x, y]`, its displacement at full travel in
   the drawing's units), `show` (its text shows the value), `blink` (blinks while TRUE),
   `alarm` (painted in the alarm colour while TRUE), `active` (painted in the moving colour
   while TRUE, glowing softly as a lit status does) or `equals` (shown only while the value
   equals `value`). A part drawn as a line,
   without a fill of its own or of its group, is painted on its line, a filled part on its fill
   (refined on 7 October 2026, when a heater element's outline filled solid while it heated). The property must be one of its properties, of a
   kind that drives the part: a boolean for `spin`, `stroke`, `blink`, `alarm` and `active`, a
   number for `angle`, `level`, `travel` and `equals`, any for `show`. A `fault` property shows Fault under the tag as on
   equipment, and `manual` and `out-of-service` show their signs.
3. **Drawing.** The drawing is cleaned when it is read. Kept are shapes (`svg`, `g`, `path`,
   `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `use`), text (`text`, `tspan`)
   and definitions (`defs`, `linearGradient`, `radialGradient`, `stop`, `clipPath`), with their
   geometry, paint, text and transform attributes; the declarations of a `style` attribute, and
   of a `<style>` element's class rules (`.name { … }`), among those become attributes. References may point only into the drawing (`url(#…)`,
   `href="#…"`). Anything else is left out: comments, metadata, an editor's own elements and
   attributes (Inkscape, Illustrator), other elements and attributes. A drawing with a script,
   an event attribute (`on…`), an embedded object or a reference outside it is refused, with
   its line. Each id is prefixed with the symbol's name, so two symbols' ids never clash.
4. **Showing it.** The Components panel lists the project's symbols under **My Symbols**. The
   editor and the runtime draw a symbol as they draw equipment: the drawing scaled to fit above
   the tag and state word, its parts driven by the bindings, in Live mode and on the runtime
   display alike. `currentColor` in the drawing is the line colour of the built-in symbols, so a
   drawing that uses it follows the dark and light themes and the project's colours (refined on
   7 October 2026, when a hand valve drawn with `currentColor` was black on the dark theme).
5. **Import.** **HMI: Import Symbol** asks for an SVG file and a name, copies the drawing to
   `hmi/symbols/<name>.svg` and writes a definition with its title, a size in the drawing's
   proportions and each element id of the drawing as a part to name, then opens the definition.
6. **Checks.** The checks of the Problems view and of a bundle before it is deployed report a
   definition's problems on their lines: a part without an element of that id, a property of
   the wrong kind, a port outside the drawing, a refused drawing. A page using a symbol the
   project does not have is told that its component type is not defined.

7. **Visual editing (Phase 5).** Opening a symbol definition offers its cleaned drawing in the
   screen's theme, a list of its named SVG elements, and an inspector for the selected part's
   rules. Selecting a part on the drawing selects the same part in the list. The inspector
   offers the seven drives of item 2 and only properties of a compatible kind; bindings and
   ports remain editable. Preview values exercise the existing shared drawing and do not
   change the definition. Edits change the TOML document through the runtime's canonical
   writer, retaining normal undo, redo, save and Open as Text. Invalid text keeps the last
   valid preview and pauses visual edits with the parser's explanation. The SVG artwork is
   unchanged. The shell uses the shared truST tokens and the preview uses the existing HMI
   palette and symbol rendering (§14.1).

### 14.5 Languages and texts

Decided with the user on 2 October 2026: every word an HMI shows can be changed, and the operator
switches the language of the runtime display with one click.

1. **Any text:** every word the drawing and the runtime display show goes through the screen's
   text table, so a project can change it. That includes truST's own words: state words, signs,
   alarm states and actions, command outcomes, "Not in running program" and the display's
   buttons and notices. A project text is the text the engineer typed; a tag is a name and is
   shown as it is.
2. **Languages:** `languages` in `hmi/screen.toml` lists the project's languages as BCP 47 tags,
   the first being the language its texts are written in; without it, `["en"]`. truST ships its
   own words in 11 languages: English (`en`), Chinese, Simplified (`zh-Hans`), German (`de`),
   Spanish (`es`), Japanese (`ja`), Portuguese, Brazil (`pt-BR`), French (`fr`), Korean (`ko`),
   Italian (`it`), Turkish (`tr`) and Swedish (`sv`). A project may list other languages; their
   words are its own. Beside Inter, truST ships a font for Chinese, Japanese and Korean (Noto
   Sans, SIL Open Font License), subset to GB 2312, JIS X 0208 and every modern Hangul syllable,
   with all characters of truST's words in those languages; a browser loads one only for a page
   that shows such text, and a page in Japanese or Korean takes its own forms first. Text beyond
   those sets shows in the device's fonts.
3. **Text files:** `hmi/texts/<tag>.toml` holds the project's texts in one language: `format`,
   `language` and a `[texts]` table from each text as written to the text shown, for example
   `"Tank T-101" = "Tanque T-101"`. A text is its own key, so there are no names to invent.
   truST's own words are keyed by their English wording; a project's file may give any of them
   its own text, in its own first language too. A text with a value names it in braces, as
   `"Shelve for {minutes} min" = "Parkera i {minutes} min"`.
4. **Lookup:** a text is shown from the project's file for the language, else from truST's words
   for the language, else as written. The message a program gives an alarm is a text of the
   project too, shown in the alarm list, the alarm banner and the event log. The editor lists
   every text without a translation.
5. **Runtime display:** a language button in the top bar, or at the top right without one, lists
   the project's languages by their own names (Svenska, Deutsch, 日本語, …). Choosing one redraws
   the page at once and is remembered by that browser; `?lang=<tag>` in the address fixes a panel
   to a language; without either, the first language applies. Numbers, dates and times follow
   the language (decimal comma in Swedish).
6. **Fit:** text is measured with the widths of the font the HMI ships (§14.2). Text that does
   not fit its object is cut with "…"; signs are never cut. The editor warns of every text that
   is cut in any of the project's languages.
7. **Editor:** a language picker above the canvas shows the page in each project language, in
   design and in Live mode. In a language other than the first, a click on a text on the canvas
   edits its translation in place, and the inspector's text fields name the language they are
   written in. The Texts table lists every text of the project in every
   language, the missing ones marked; a "longest" preview lengthens every text by 40 % to find
   tight objects before translating.
8. **Language server:** checks the text files with file and line like the other HMI files, and
   gives the editor the project's texts: those on its screen, pages and popups, those of the
   panels they show, placed on a page or opened from equipment, its own and its libraries' (refined
   on 7 October 2026, when a placed sequence panel's labels could not be translated), the messages
   its declares for alarms, and truST's own words. A panel placed on a page shows its labels and
   buttons in the display's language as a page does. Dynamically computed alarm messages are
   available from the running PLC in Live mode. Authoring must not execute a scan to discover
   them; unavailable computed messages remain explicitly unknown.

## 15. File format

### 15.1 Files

| File | Holds |
| --- | --- |
| `hmi/screen.toml` | screen layout (§4.1) |
| `hmi/pages/<name>.toml` | one page |
| `hmi/popups/<name>.toml` | one popup |
| `hmi/pairs/<name>.toml` | project equipment pairs; shipped pairs live in their library |
| `hmi/symbols/` | customer symbols |
| `hmi/texts/<tag>.toml` | the project's texts in one language (§14.5) |

### 15.2 Rules

1. **Validation:** every file has `format = 1`. Unknown keys and invalid values are errors with
   file, line and a plain message. A display range (`min`, `max`) is a finite number.
2. **Canonical writing:** the editor writes canonical TOML:
   - fixed key order;
   - objects in z-order;
   - bindings sorted by property name;
   - one value per line;
   - whole-pixel positions on the grid;
   - stable object ids (§5.1);
   - no timestamps.
   Pipes are stored as connections plus optional waypoints.
3. **Small diffs:** moving one object changes one line, and rebinding one property changes one
   line.

### 15.3 Example

```toml
# hmi/screen.toml
format = 1
size = [1920, 1080]
theme = "light"
symbols = "flat"

[top_bar]
title = "Feed station"
clock = true
alarms = true
user = true

[navigation]
style = "side"

[colors.light]
accent = "#0e7490"
```

```toml
# hmi/pages/overview.toml
format = 1
title = "Overview"

[[object]]
id = "tank-1"
tag = "TK101"
type = "tank"
at = [400, 300]
size = [160, 260]

[object.bind]
level = { path = "FeedArea.TK101.Level", unit = "%" }

[[object]]
id = "pump-1"
tag = "P101"
type = "pump"
variant = "horizontal"
at = [640, 480]
size = [80, 80]

[object.bind]
fault = "FeedArea.P101.Fault"
running = "FeedArea.P101.Running"
speed = { path = "FeedArea.P101.Speed", unit = "rpm" }

[object.click]
faceplate = "pump"

[[pipe]]
from = "tank-1.outlet"
to = "pump-1.inlet"
```

## 16. Deployment

### 16.1 The HMI bundle

Deploy sends one HMI bundle: screen layout, pages, popups, assets, faceplates, pair definitions,
the binding list and the page permissions. Its version is the hash of its content.

### 16.2 Transaction

1. **Editor check:** before sending, the editor validates the complete bundle: schema, assets,
   bindings and permissions.
2. **Runtime check:** the runtime validates it again against the running program and the runtime
   write policy:
   - every bound path exists, with a compatible type and direction;
   - every control is within the policy.
3. **Atomic activation:** a bundle that passes every check is activated in one step, and the
   previous bundle is kept. The runtime's existing deployment mechanism (payload validation and
   atomic switch) is reused where it applies.
4. **Failure:** if any check fails, nothing changes, the previous bundle keeps running, and the
   failure is reported with its reasons.
5. **Rollback:** one action reactivates the previous bundle.
6. **Old browsers:** browsers still showing an older bundle are told to reload. Commands carrying
   an older bundle version are refused as out of date.
7. **Authorization:** deploy requires the deploy permission (§12.3) and is recorded in the audit
   log.
8. **From the editor** (decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026): Deploy
   builds and checks the project's bundle (item 1) and sends it to the selected runtime's web
   server (`POST /api/hmi/deploy`, §16.4), found from the web endpoint the runtime reports, with
   the runtime's administrator access (its control token). Problems stop it and are listed. The
   simulator has no web server, so Deploy says to use Live mode or start a runtime. Deploy saves
   the project's unsaved HMI files first, since the bundle is built from the saved files. After a
   deployment the editor offers Roll back (`POST /api/hmi/rollback`).

### 16.3 Program changes

When a program deployment would break bindings of the active HMI bundle, the deploy shall:
- list the affected bindings;
- offer to deploy the program together with an updated HMI bundle.

Bindings that remain broken are shown as "not in running program".

### 16.4 Bundle and requests

Decided on 2 October 2026 while the user was away, approved by the user on 3 October 2026.

1. **The bundle** is one JSON document:

   ```json
   { "format": 1, "version": "<hash>", "files": { "hmi/screen.toml": "<text>", "hmi/assets/pump.png": { "base64": "<bytes>" } } }
   ```

   - It holds every file under `hmi/`: the screen layout, pages, and later popups and
     faceplates, with their assets.
   - The binding list and the page permissions are read from the pages; they are not copied.
   - The version is the SHA-256 of the files in path order, each written as path, a zero byte,
     contents and a zero byte. Its first 16 hexadecimal digits are the page version of §12.8.
     One implementation in the runtime serves the language server and the runtime.
2. **The editor check:** the language server builds the bundle from the project's `hmi/` folder
   and checks it: files load, referenced assets exist, bindings resolve in the edited program,
   and controls are within the project's write policy. It answers with the bundle and its
   problems.
3. **The runtime** keeps the active bundle as the `hmi/` folder of its project and the previous
   one beside it. `POST /api/hmi/deploy` takes the bundle:
   - the version must match the content;
   - files load and assets exist;
   - every bound path exists in the running program with a compatible type and direction;
   - every control is within the runtime write policy.

   A bundle that passes is activated in one step, under a lock that every HMI read takes, so a
   page is never read half-switched. Deploying the version that is already active changes
   nothing, so the previous bundle stays the one to go back to. `POST /api/hmi/rollback` makes
   the previous bundle active. Both answer with the active and previous versions, or the problems
   with file and line.
4. **Deploy permission:** the runtime's administrator access (as for the project deploy), or an
   HMI session with the Engineer role. Each deploy and rollback, refused or done, goes to the
   audit log with who, the versions and the outcome.
5. **Old browsers:** the value stream sends `{ "bundle": "<version>" }` when the active version
   changes. A page drawn from another version tells the user to reload, and its commands are
   refused as out of date.
6. **Program changes:** a program deployment through the runtime (`POST /api/deploy` with new
   sources) checks the active bundle's bindings against the new program before it writes
   anything:
   - when bindings would break, nothing changes and the answer lists them (`broken`, each with
     file, line and message);
   - the deployment may carry an updated bundle (`hmi` in the deploy request); it must pass
     against the new program and is activated with it;
   - with `accept_broken_bindings`, the program is deployed alone and the answer still lists the
     broken bindings.

   A binding the running program does not have is shown on the page as "not in running
   program" (§11.1).

## 17. Architecture

### 17.1 Kept from the platform

- the runtime web server, control endpoint, authentication, pairing and roles;
- the control operation registry and its audit trail;
- the historian, as the source for trends;
- the VS Code connection to a running simulator or runtime;
- the language server's knowledge of program symbols, and `trust-lsp.hmiBindings` (process
  image and control symbols, used by Control Studio and Network Canvas).

### 17.2 Removed (decision 12)

The legacy HMI was deleted before the builder's runtime display existed:

- its page format: `hmi/*.toml` sections and widgets, `_config.toml`, the layout-overrides file,
  the auto-generated process template, and the scaffold that `trust-runtime run` wrote into
  projects;
- the `/hmi` page, its browser modules and assets, the offline export and the live websocket;
- the `hmi.*` control operations with their write path, alarm and trend state, and the
  `[runtime.hmi_persistence]` settings;
- the `trust-runtime hmi` command, the language server's `trust-lsp.hmiInit` command and its HMI
  diagnostics;
- the VS Code HMI preview panel and the HMI language-model tools;
- the legacy example HMI folders, HMI tutorials and documentation, and the tests, fuzz target,
  soak workflow and browser captures that covered them.

Proven designs from the removed code (value quality, writes checked for type, capacity, range
and non-finite values, alarm states) may be taken from version history, but they return only
through the new modules below, test-first. Existing pages are not converted. Shipped examples
and tutorials are rebuilt with the builder.

### 17.3 New modules

1. **Drawing module** (TypeScript): draws a page from its model and live values, including value
   states and staleness; shared by the runtime and the editor.
2. **Page model and TOML serializer** (TypeScript) for the editor; **loader and validator**
   (Rust) in the runtime.
3. **HMI Builder** custom editor: React for panels and inspector; the drawing module plus an
   editing layer for the canvas.
4. **Runtime modules:**
   - page serving and the live-value stream with value quality and timestamps (§11);
   - nested points (§7.3);
   - writes checked for type, capacity, range and non-finite values;
   - write policy and permission check (§12.2);
   - command tracker with outcomes and deduplication (§12.5);
   - session handling (§12.4);
   - alarm and trend state (§13);
   - audit log (§12.7);
   - bundle store with atomic activation and rollback (§16).
5. **Language-server requests:**
   - parse, format and binding checks for page files (§7, §15);
   - the component catalogue (§5.2, §5.6), so the editor's Components panel and the checks use one
     definition;
   - the variable tree with members; each variable says which kinds of value it holds and
     whether a binding may write it, by the rules of the binding checks (§7.6);
   - **Add missing variables to program**;
   - rename and references across HMI files.
6. **Plant library**: ST function blocks (specification `trust-plant-library`), panels and
   component definitions.
7. **Symbol library**: the symbol definitions and their shared style (§14.3).

Model, drawing, editor, runtime loader and runtime command handling are separate modules. No
source file grows beyond about 1,000 lines.

### 17.4 Data flow

- **Editing:** editor ⇄ TOML text documents.
- **Deploy:** bundle ⇄ runtime checks ⇄ atomic activation ⇄ schema ⇄ drawing module in the
  browser.
- **Live values:**
  - runtime ⇄ websocket ⇄ drawing module, in the runtime display;
  - runtime ⇄ VS Code connection ⇄ editor, in Live mode.
- **Commands:** control ⇄ runtime authorization ⇄ command tracker ⇄ guarded write ⇄ program ⇄
  feedback readback ⇄ outcome on the control ⇄ audit log.

## 18. Delivery

### Phase 1 — the builder

Phase 1 is delivered in four milestones. Each milestone is accepted on its own.

#### 1A — Authoring

What must work:
- create, bind, save, reopen, undo and redo;
- split view with text, external changes, restart recovery;
- nested member bindings in the editor and in the runtime;
- read-only rendering identical in the editor and the browser.

Done when:
- a page with values, a gauge and a trend bound to members of a `Control.PID` instance renders
  identically in the editor and in the browser, compared by screenshot;
- every editor-reliability test of §19.2 passes.

#### 1B — Operation

What must work:
- one setpoint and one command, with authorization (Operator backend change), confirmation,
  limits, command outcomes, readback, audit, session expiry and failure recovery.

Done when each of these cases is shown:
- an Operator changes a permitted setpoint and sees it confirmed by readback;
- a denied write is refused with its reason;
- an expired session disables the controls;
- a disconnect during a command ends as unknown and is not resent;
- every case appears in the audit log.

#### 1C — Deployment

What must work:
- the bundle transaction of §16: validation, compatibility check, atomic activation, rollback;
- refusal of commands from out-of-date browsers;
- reconnect.

Done when:
- an invalid bundle is refused while the previous one keeps running;
- a valid bundle activates in one step;
- a rollback restores the previous bundle;
- a browser showing the old bundle has its commands refused and is told to reload.

#### 1D — Complete builder

What must work:
- the remaining phase 1 components;
- screen layout, navigation and popups;
- trends with limit bands, alarm counter and alarm list;
- the usability requirements of §3.5;
- visual acceptance of rendered screenshots.

Done when the first acceptance project passes, a tank-level process using the real
`Control.PID`:
1. build its screen in the builder;
2. observe it in Live mode against the simulator;
3. deploy it;
4. change a permitted setpoint in the browser as Operator;
5. trigger and acknowledge a high-level alarm;
6. disconnect and reconnect;
7. roll back a deployment;
8. rename a bound variable in the program and recover its bindings.

### Phase 2 — process graphics and equipment

Milestones:
1. **2A:** the pump and valve symbol proof of §14.3, approved by the user.
2. **2B:** the equipment behaviour specification (§8.2), approved; then the shipped function
   blocks with their tests.
3. **2C:**
   - the phase 2 components (§5.2);
   - states and visualizations (§5.3);
   - pipes (§6);
   - the shipped pairs and controller faceplates (§8.1);
   - the course coverage test (§5.5).
4. **2D:** Create pages from program (§9) and Add missing variables to program (§10).

Done when all of these hold:
- **Program first:** pages are created from a program containing pumps, valves, a tank and a PID
  loop, and work in Live mode with their faceplates.
- **HMI first:** a page drawn first produces, after one approved change, a program that compiles,
  runs in the simulator and drives the same page. Running the action again adds nothing.
- **Coverage:** every plant and controller type of the control course has its component or
  faceplate, and the coverage test passes.

### Phase 3 — breadth and quality

Deliverables:
- adaptive display (§4.5);
- phase 3 components (§5.2);
- My Symbols (§14.4);
- screen tests: operator journeys against the simulator, and screenshot comparison.

Done when all of these hold:
- **Adaptive display:** one page shown at 1920×1080, 1280×800 and tablet sizes passes the visual
  checks, with no overlaps and the minimum sizes kept.
- **My Symbols:** an imported SVG symbol with named parts animates in Live mode and in the runtime.
- **Screen tests:** an operator-journey test and a screenshot comparison run in CI, and both fail
  on a deliberate break.

Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026: the screen tests are truST's own,
in `scripts/hmi-screen-tests` (Playwright with Chromium), run by CI's HMI Screen Tests job. The
operator journey runs the tank-level example in `trust-runtime`: the live level shows, an Operator
logs in, changes the setpoint and sees it confirmed by readback, and acknowledges the high-level
alarm. The screenshot comparison draws a reference page of displays, controls and equipment in
fixed states with the drawing bundle the runtime serves, in both themes, against approved images;
a pixel may differ by antialiasing (Playwright's threshold 0.2), and at most 0.2 % of the pixels.

### Phase 4 — engineering views

Deliverables:
- a P&ID view of the same objects;
- **Generate process screen from P&ID**;
- agent-driven authoring from a description, with rendered refinement (specification 43).

Done when all of these hold:
- **P&ID view:** the P&ID of TK101, P101, XV101, PT101 and LT101 shows the same objects as the
  HMI, and an edit in one view appears in the other.
- **Generation:** generating a process screen from it gives a working screen bound to the same
  instances.
- **Agent authoring:** specification 43's end-to-end agent, visual review and edit-transaction
  acceptance passes; a generated file alone is insufficient.

Decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026:

1. **P&ID files.** A P&ID is `hmi/pid/<name>.toml`, in the page format of §15 with its title,
   objects and pipes; its objects are process objects (equipment, instruments, junctions and
   the project's own symbols), texts and lines. It opens in the HMI Builder in the screen's
   theme, as the pages do (the user, 3 October 2026: the same look as the HMI, not a plain
   black-on-white one), each symbol as it stands without states, its tag under it; the
   Components panel offers what a P&ID may hold, and there is no Live mode. A P&ID never runs,
   so of the quality checks (§14.1 item 4) only cut texts apply to it: an object without a
   variable, such as a hand valve, is no warning there.
2. **The same objects.** A page names its P&ID with `pid = "<name>"`. A process object with the
   same `id` in both is the same object. Its type, variant, tag and bindings, whether it exists,
   and the pipes between such objects are one in both views: changed in one, they change in the
   other, saved or not. Positions and sizes are each view's own; texts, lines, displays and
   controls belong to their view alone. An object added to one view comes into the other at
   the same place scaled from one view's size to the other's.
3. **Generate Process Screen from P&ID** writes `hmi/pages/<name>.toml` linked to the P&ID, if
   it has no such page: the P&ID's title, its process objects and their pipes, each placed
   where it stands on the P&ID scaled to the screen's design size, bound as on the P&ID. It
   opens in the builder.
4. **Agent-driven page authoring** uses the customer's existing agent and truST engineering
   tools, as specified in [43 — Agent-driven HMI authoring](43-hmi-agent-authoring.md).
   The agent inspects program context and the visual library, composes an editable draft,
   renders and critiques it, validates bindings and visual quality, then presents an undoable
   change in the builder. The extension does not call a model itself. Specification 43
   supersedes the former direct-model “Draft Page with the Assistant” implementation and
   its real-model gate; that implementation is not acceptance of the agent workflow.

### Phase 5 — state of the art and easy to use

Decided with the user on 3 October 2026, after an independent review against Ignition,
WinCC Unified, FactoryTalk Optix and TwinCAT HMI: the builder shall be as easy to use as a
modern design tool, and the runtime display operator-grade. Each item is specified in its
section before it is built.

**Easy, without settings** (the user, 3 October 2026: "very easy to use without 1000000
settings", not a heavy engineering tool). This rule governs every item below:
- each feature works with no configuration, from defaults taken from the program, the libraries
  and the project: units, limits, decimals, faceplates, history and sound need no setup;
- a gesture or a key does what a design tool does there, without a dialog;
- a new setting exists only where no default can be right, and then once, in the place the user
  already looks; a feature that cannot meet this is dropped or simplified, not configured;
- a new user finds each feature on the page or in the context menu, without reading a manual.

Deliverables:
1. **Editing** (§3.1–§3.3, §3.5):
   - selection by Shift-click, box and Ctrl+A;
   - copy, paste and duplicate, also across pages;
   - align and distribute, group and ungroup, bring forward and send back;
   - the Pages and Layers panels and a context menu;
   - zoom with the wheel and pinch, pan with Space-drag, zoom to the selection;
   - current values in the variable picker;
   - a Live mode that operates controls through the run target, under the runtime's roles and
     confirmations.
2. **Reuse and dynamics** (narrowed with the user on 3 October 2026: faceplates and pairs already
   give each block type its faceplate on every instance, and the program and the Equipment
   library scale values and decide states, so no separate template system, scaling or colour
   bindings):
   - a faceplate placed inline on a page, bound to an instance, beside its use as a popup;
   - a visibility binding: an object or group shown while a BOOL the program computes is TRUE
     (decided on 4 October 2026 while the user was away, to be approved: every component takes
     the binding `visible`, a BOOL read; the object is drawn while it is TRUE and left out while
     it is FALSE, stale or not; while its value is unknown the object stays shown, so nothing
     disappears on a lost connection. The inspector names it "Visible while"; with several
     objects selected, a group among them, its Visibility section sets it on each. A trend's
     `visible` is no pen);
   - a visual editor for the parts and rules of the project's own symbols.
3. **Operator displays:**
   - trends with zoom, pan, cursor, time range, pause and CSV export, with history on by default;
   - the alarm list with value and limit columns, sorting, filtering and an audible alarm that
     can be silenced;
   - normal and alarm ranges on bars and gauges, and one number format per value everywhere;
   - controls disabled, with the reason, where the user's role may not use them;
   - the defects of the review fixed, and two showcase pages at professional density.
     The equipment example's Overview combines the process graphic, level/actuator readings,
     status, trend and alarms; Cards combines the three bound equipment cards, separate level
     and output trends, and alarms. Both use actual existing program variables and are reviewed
     in both themes, with the login inside the top bar and no empty band below it.
4. **Proven operation:**
   - the display stream sends every value once, then only the values that changed, and the
     cycle count alone when nothing changed, so staleness is still detected (§11.1);
   - moving parts in a layer of their own, so an update does not repaint them: the runtime
     display draws each turning part in an element of its own above the page, which the browser
     turns by itself, and sets the page's own part aside. The part is measured when it starts
     turning and again when its object moves; a change of its drawing is copied over; a part
     that stops is given back to the page; where the page cannot be measured, the parts turn in
     the page;
   - **the HMI runs where the program calls it** (decided with the user on 3 October 2026):
     a block `Hmi.Display` of the HMI library (`libraries/hmi`), called in a program that runs
     in any normal task the user chooses. At the end of each scan in which it was called, after
     that scan's outputs are published and before the next scan, the runtime applies the
     commands the screens queued, calls the methods they asked for (§21.2), copies the values
     the open screens show and reads the alarm states, on the PLC thread; the calling task's
     interval and priority set how often this happens, and a scan without a call does no HMI
     work. A command therefore reaches every task before any output it affects is published.
     (Integration with the shared execution engine, 23 October 2026: this work moved from
     directly after the calling task to the end of that scan.) Sending pages to browsers, logins
     and websockets stay on the web server's own threads. The screens' values and commands follow §11.1; the editor's one-off reads (a
     faceplate's block type, a command's readback, Live mode) also wait for the next call; the
     alarms are read at each call, by one walk of the program's instances (specification 40, §5).
     **New HMI Screen** (§4.10) adds the library, a `Screens` program calling the block in
     `src/screens.st`, and its task beside the configuration's last program:
     `TASK ScreenTask (INTERVAL := T#250ms, PRIORITY := 10);` and
     `PROGRAM Screens WITH ScreenTask : Screens;`, unless the program already calls the block;
     without a configuration the program runs with the project's other programs. Without a call,
     the editor warns at the top of `hmi/screen.toml`: "No program calls Hmi.Display: call it in
     a task to show live values on the screens". The shipped examples with screens call it this
     way;
   - stale values of failed links (specification 42, §5 item 4);
   - logins measured under command load, and at most 5 login attempts a minute from one address.
5. **Tags of other runtimes:** a variable another runtime publishes on the mesh, picked like a
   variable of the program, shown live and written under the owning runtime's write policy,
   stale while that runtime is unreachable (§7 item 9).
6. **Panels and engineering:**
   - an on-screen keypad on touch screens: a number or text entry tapped opens a large keypad
     showing the allowed range, with OK and Cancel; with a keyboard nothing opens (decided on
     4 October 2026 while the user was away, to be approved: when the press that opened a number
     entry was a touch, the entry shows keys 0–9, a decimal point, ± and a backspace under its
     range, its field opens no device keyboard, and its Cancel and Send are the keypad's Cancel
     and OK, touch-sized; a mouse or pen press opens the entry as before. The display has no text
     entries yet, so only numbers have the keypad);
   - a day and night switch in the display's top bar that operators may use, remembered by each
     panel, the project's theme being the default (decided on 4 October 2026 while the user was
     away, to be approved: a round button before the session showing a sun in the night theme and
     a moon in the day theme, named Day or Night for screen readers; the browser keeps the choice
     as `trust-hmi-theme`; a screen without a top bar has no switch);
   - `trust-runtime hmi check`, the editor's checks over every HMI file, failing on an error, and
     `hmi test`, every page drawn in a headless browser and compared with its approved
     screenshot kept in the project, failing on a change, `--update` approving the new look;
     (`hmi check`, 4 October 2026: the deployment's checks of §16.2, `check_bundle`, against the
     project's program compiled with its libraries and the write policy of its `runtime.toml`;
     each problem as `file:line: message`, exit 1 on any, `--json` for tools. `hmi test`, decided
     the same day while the user was away, to be approved: each page laid out for its page area
     at the design size as the runtime display lays it out, drawn by the runtime's drawing module
     with the editor's design samples in the screen's theme, in headless Chromium (`--chromium`,
     `CHROMIUM`, the usual names on the path, or Playwright's); a page differs when more than
     0.2 % of its pixels differ by more than 24 in a channel; a changed or unapproved page fails
     and leaves `hmi/screens/<page>.actual.png`; `--update` writes `hmi/screens/<page>.png`);
   - an HMI Builder guide: getting started, the main tasks with pictures, every key.

Done when all of these hold:
- **Editing:** every action of deliverable 1 works by mouse and by keyboard, each as one
  undoable text edit, and a rendered test covers each.
- **Reuse:** a faceplate placed inline three times for three instances shows each instance's
  values and, changed once, changes all three; a visibility binding follows its variable on the
  display.
- **Operator displays:** a trend can be zoomed, panned, read with the cursor and exported; the
  alarm list sorts, filters and sounds; the showcase pages' screenshots pass review in both
  themes.
- **Operation:** the browser budget of §19.6 holds on the Pi 5 with no task over 50 ms in 20 s;
  the runtime budget holds on the AM6442 once the board is available.
- **Tags of other runtimes:** a page served by one runtime shows a variable another runtime
  publishes, picked in the variable picker like a local one, shows it stale when that runtime
  stops, and writes it when the owning runtime's policy allows the user's role, refused when it
  does not or the runtime is unreachable.
- **Panels and engineering:** a project's CI runs `hmi check` and `hmi test`, and they fail on a
  broken binding and on a changed screen.

Later, on demand: recipes, alarm notifications, reports, LDAP/AD and single sign-on, electronic
signatures. Not planned: general scripting (logic stays in Structured Text), redundancy, data
sources other than the program, a native mobile app.

## 19. Verification

### 19.1 Test-first

Every behaviour is test-first: the failing test before the code, the same test green after.

### 19.2 Required tests

- **Files:** TOML validation and canonical writing (Rust and TypeScript).
- **Drawing module:** unit and screenshot tests, including value states and staleness.
- **Page model:** add, move, resize, delete, copy, undo.
- **Editor reliability:**
  - text edits reflected on the canvas;
  - split views;
  - external changes with unsaved work;
  - restart recovery;
  - invalid intermediate text.
- **Runtime points:** nested members (function block instance members, structure fields, array
  elements).
- **Authorization:** policy, page permissions, roles and sessions, including deny-by-default.
- **Command lifecycle:** every outcome, duplicates, no replay after reconnect, pulse expiry
  without a browser.
- **Deployment:** validation, compatibility, atomic activation, rollback, out-of-date browsers.
- **Routing:** route computation.
- **Generation:** both directions, including repeated runs.
- **VS Code:** integration tests registered in `editors/vscode/src/test/suite/index.ts`.
- **Browser:** Playwright checks of deployed pages with live values, commands and failure cases.

### 19.3 Acceptance failure cases

Phase 1 acceptance shall include:
- a disconnect during a command;
- a denied write;
- an expired login;
- stale data;
- an invalid deployment;
- a rollback;
- rename recovery.

### 19.4 Same drawing

A set of reference pages shall render identically in the editor and in the runtime, compared by
screenshot:
- both surfaces draw each page at its design size and the same values. The reference program
  holds the editor's design samples (§3.4);
- the comparison runs in the dark and the light theme;
- a pixel may differ by rasterization noise alone: at most 32 of 255 in any colour channel, as
  gradient dithering and the edges of blur effects vary between documents (up to 28 measured).
  Any larger difference fails;
- a control proves the comparison can see a one-digit change in a value.

### 19.5 Visual review

Rendered screenshots of the editor and the runtime are reviewed for every visible change.

### 19.6 Performance budget

These are the initial budgets. Changing them requires a recorded measurement and an update of
this specification.

- **Runtime**, on the AM6442 product target, serving one page with 300 bound values at 4 updates
  per second (one every 250 ms, the stream's rate in §11.1 item 2) to three browsers. Changed
  with the user on 3 October 2026 from 10 updates per second, which the stream never sends:
  nobody reads numbers that change faster, the turning symbols are animated in the browser, and
  the product CPU is small.
  - adds at most 10 % CPU load;
  - leaves the PLC task's cycle timing within its declared budget.
- **Browser**, on the reference panel PC (Raspberry Pi 5 with Chromium at 1920×1080), showing
  that page with 30 animated symbols:
  - value-to-screen latency of at most 100 ms at the 95th percentile and 250 ms at the 99th;
  - no main-thread task longer than 50 ms during updates;
  - ordinary unchanged-state value cards may be coalesced and painted in bounded animation-frame
    batches (at most 96 cards per frame). The first batch shares the drawing turn rather
    than adding an empty frame; subsequent batches bound browser SVG layout work. Fresh stream updates supersede
    redundant timer redraws; stale/quiet pages still redraw and advance their ages. Changed text retains its DOM text node.
    Labelled bars reserve a separate row for stale ages above their track.
    A card changes its value and unit together; newer
    pending data replaces older pending data. Controls, alarm/quality changes, geometry and
    structural changes remain immediate. Cancellation drops pending paints. Latency evidence
    must cover completion of the entire value update, not only the first changed card;
  - at least 30 frames per second while animating;
  - page load of 2 s or less from a running runtime.

## 20. Decisions

Recorded with the user on 1 October 2026:
1. Customers build HMI pages by drag and drop in truST; the current HMI is replaced.
2. Starting from the program and starting from the HMI are both supported.
3. The complete layout is stored in TOML and must be Git-friendly.
4. Global variables are avoided; each equipment pair defines where new instances are declared
   and called.
5. truST ships equipment pairs (pump, valve, motor, tank, PID loop) to use as they are or to copy
   and adapt.
6. Pages are free and not tied to programs.
7. Equipment symbols are two-dimensional and based on P&ID symbols, and what moves in the plant
   moves in the symbol; a three-dimensional style, for example x-ray views that show the moving
   parts, may follow later. Changed with the user on 2 October 2026; it replaces "both symbol
   styles, flat and rendered, are chosen per project and made from Blender models", after the
   rendered pump and valve proof was rejected: it did not look professional, and a lit part or a
   glowing body is not how professionals show that equipment runs.
8. The editor draws with the same drawing module as the runtime.
9. Existing HMI pages are dropped; examples are rebuilt with the builder.
10. The components cover at least everything in the control course: its plants, instruments and
    controller structures.
11. The review of 1 October 2026 is adopted:
    - command outcomes;
    - runtime policy with page permissions that only narrow it;
    - deployment as a transaction;
    - nested bindings as foundation work;
    - a separate equipment behaviour specification;
    - measured, commanded, estimated and unknown states kept apart;
    - usability in phase 1;
    - milestones 1A to 1D;
    - the tank-level acceptance project;
    - "done when" criteria for every phase;
    - a measurable performance budget.
12. The legacy HMI code is deleted at the start of phase 1, before the new runtime display
    exists. Until milestone 1A delivers the runtime display, the runtime serves no HMI pages.
13. Alarm handling follows ISA-18.2 and EEMUA 191 (§13): every alarm is a function block instance
    in the program, with no alarm definitions in the HMI files; a shipped object-oriented base
    block holds the lifecycle, and customers write their own detection by extending it; three
    priorities, High, Medium and Low; information and trips are events, a trip's cause marked
    first out; the OpenOT records are the history; historian alert rules stay separate.
14. A separate alarm tutorial follows the alarm implementation (§13.9).

### Cross-runtime operation contract refinement — 4 October 2026

Qualified bindings keep the exact mesh runtime identity before `:`; the local IEC member path
remains case-insensitive. Only explicitly published paths are offered or accepted. The existing
mesh worker obtains typed catalogue entries and HMI samples from the owner, without network
work on the PLC task. Hmi.Display copies cached samples at its normal boundary. Initial remote
freshness bounds are stale after 3 seconds and unknown after 10 seconds; unavailable catalogue
metadata cannot establish a valid deployment. These are product limits, not timing qualification.

A local named login never authorizes a write on another runtime by itself. A control whose owner
has no session asks explicitly for a named login on that runtime. Credentials cross only an
existing mesh configured with both authentication and TLS. The serving runtime stores the owner
token privately, scoped to the local session and peer; no token is returned to the browser or
written to artifacts. The owner uses its normal SessionStore and its own write policy, typed
write checks, limits and audit. Originating controls also retain their existing local policy,
confirmation and audit. No command is automatically resent after login or a network timeout.

Remote write receipts bind the owner session, command ID and exact action/path. Reusing an ID
with different input fails; duplicate completed requests return the same result. Remote pulses
are initially bounded to 1 ms–60 seconds. Their successful start schedules their end inside the
owning Hmi.Display exchange, so connection loss cannot leave pulse termination to the sender.
If the owner's PLC task stops, no scan-bound operation is claimed to execute while stopped.

### Cold editor requests and command feedback (6 October 2026 repair contract)

Read-only project compilation and inspection shall not run on the language-server transport
task. Page parsing, visual edits and symbol editing shall remain responsive while PLC
metadata is being prepared. Results continue to use exact source snapshots and version-checked
edits; background work never applies a source edit itself.

An equipment popup with a bound settings instance uses that PLC instance name as its heading.
Known PLC rejection codes are presented as operator-readable explanations in notices and
control status; application-provided explanations remain intact. A refusal never retries a
command, acquires control or changes modes automatically.

Rendered fixture projects must call the existing Hmi.Display in a configured task, just as
customer projects do. Missing fixture setup is not a reason to change runtime behavior.

Concurrent metadata reads of identical source text shall reuse a pristine compilation; an
executed or mutably accessed runtime shall never be reused. Settings inspection may retain
two immutable results keyed by exact source text and the complete probe, including supplied
settings. Changed sources or settings must produce a fresh result.

Same-drawing comparison uses the builder's Live view and runtime view with the same recorded
runtime values. Static Build presentation is a separate acceptance case; matching live output
must not restore running equipment or sample values in Build mode.

The same-drawing harness fixes the browser clock to the same instant on both surfaces while
leaving PLC execution untouched. Time-axis labels must not invalidate an otherwise identical
frame. The drag capture expects alignment-guide snapping, not the raw unsnapped pointer delta.

The comparison supplies identical live format metadata as well as values. Its reference screen
explicitly disables navigation to reserve the full design-size drawing area; floating session
chrome is outside the drawing-only crop, with authentication/role presentation verified by
the separate browser suite. No page SVG content is masked. A deliberate live-value change
must be detected, and the original raster-noise tolerance remains unchanged.

### Cold metadata compilation work (6 October 2026 performance repair)

Cold equipment discovery and settings inspection must resolve imported types from the
source ranges already recorded in the revision-matched project type catalog, rather than
searching the complete library syntax tree for every type. Declaration precollection skips
executable statement trees; constant initializers retain their original namespace, POU and
method scopes and declaration order. Source edits still invalidate Salsa inputs normally.
These are compiler work reductions, not changes to IEC language or PLC execution semantics.
Provider methods continue to execute in the existing isolated driverless inspection path;
no static approximation of customer settings and no customer program/task execution is allowed.
Measure the unchanged cold pump-inspector journey and record its actual duration against the
137.376-second baseline; repeated cached reads must not be reported as cold measurements.

The constant-location prelude is tracked per source revision. Project symbol collection and
analysis reuse these locations instead of walking every project's syntax tree for each file.
Reconstruction uses the original syntax nodes and positions, preserving constant scope,
first-declaration precedence and diagnostics. Editing or removing a declaration invalidates
its prelude through the existing Salsa dependencies; a cache hit cannot retain an old value.

Project-wide VAR_CONFIG validation reuses the source-revision list of files containing
configuration initializers, preserving their complete original roots and ranges. Task-sharing
hazard analysis runs only for a file owning globals: its existing diagnostics are emitted at
that owner, never at an importing file. These reductions do not skip a diagnostic or change
PLC behavior. Native source-edit/configuration/hazard tests and the real cold equipment
inspector establish correctness and measured speed separately.

The compiler's optional OpenOT preprocessing must not perform semantic project analysis for
ordinary HMI projects without OpenOT attributes. Attributed projects share one authoring
model for validation and instrumentation (specification 27, compiler preparation cost).
The HMI probe still uses ordinary isolated compilation/execution and retains all diagnostics.

### Diagnostic timing logs

`TRUST_COMPILER_TRACE=1` enables opt-in structured timing on the existing logging sink
(stderr for the language server). Each synchronous request/stage has a unique ID, parent
span context, begin/end events and elapsed microseconds, including early returns/unwinds.
Log request names, source counts/bytes, cache hits/misses/waits/invalidation, OpenOT preparation,
semantic analysis, symbol merging, lowering/initialization and isolated settings-probe execution.
Do not log source text, PLC values, credentials or command arguments. Logging is off by default.
Timing logs are diagnostic evidence, not a replacement for native behavior or rendered tests.

## 21. Rebuild contract — 7 October 2026

Agreed with the user on 7 October 2026. This section supersedes any earlier statement of this
specification that contradicts it.

### 21.1 One page format

1. Screen, pages, popups, panels and component definitions have one format. The editor writes
   it, the runtime serves it, and `trust-runtime hmi check` and the language server check it,
   all through the `trust-runtime` page loader. Deploying sends the files exactly as written;
   there is no conversion step.
2. An equipment object carries `block`, `instance` and `faceplate`. `block` records the
   instance's function block type so that a design whose PLC code is not created yet knows what
   to generate; when the instance is declared, a different declared type is an error at the
   `block` line.
3. A docked panel (`[panel]` with `scope`, `pages`, `dock`, `order`) is composed into every page
   it applies to by the loader, identically for the runtime display, `hmi check` and the editor.
   A docked panel binds no instance.
4. Reusable sections live in `hmi/panels/` (project) and `<library>/hmi/panels/`.
   The former `hmi/faceplates/` folder is no longer read.
5. Create HMI and the agent tools write pages through the same canonical page writer.

### 21.2 Commands that call methods

1. `[object.command]` names `interface`, `method`, `instance` (`"$instance"` in a panel, or
   `"$instance.<member>"` for a block inside it) and optional `[object.command.arguments]`: fixed
   values, or `{ input = true }` for the value the control sends. The object's `role`
   (`operator` by default, or `engineer`) is the role the command needs, as for any control.
2. The runtime requires a named HMI session with the command's role; checks the object's
   `interlock` permit and refuses with its `interlock-reason`; requires the instance to
   implement the interface and the interface to declare the method; converts and checks inputs
   (a mode selector's value must be one of its options, a number must be finite and within the
   control's limits); and calls the method at the next display boundary between scans. The
   method's BOOL result is the outcome: accepted or refused. No request number, owner or
   context reaches PLC code.
3. A refused command shows "Not accepted: <reason>" beside the control, with the device's
   `CommandText` as the reason; an accepted one is confirmed by its bound feedback (§12.5).
4. A control that calls a method writes nothing through its bindings: they show the program's
   values (the setpoint's `value`, the mode selector's `mode`), so they may only read, also
   where the control writes its binding otherwise. The binding checks of the runtime,
   `hmi check` and the editor share this rule.

### 21.3 Builder

1. **Palette:** Equipment, Instruments, Controllers, Groups and sequences first, each component
   with a definition listed by what it is (a controller symbol is a controller, drawn equipment is
   equipment, a measurement is an instrument, anything else a group, sequence or condition); then
   the general elements: Basic shapes, Symbols (equipment pictures without a definition), My
   Symbols (§14.4), Displays, Controls, Navigation.
2. **Inspector** of an equipment object shows only:
   - the tag (the instance name; editable while the instance is new);
   - Hardware: Connected, Partly connected, Not connected or Check connections, with one button
     (Connect hardware, or Edit hardware connections) that opens Devices & Connections focused
     on the instance;
   - the definition's settings with their real values: the declaration's initialiser or the
     block's default for a declared instance, the `plant.toml` choice or the default for a new
     one. The key settings show at once and the others under a closed More settings, so every
     setting the runtime's Settings dialog has can also be set before the PLC code runs (refined
     on 7 October 2026). No generic explanation, no per-field links.
3. A just-dropped instance shows its settings and hardware terminals at once, from its
   definition, before any PLC code exists.
4. Every canvas edit reaches the document in order. An edit made while an earlier one is being
   applied is queued, never dropped, and no "edits were preserved" message is shown for an edit
   that was in fact applied.
5. The toolbar never wraps: the page title truncates with an ellipsis and shows its full name on
   hover; below 1100 px the less frequent actions move into a More menu.
6. Canvas shortcuts (arrows, Delete, Ctrl+Z, …) act only while the canvas has focus; while a
   button, field or menu has focus, the keys belong to it.
7. Live mode also bridges the settings route (§21.4).
8. **Opening the HMI:** the truST sidebar has an HMI destination beside Devices, and the command
   palette has *truST: Open HMI*. Both open the project's start page (`start_page` in
   `hmi/screen.toml`, else the first page by name, else the screen layout) in the HMI Builder;
   a project without `hmi/screen.toml` gets the new-screen dialog instead. Without an open
   project the sidebar shows the destination disabled.

### 21.4 Settings dialog

1. Engineers only: the Settings button appears in an Engineer session, and the route refuses
   other roles.
2. It lists the definition's `[[settings]]` in order: plain title, value with its unit, and a
   help line only where the definition gives one.
3. Apply and Cancel stay visible at the bottom at every window size; the result ("Applied", or
   the refusal naming the field) appears beside Apply.
4. `POST /api/hmi/settings`:
   - `{ "faceplate": <address>, "action": "read" }` returns the settings and current values;
   - `{ "faceplate", "action": "apply", "changes": {member: value}, "expected": {member: value} }`
     checks the role, each value's type and limits, and that the current values still equal
     `expected` (else "Changed meanwhile: refresh", keeping the draft); then writes all changes
     at one boundary between scans, audits each, and returns the values read back.
5. There is no provider, revision, request number, import, export or defaults transaction.
6. **Keep in project** (decided with the user on 7 October 2026): opened in the HMI Builder's
   Live mode, the dialog also has Keep in project, beside Apply. It keeps the values the PLC has,
   as last read or applied, not an edit that is not applied: those that differ from the project's
   values are written as one project edit, as a change in the inspector is (§21.3 item 2), so
   commissioning ends with the values in `plant.toml` and the generated declaration. Equipment
   declared by hand is refused with "copy the values there". A browser shows no such button.

### 21.5 Runtime fixes

1. **Pulses** always end: the runtime writes the end after the pulse duration even when the
   browser disconnects, the session ends or a write fails, retrying at each display boundary
   until written, and audits it.
2. **Replay records** are created only after a command is authorized, and are counted per user
   (at most 64 open per user); one user can no longer exhaust capacity for others.
3. **Cross-site check:** every HMI write route (login, logout, command, alarm, settings, deploy,
   rollback) requires `Content-Type: application/json` and, when the request carries `Origin`,
   the runtime's own origin; otherwise it answers 403 without effect, as the other write routes
   do.
4. **Bundle:** the active bundle is read and hashed once when activated (start, deploy,
   rollback) and again only when its files change; requests use that snapshot.

### 21.6 Attached instruments

An instrument attached to a vessel or pipe (a level transmitter on a tank) draws its
attachment line to the host's drawn outline, not to the host's bounding box, so it touches the
tank wall for every tank shape and size.

### 21.7 Verification

VS Code tests cover each editor item (ordered edits, Live settings, a just-dropped pump's
settings, the toolbar, keyboard focus, attachment); runtime tests cover each runtime item and
the one page format; language-server tests cover Create PLC code and Create HMI; browser
screenshots of the pump popup and the Settings dialog at 1280 × 800, as Operator and as
Engineer, are reviewed before acceptance.

### 21.8 Details decided while building (7 October 2026)

1. **Method commands:** the role is the higher of the object's `role` and the role of a write
   rule that covers the instance; no rule is needed. A number is also checked against the values
   the control's `low-limit` and `high-limit` bindings read, and a bound limit that cannot be read
   refuses. A method that answers FALSE without a `CommandText` is refused as "not accepted".
2. **Replay records:** a repeat is looked up before the checks, a record is made only after them.
   A user keeps at most 64 records: the oldest answered one makes room, and with 64 still open a
   new command is refused. A record can so be forgotten before ten minutes, never while open.
3. **Settings route:** reading also needs an Engineer. Each setting is answered as the inspector
   shows it: `member`, `title`, `unit`, `min`, `max`, `step`, `choices`, `help`, `key`, `kind`
   (`bool`, `integer`, `real`, `time`, `text`, `other`) and `value` in its unit; a refusal is
   `{ ok: false, error, field?, changed? }`. After "Changed meanwhile: refresh" the dialog reads
   the values again for the fields not edited and keeps the edits.
4. **Popup title:** "P101 — Pump" takes the component definition's `title`, else the panel's
   title; a panel's own `description` stays below. Every refusal reads "Not accepted: <reason>".

### 21.9 My equipment

Decided with the user on 7 October 2026: a customer makes their own equipment from their own
function block without writing a definition by hand.

1. **Listed at once:** each function block declared in the project's own sources, not in a
   library, without a component definition, appears in the Components panel under My equipment,
   after Groups and sequences. Its proposed definition shows the plain `device` picture (§5.2),
   whose lamp follows a BOOL `Running`, and an automatic panel. Blocks the project has defined
   itself are listed there too.
2. **Placed:** placing a proposed block writes its definition to `hmi/pairs/<name>.toml` and its
   panel to `hmi/panels/<name>.toml` (`<name>` is the block's name in lower case with hyphens,
   `FB_Mixer` gives `fb-mixer`), both saved. The panel shows the block's outputs and `PUBLIC`
   members, BOOL ones as lamps (in the alarm colour when the name says fault, alarm, trip or
   error) and the others as values, then a button for each method without inputs giving BOOL
   that an interface the block implements declares (Stop drawn as a stop button); with no outputs
   it shows the inputs. Members declared `PUBLIC` in a `RETAIN` section are its settings, offered
   in the panel's Settings dialog. Names become words: `MixTime` is "Mix time", `PIDOutput`
   "PID output".
3. **Extended blocks:** a block that extends one with a definition takes that definition for
   itself (its picture, panel, settings, hardware terminals and dependencies), titled by its own
   name, and is given no panel of its own.
4. **Picture:** the inspector of the project's own definition offers Shown as: the plain device
   or one of My Symbols (§14.4), with Import SVG to add a drawing. Choosing one writes it to the
   definition, and every object of the block on the project's pages and popups shows it.
5. **Inputs:** each input of the picture is driven by a member of the block that holds a kind of
   value the input takes: by default the member of the same name (compared without case, hyphens
   or underscores), else the one chosen in the inspector, or none ("Not shown"). A definition in a
   library is the library's and is not changed from the project.
6. **Language server:** `trust-lsp.hmiOwnEquipment` proposes the definitions, as the editor reads
   definitions, gives a block's files and changes a picture; the editor writes what it gives.
