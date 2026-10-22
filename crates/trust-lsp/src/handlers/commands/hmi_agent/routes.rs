//! A pipe's route (specification 39, §6 item 3): orthogonal, around the objects, with few bends,
//! leaving each port the way it faces and through the bends the user placed.
//!
//! Port of `editors/vscode/src/hmiBuilder/drawing/pipes/route.ts` (`routePipe`, `findRoute`, the
//! search over the grid of the lines through the stops and the objects' edges, and the heap) and of
//! `pipeRoutes` and `solidBoxes` in `drawing/pipes/draw.ts`. The routes are inspected by findings
//! that are stored and reported, so the route is logic (AGENTS.md). Left out: the route cache of
//! `route.ts`, which only saves recomputing a route it has already computed, and the drawing of
//! `draw.ts` (SVG paths, colours, stroke order and junction glyphs).

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use trust_runtime::hmi::page_files::{HmiObject, HmiPage};

use super::geometry::{coord, Rect};
use super::ports::{
    label_height, port_point, round_hundredths, symbol_geometry, symbol_placement, PortDirection,
    PortPoint, SymbolLabel,
};

/// A point of a route, in page pixels (`Point` in `route.ts`).
pub(super) type Point = [f64; 2];

/// How far a pipe runs straight out of its port before it may turn (`STUB`).
const STUB: f64 = 16.0;
/// The room a pipe keeps from an object (`CLEARANCE`).
const CLEARANCE: f64 = 8.0;
/// What a bend costs, in pixels of length (`BEND`).
const BEND: f64 = 40.0;

/// The directions in the order the search numbers them (`DIRECTIONS` in `route.ts`).
const DIRECTIONS: [PortDirection; 4] = [
    PortDirection::Left,
    PortDirection::Right,
    PortDirection::Up,
    PortDirection::Down,
];

/// Each pipe's route on the page, in the pipes' order; `None` for a pipe with an end that is not a
/// port of the page, which the Problems view reports (`pipeRoutes` in `drawing/pipes/draw.ts`).
#[must_use]
pub(super) fn pipe_routes(page: &HmiPage, size: [u32; 2]) -> Vec<Option<Vec<Point>>> {
    let obstacles: Vec<Rect> = page.objects.iter().flat_map(solid_boxes).collect();
    page.pipes
        .iter()
        .map(|pipe| {
            let from = end_port(page, &pipe.from)?;
            let to = end_port(page, &pipe.to)?;
            let waypoints: Vec<Point> = pipe
                .waypoints
                .iter()
                .map(|point| [coord(point[0]), coord(point[1])])
                .collect();
            Some(route_pipe(from, to, &obstacles, &waypoints, size))
        })
        .collect()
}

/// What a pipe keeps clear of: a symbol and its label, or a whole object; nothing of a junction
/// (`solidBoxes` in `drawing/pipes/draw.ts`). A rotated symbol, or one with its label beside it,
/// stands in its whole box.
#[must_use]
fn solid_boxes(object: &HmiObject) -> Vec<Rect> {
    if object.kind == "junction" {
        return Vec::new();
    }
    let whole = Rect {
        x: coord(object.at[0]),
        y: coord(object.at[1]),
        width: f64::from(object.size[0]),
        height: f64::from(object.size[1]),
    };
    let Some(definition) = symbol_geometry(&object.kind) else {
        return vec![whole];
    };
    if definition.label == SymbolLabel::Beside || !object.rotation.is_multiple_of(360) {
        return vec![whole];
    }
    // `solidBoxes` places the symbol with the default label, under it.
    let placement = symbol_placement(object.size, definition.size, SymbolLabel::Under);
    vec![
        Rect {
            x: whole.x + placement.left,
            y: whole.y + placement.top,
            width: f64::from(definition.size[0]) * placement.scale,
            height: f64::from(definition.size[1]) * placement.scale,
        },
        Rect {
            x: whole.x,
            y: whole.y + placement.area,
            width: whole.width,
            height: label_height(),
        },
    ]
}

/// The port a pipe's end names, `object.port`, or `None` when no object of the page has it. The
/// port name follows the last dot (`end` in `drawing/pipes/draw.ts`).
fn end_port(page: &HmiPage, written: &str) -> Option<PortPoint> {
    let at = written.rfind('.')?;
    if at == 0 {
        return None;
    }
    let object = page
        .objects
        .iter()
        .find(|candidate| candidate.id == written[..at])?;
    port_point(object, &written[at + 1..])
}

/// The route from one port to another, through the bends the user placed, as the points of its line
/// (`routePipe`). `bounds` is the page's design size.
#[must_use]
fn route_pipe(
    start: PortPoint,
    end: PortPoint,
    obstacles: &[Rect],
    waypoints: &[Point],
    bounds: [u32; 2],
) -> Vec<Point> {
    simplified(find_route(start, end, obstacles, waypoints, bounds))
}

/// The route before its repeats are removed (`findRoute`).
fn find_route(
    start: PortPoint,
    end: PortPoint,
    obstacles: &[Rect],
    waypoints: &[Point],
    bounds: [u32; 2],
) -> Vec<Point> {
    let boxes: Vec<Rect> = obstacles.iter().map(|body| inflated(*body)).collect();
    // Two ports facing each other on one clear line are joined straight.
    if waypoints.is_empty()
        && facing(start, end)
        && !boxes.iter().any(|body| crosses([start.at, end.at], *body))
    {
        return vec![start.at, end.at];
    }
    let exit = moved(start.at, start.direction, STUB);
    let entry = moved(end.at, end.direction, STUB);
    // Only the boxes that still enclose the outward stub are left out. A symbol's body remains an
    // obstacle: its port may touch the body without permitting a route through it.
    let solid: Vec<Rect> = boxes
        .into_iter()
        .filter(|body| ![exit, entry].iter().any(|point| inside(*point, *body)))
        .collect();
    let mut stops = vec![exit];
    stops.extend_from_slice(waypoints);
    stops.push(entry);
    let grid = Grid::new(&stops, &solid, [f64::from(bounds[0]), f64::from(bounds[1])]);
    let last_leg = stops.len() - 2;
    let mut path = vec![start.at];
    let mut heading = start.direction;
    for leg in 0..=last_leg {
        let arriving = if leg == last_leg {
            Some(opposite(end.direction))
        } else {
            None
        };
        let Some(found) = grid.search(stops[leg], stops[leg + 1], heading, arriving) else {
            // No way through the grid: the route runs out of both ports and turns once.
            return vec![start.at, exit, [entry[0], exit[1]], entry, end.at];
        };
        path.extend(found.points);
        heading = found.heading;
    }
    path.push(end.at);
    path
}

/// A box grown by the clearance on every side.
fn inflated(body: Rect) -> Rect {
    Rect {
        x: body.x - CLEARANCE,
        y: body.y - CLEARANCE,
        width: body.width + 2.0 * CLEARANCE,
        height: body.height + 2.0 * CLEARANCE,
    }
}

/// Whether the start leaves its port towards the end, which faces it on the same line (`facing`).
fn facing(start: PortPoint, end: PortPoint) -> bool {
    let (dx, dy) = (end.at[0] - start.at[0], end.at[1] - start.at[1]);
    if start.direction != opposite(end.direction) {
        return false;
    }
    let (sx, sy) = step(start.direction);
    (sx != 0 && dy == 0.0 && sign(dx) == sx) || (sy != 0 && dx == 0.0 && sign(dy) == sy)
}

/// `Math.sign`: -1, 0 or 1.
fn sign(value: f64) -> i8 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// The point a distance out of a port, in the direction the port faces (`moved`).
fn moved(point: Point, direction: PortDirection, distance: f64) -> Point {
    let (sx, sy) = step(direction);
    [
        point[0] + f64::from(sx) * distance,
        point[1] + f64::from(sy) * distance,
    ]
}

/// The unit step of a direction on the page (`STEPS` in `route.ts`).
fn step(direction: PortDirection) -> (i8, i8) {
    match direction {
        PortDirection::Left => (-1, 0),
        PortDirection::Right => (1, 0),
        PortDirection::Up => (0, -1),
        PortDirection::Down => (0, 1),
    }
}

/// The direction opposite to another (`OPPOSITE` in `route.ts`).
fn opposite(direction: PortDirection) -> PortDirection {
    match direction {
        PortDirection::Left => PortDirection::Right,
        PortDirection::Right => PortDirection::Left,
        PortDirection::Up => PortDirection::Down,
        PortDirection::Down => PortDirection::Up,
    }
}

/// A direction's number in `DIRECTIONS`.
fn direction_index(direction: PortDirection) -> usize {
    match direction {
        PortDirection::Left => 0,
        PortDirection::Right => 1,
        PortDirection::Up => 2,
        PortDirection::Down => 3,
    }
}

/// Whether a point lies strictly inside a box; its edges do not count (`inside`).
fn inside(point: Point, body: Rect) -> bool {
    point[0] > body.x
        && point[0] < body.x + body.width
        && point[1] > body.y
        && point[1] < body.y + body.height
}

/// Whether the bounding box of a segment overlaps a box, edges not counting (`crosses`).
fn crosses(segment: [Point; 2], body: Rect) -> bool {
    let [first, second] = segment;
    let (low_x, high_x) = (first[0].min(second[0]), first[0].max(second[0]));
    let (low_y, high_y) = (first[1].min(second[1]), first[1].max(second[1]));
    low_x < body.x + body.width
        && high_x > body.x
        && low_y < body.y + body.height
        && high_y > body.y
}

/// The distance along the page's two axes between two points (`distance`).
fn distance(from: Point, to: Point) -> f64 {
    (from[0] - to[0]).abs() + (from[1] - to[1]).abs()
}

/// Whether a box edge lies within the page on its axis (`within`).
fn within(value: f64, limit: f64) -> bool {
    (0.0..=limit).contains(&value)
}

/// The distinct values, each rounded to two decimals, in ascending order (`unique`).
fn unique(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut rounded: Vec<f64> = values.map(round_hundredths).collect();
    rounded.sort_by(f64::total_cmp);
    rounded.dedup();
    rounded
}

/// The position of a stop's coordinate among the grid's values. The stops are always among the
/// values, so the search finds each one (`indexOf` in `route.ts`).
fn index_of(values: &[f64], value: f64) -> usize {
    match values.binary_search_by(|probe| probe.total_cmp(&value)) {
        Ok(index) | Err(index) => index,
    }
}

/// The neighbouring cell in a direction, when it lies within the grid.
fn neighbour(
    column: usize,
    row: usize,
    width: usize,
    height: usize,
    direction: PortDirection,
) -> Option<(usize, usize)> {
    match direction {
        PortDirection::Left => column.checked_sub(1).map(|next| (next, row)),
        PortDirection::Right => (column + 1 < width).then_some((column + 1, row)),
        PortDirection::Up => row.checked_sub(1).map(|next| (column, next)),
        PortDirection::Down => (row + 1 < height).then_some((column, row + 1)),
    }
}

/// The points without repeats and without the middle of a straight run (`simplified`).
#[allow(clippy::float_cmp)] // Route coordinates are exact grid values: equal coordinates are one stop.
fn simplified(points: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        if kept.last() == Some(&point) {
            continue;
        }
        let collinear = match kept.as_slice() {
            [.., before, last] => {
                (before[0] == last[0] && last[0] == point[0])
                    || (before[1] == last[1] && last[1] == point[1])
            }
            _ => false,
        };
        if collinear {
            kept.pop();
        }
        kept.push(point);
    }
    kept
}

/// The result of one leg of the search: the grid points from its start to its goal, and the heading
/// it arrives with.
struct Found {
    /// The grid points, the start first.
    points: Vec<Point>,
    /// The direction the route is moving in at the goal.
    heading: PortDirection,
}

/// The lines through the stops and the objects' edges, within the page: the cells the route is
/// searched over (`Grid` in `route.ts`). A cell is a column and a row; the state of a search is a
/// cell and the direction it was entered in.
struct Grid {
    /// The distinct x values of the grid, ascending.
    xs: Vec<f64>,
    /// The distinct y values of the grid, ascending.
    ys: Vec<f64>,
    /// The solid boxes a route keeps out of.
    boxes: Vec<Rect>,
}

impl Grid {
    /// The grid through the stops and the edges of the solid boxes that lie within the page.
    fn new(stops: &[Point], boxes: &[Rect], bounds: [f64; 2]) -> Self {
        let xs = unique(
            stops.iter().map(|point| point[0]).chain(
                boxes
                    .iter()
                    .flat_map(|body| [body.x, body.x + body.width])
                    .filter(|x| within(*x, bounds[0])),
            ),
        );
        let ys = unique(
            stops.iter().map(|point| point[1]).chain(
                boxes
                    .iter()
                    .flat_map(|body| [body.y, body.y + body.height])
                    .filter(|y| within(*y, bounds[1])),
            ),
        );
        Self {
            xs,
            ys,
            boxes: boxes.to_vec(),
        }
    }

    /// The cheapest way from one stop to the next, starting with a heading, with a bend's cost when
    /// it does not arrive with the heading asked for (`search`). Each step costs its length, each
    /// bend 40 px, and a route that arrives at the goal other than as asked pays one more bend.
    fn search(
        &self,
        from: Point,
        to: Point,
        heading: PortDirection,
        arriving: Option<PortDirection>,
    ) -> Option<Found> {
        let width = self.xs.len();
        let height = self.ys.len();
        let goal = self.node(to);
        let state = |cell: usize, direction: usize| cell * 4 + direction;
        let first = state(self.node(from), direction_index(heading));
        let mut best: HashMap<usize, f64> = HashMap::from([(first, 0.0)]);
        let mut parent: HashMap<usize, usize> = HashMap::new();
        let mut queue = Queue::default();
        queue.push(first, distance(from, to));
        while let Some(current) = queue.pop() {
            let (cell, direction) = (current / 4, current % 4);
            let cost = best.get(&current).copied().unwrap_or(f64::INFINITY);
            if cell == goal {
                return Some(Found {
                    points: self.trace(&parent, current),
                    heading: DIRECTIONS[direction],
                });
            }
            let (column, row) = (cell % width, cell / width);
            for (next_index, next) in DIRECTIONS.into_iter().enumerate() {
                let Some((next_column, next_row)) = neighbour(column, row, width, height, next)
                else {
                    continue;
                };
                let next_cell = next_column + next_row * width;
                let (here, target) = (self.point(cell), self.point(next_cell));
                if !self.open(here, target) {
                    continue;
                }
                // A pipe never doubles back on itself.
                if next == opposite(DIRECTIONS[direction]) {
                    continue;
                }
                let turn = if next == DIRECTIONS[direction] {
                    0.0
                } else {
                    BEND
                };
                let arrival = if next_cell == goal && arriving.is_some_and(|wanted| next != wanted)
                {
                    BEND
                } else {
                    0.0
                };
                let next_state = state(next_cell, next_index);
                let next_cost = cost + distance(here, target) + (turn + arrival);
                if next_cost < best.get(&next_state).copied().unwrap_or(f64::INFINITY) {
                    best.insert(next_state, next_cost);
                    parent.insert(next_state, current);
                    queue.push(next_state, next_cost + distance(target, to));
                }
            }
        }
        None
    }

    /// The grid points of the path to a state, from the start of the search (`search`'s
    /// reconstruction).
    fn trace(&self, parent: &HashMap<usize, usize>, goal_state: usize) -> Vec<Point> {
        let mut points = Vec::new();
        let mut at = Some(goal_state);
        while let Some(state) = at {
            points.push(self.point(state / 4));
            at = parent.get(&state).copied();
        }
        points.reverse();
        points
    }

    /// The cell of a stop: its column and its row among the grid's values.
    fn node(&self, point: Point) -> usize {
        let column = index_of(&self.xs, round_hundredths(point[0]));
        let row = index_of(&self.ys, round_hundredths(point[1]));
        column + row * self.xs.len()
    }

    /// The point of a cell: its column's x and its row's y (`point` in `route.ts`).
    fn point(&self, cell: usize) -> Point {
        let width = self.xs.len();
        [self.xs[cell % width], self.ys[cell / width]]
    }

    /// Whether a step between two neighbouring grid points stays out of every solid box: neither its
    /// middle nor its far end lies inside one (`open`).
    fn open(&self, from: Point, to: Point) -> bool {
        let middle = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
        !self
            .boxes
            .iter()
            .any(|body| inside(middle, *body) || inside(to, *body))
    }
}

/// The states waiting to be expanded, the cheapest first; among equal priorities the one pushed
/// first comes out first (`Heap` in `route.ts`).
#[derive(Default)]
struct Queue {
    /// The waiting states, ordered by `Queued`.
    heap: BinaryHeap<Queued>,
    /// How many states have been pushed, which orders equal priorities.
    pushed: u64,
}

impl Queue {
    /// Adds a state with its priority.
    fn push(&mut self, state: usize, priority: f64) {
        self.heap.push(Queued {
            priority,
            order: self.pushed,
            state,
        });
        self.pushed += 1;
    }

    /// Takes the cheapest state out.
    fn pop(&mut self) -> Option<usize> {
        self.heap.pop().map(|queued| queued.state)
    }
}

/// A state with its priority and the order it was pushed in.
#[derive(Debug, Clone, Copy)]
struct Queued {
    /// The estimated cost through the state; lower comes out first.
    priority: f64,
    /// The push order; among equal priorities, lower comes out first.
    order: u64,
    /// The search state.
    state: usize,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(&self, other: &Self) -> Ordering {
        // The heap pops its greatest item, so the lower priority, and among equal priorities the
        // earlier push, must compare as the greater.
        other
            .priority
            .total_cmp(&self.priority)
            .then_with(|| other.order.cmp(&self.order))
    }
}

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
