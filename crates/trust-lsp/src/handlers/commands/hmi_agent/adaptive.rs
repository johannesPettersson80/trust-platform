//! The adaptive layout of a page for an area smaller than its design size (specification 39, §4
//! item 5): at its design size when the area is at least that large, else laid out again in blocks
//! at the area's width, each block at scale 1 or scaled, and arranged containers rearranged.
//!
//! Port of `editors/vscode/src/hmiBuilder/drawing/adaptive.ts`: `adaptPage` and the layout it calls.
//! Its drawing is not ported. The adapted page keeps its pipes, whose bends move with the process
//! graphic as the `TypeScript` moves them. The layout works in exact pixels as the `TypeScript` does;
//! `AdaptedPage::boxes` carries the exact drawn boxes, because an arranged container narrower than
//! its margins has a negative width that an `HmiObject`'s unsigned size cannot hold.

use std::cmp::Ordering;
use std::collections::HashMap;

use trust_runtime::hmi::page_files::{HmiArrange, HmiObject, HmiPage};

use super::geometry::{contains, coord, drawn_box, js_round, overlaps, Rect};
use super::ports::symbol_geometry;

/// The space between blocks, and an arranged container's padding and default gap (`SPACE`).
const SPACE: f64 = 16.0;
/// The room a titled panel or group keeps above its objects for its title: the 44 px header
/// (`TITLE`, §8.9).
const TITLE: f64 = 44.0;
/// The least width and height of a touch target (`TOUCH`, §4 item 2).
const TOUCH: f64 = 44.0;
/// The components an operator touches, which keep their touch size (`CONTROLS`).
const CONTROLS: [&str; 9] = [
    "button",
    "pulse-button",
    "switch",
    "selector",
    "mode-selector",
    "slider",
    "numeric-input",
    "setpoint",
    "page-button",
];

/// The page as a viewport shows it (`AdaptedPage` in `adaptive.ts`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AdaptedPage {
    /// The page with its objects at their adapted positions and its pipes' bends moved. Positions
    /// are whole pixels; a size is zero where the layout gives a negative one (see `boxes`).
    pub(super) page: HmiPage,
    /// The content size in pixels, width then height.
    pub(super) size: [f64; 2],
    /// Whether the page is scaled uniformly to fit the area: the area is at least the design size.
    pub(super) fit: bool,
    /// Each object's drawn box after the layout, in the order of `page.objects`, in exact pixels.
    pub(super) boxes: Vec<Rect>,
}

/// An object's placement in the adapted layout: its position and size in exact pixels.
#[derive(Debug, Clone, Copy)]
struct Placed {
    /// The position of the top-left corner.
    at: [f64; 2],
    /// The width and height.
    size: [f64; 2],
}

/// How a block's design points move into its laid position, relative to the block's corner.
#[derive(Debug, Clone, Copy)]
enum Relative {
    /// Scaled by `scale` about the corner, each point rounded to whole pixels (`scaled`).
    Scaled {
        /// The block's corner in the design.
        corner: [f64; 2],
        /// The factor of the scaling.
        scale: f64,
    },
    /// Moved so that the block's corner is the origin (`arranged`).
    Shifted {
        /// The container's corner in the design.
        corner: [f64; 2],
    },
}

impl Relative {
    /// A design point, as its place relative to the block's corner.
    fn apply(self, point: [f64; 2]) -> [f64; 2] {
        match self {
            Self::Scaled { corner, scale } => [
                js_round((point[0] - corner[0]) * scale),
                js_round((point[1] - corner[1]) * scale),
            ],
            Self::Shifted { corner } => [point[0] - corner[0], point[1] - corner[1]],
        }
    }
}

/// A block laid out at its size: where each of its objects goes, and how design points move
/// (`Laid` in `adaptive.ts`).
struct Laid {
    /// The width of the block.
    width: f64,
    /// The height of the block.
    height: f64,
    /// Each object of the block by id, relative to the block's corner.
    objects: HashMap<String, Placed>,
    /// How a design point of the block moves relative to its corner.
    relative: Relative,
}

/// An object with what lies inside it, which moves and scales with it (`Item` in `adaptive.ts`).
struct Item<'a> {
    /// The object.
    object: &'a HmiObject,
    /// What lies wholly inside it, in page order.
    inside: Vec<Item<'a>>,
}

/// A block of the page: a top-level container with what lies inside it, an object on its own, or
/// the process graphic (`Block` in `adaptive.ts`).
struct Block<'a> {
    /// The container, when the block is one.
    container: Option<Item<'a>>,
    /// The objects of a block that is not a container.
    items: Vec<Item<'a>>,
    /// The block's box in the design.
    rect: Rect,
    /// Whether the block is the process graphic, whose pipes' bends move with it.
    graphic: bool,
}

/// The page as it is laid out for an area of `area` pixels, where `design` is its design size
/// (`adaptPage`). An area at least the design size keeps the design, scaled to fit; a narrower area
/// lays the blocks out again at its width.
#[must_use]
pub(super) fn adapt_page(page: &HmiPage, design: [u32; 2], area: [u32; 2]) -> AdaptedPage {
    let design_size = [f64::from(design[0]), f64::from(design[1])];
    let fits = area[0] >= design[0] && area[1] >= design[1];
    // Shared regions reserve real screen space: their authored layout is kept and scrolls when
    // needed, since blocks cannot move a page object into its shared sidebar.
    if !page.regions.is_empty() || fits {
        return unchanged(page, design_size, fits);
    }
    if area[0] >= design[0] {
        return unchanged(page, design_size, false);
    }
    laid_out_at_width(page, area)
}

/// The page at its design size, with the drawn box of each object (`{ page, size, fit }` in
/// `adaptPage`).
fn unchanged(page: &HmiPage, size: [f64; 2], fit: bool) -> AdaptedPage {
    AdaptedPage {
        page: page.clone(),
        size,
        fit,
        boxes: page.objects.iter().map(drawn_box).collect(),
    }
}

/// The page laid out again at the width of the area, its blocks placed in reading order, and as
/// tall as they need (the layout part of `adaptPage`).
fn laid_out_at_width(page: &HmiPage, area: [u32; 2]) -> AdaptedPage {
    let width = f64::from(area[0]);
    let height = f64::from(area[1]);
    let blocks = blocks_of(page);
    if blocks.is_empty() {
        return unchanged(page, [width, height], false);
    }
    let margin = blocks
        .iter()
        .map(|block| block.rect.x)
        .fold(f64::INFINITY, f64::min)
        .max(0.0);
    let top = blocks
        .iter()
        .map(|block| block.rect.y)
        .fold(f64::INFINITY, f64::min)
        .max(0.0);
    let mut placed: Vec<Rect> = Vec::new();
    let mut objects: HashMap<String, Placed> = HashMap::new();
    // The process graphic's bends move with it: its relative mapping and the corner it was placed at.
    let mut graphic_frame: Option<(Relative, [f64; 2])> = None;
    for index in reading_order(&blocks) {
        let block = &blocks[index];
        let laid = layout(block, width - 2.0 * margin);
        let mut x = block.rect.x;
        if x + laid.width > width - margin {
            x = margin.max(width - margin - laid.width);
        }
        let mut rect = Rect {
            x,
            y: block.rect.y,
            width: laid.width,
            height: laid.height,
        };
        // A block that overlaps one already placed starts a new row at the left margin...
        if placed.iter().any(|other| overlaps(rect, *other, 0.0)) {
            rect.x = margin;
        }
        // ...and moves down until it overlaps none, 16 px below the blocks above it.
        loop {
            let bottoms: Vec<f64> = placed
                .iter()
                .filter(|other| overlaps(rect, **other, 0.0))
                .map(|other| other.y + other.height)
                .collect();
            if bottoms.is_empty() {
                break;
            }
            rect.y = bottoms.into_iter().fold(f64::NEG_INFINITY, f64::max) + SPACE;
        }
        placed.push(rect);
        let relative = laid.relative;
        for (id, where_) in laid.objects {
            objects.insert(
                id,
                Placed {
                    at: [rect.x + where_.at[0], rect.y + where_.at[1]],
                    size: where_.size,
                },
            );
        }
        if block.graphic {
            graphic_frame = Some((relative, [rect.x, rect.y]));
        }
    }
    let right = placed
        .iter()
        .map(|rect| rect.x + rect.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = placed
        .iter()
        .map(|rect| rect.y + rect.height)
        .fold(f64::NEG_INFINITY, f64::max);

    let size = [width.max(right + margin), height.max(bottom + top)];
    adapted_page(page, &objects, graphic_frame, size)
}

/// The adapted page: each object at its laid place with its drawn box, and the pipes' bends moved
/// with the process graphic (the end of `adaptPage`).
fn adapted_page(
    page: &HmiPage,
    objects: &HashMap<String, Placed>,
    graphic_frame: Option<(Relative, [f64; 2])>,
    size: [f64; 2],
) -> AdaptedPage {
    let mut adapted = page.clone();
    let mut boxes = Vec::with_capacity(page.objects.len());
    for (object, adapted_object) in page.objects.iter().zip(adapted.objects.iter_mut()) {
        match objects.get(&object.id) {
            Some(placement) => {
                adapted_object.at = [whole(placement.at[0]), whole(placement.at[1])];
                adapted_object.size =
                    [whole_size(placement.size[0]), whole_size(placement.size[1])];
                boxes.push(drawn_placed(*placement, object.rotation));
            }
            None => boxes.push(drawn_box(object)),
        }
    }
    if let Some((relative, corner)) = graphic_frame {
        for pipe in &mut adapted.pipes {
            for waypoint in &mut pipe.waypoints {
                let [x, y] = relative.apply([coord(waypoint[0]), coord(waypoint[1])]);
                *waypoint = [whole(x + corner[0]), whole(y + corner[1])];
            }
        }
    }
    AdaptedPage {
        page: adapted,
        size,
        fit: false,
        boxes,
    }
}

/// The indices of the blocks in reading order (§4 item 5): by bands, top to bottom, a band being the
/// blocks whose heights overlap, directly or through others; within a band by columns, left to
/// right, a column being the blocks whose widths overlap; and within a column, top to bottom
/// (`readingOrder`).
fn reading_order(blocks: &[Block<'_>]) -> Vec<usize> {
    let mut by_top: Vec<usize> = (0..blocks.len()).collect();
    by_top.sort_by(|&first, &second| {
        compare_pixels(blocks[first].rect.y, blocks[second].rect.y)
            .then(compare_pixels(blocks[first].rect.x, blocks[second].rect.x))
    });
    let mut bands: Vec<Vec<usize>> = Vec::new();
    let mut band_bottom = f64::NEG_INFINITY;
    for index in by_top {
        let rect = blocks[index].rect;
        let bottom = rect.y + rect.height;
        if !bands.is_empty() && rect.y < band_bottom {
            if let Some(band) = bands.last_mut() {
                band.push(index);
            }
            band_bottom = band_bottom.max(bottom);
        } else {
            bands.push(vec![index]);
            band_bottom = bottom;
        }
    }
    bands
        .iter()
        .flat_map(|band| column_order(blocks, band))
        .collect()
}

/// The blocks of one band in reading order: by columns from the left, and top to bottom in each
/// column (the column part of `readingOrder`).
fn column_order(blocks: &[Block<'_>], band: &[usize]) -> Vec<usize> {
    let mut by_left = band.to_vec();
    by_left.sort_by(|&first, &second| {
        compare_pixels(blocks[first].rect.x, blocks[second].rect.x)
            .then(compare_pixels(blocks[first].rect.y, blocks[second].rect.y))
    });
    // Each column with the right edge of its widest block so far.
    let mut columns: Vec<(f64, Vec<usize>)> = Vec::new();
    for index in by_left {
        let rect = blocks[index].rect;
        let joins_column = columns.last().is_some_and(|(right, _)| rect.x < *right);
        if joins_column {
            if let Some((right, column)) = columns.last_mut() {
                column.push(index);
                *right = right.max(rect.x + rect.width);
            }
        } else {
            columns.push((rect.x + rect.width, vec![index]));
        }
    }
    columns
        .into_iter()
        .flat_map(|(_, mut column)| {
            column.sort_by(|&first, &second| {
                compare_pixels(blocks[first].rect.y, blocks[second].rect.y)
                    .then(compare_pixels(blocks[first].rect.x, blocks[second].rect.x))
            });
            column
        })
        .collect()
}

/// The blocks of the page (`blocksOf`): each top-level container with what lies wholly inside it,
/// each other object on its own, and the equipment, lines and images, with the pipes' bends, as the
/// process graphic.
fn blocks_of(page: &HmiPage) -> Vec<Block<'_>> {
    // Each object lies in the smallest container that holds it wholly; the first of equal areas.
    let containers: Vec<&HmiObject> = page
        .objects
        .iter()
        .filter(|object| is_container(&object.kind))
        .collect();
    let mut holder: HashMap<String, String> = HashMap::new();
    for object in &page.objects {
        let smallest = containers
            .iter()
            .copied()
            .filter(|container| container.id != object.id && holds(container, object))
            .min_by_key(|container| area(container));
        if let Some(container) = smallest {
            holder.insert(object.id.clone(), container.id.clone());
        }
    }
    let loose: Vec<&HmiObject> = page
        .objects
        .iter()
        .filter(|object| !holder.contains_key(&object.id))
        .collect();
    let mut blocks: Vec<Block<'_>> = Vec::new();
    for container in loose
        .iter()
        .copied()
        .filter(|object| is_container(&object.kind))
    {
        blocks.push(Block {
            container: Some(item_of(page, &holder, container)),
            items: Vec::new(),
            rect: box_of(container),
            graphic: false,
        });
    }
    for object in loose
        .iter()
        .copied()
        .filter(|object| !is_container(&object.kind) && !is_graphic(&object.kind))
    {
        blocks.push(Block {
            container: None,
            items: vec![item_of(page, &holder, object)],
            rect: box_of(object),
            graphic: false,
        });
    }
    let graphic: Vec<&HmiObject> = loose
        .iter()
        .copied()
        .filter(|object| !is_container(&object.kind) && is_graphic(&object.kind))
        .collect();
    if !graphic.is_empty() {
        blocks.push(graphic_block(page, &holder, &graphic));
    }
    blocks
}

/// The process graphic: the equipment, lines and images outside every container, with the pipes'
/// bends among its points, so that its box holds them all (`blocksOf`).
fn graphic_block<'a>(
    page: &'a HmiPage,
    holder: &HashMap<String, String>,
    graphic: &[&'a HmiObject],
) -> Block<'a> {
    let mut points: Vec<[i64; 2]> = Vec::new();
    for object in graphic {
        points.push(object.at);
        points.push([
            object.at[0] + i64::from(object.size[0]),
            object.at[1] + i64::from(object.size[1]),
        ]);
    }
    points.extend(
        page.pipes
            .iter()
            .flat_map(|pipe| pipe.waypoints.iter().copied()),
    );
    let left = points
        .iter()
        .fold(i64::MAX, |best, point| best.min(point[0]));
    let upper = points
        .iter()
        .fold(i64::MAX, |best, point| best.min(point[1]));
    let right = points
        .iter()
        .fold(i64::MIN, |best, point| best.max(point[0]));
    let lower = points
        .iter()
        .fold(i64::MIN, |best, point| best.max(point[1]));
    Block {
        container: None,
        items: graphic
            .iter()
            .map(|object| item_of(page, holder, object))
            .collect(),
        rect: Rect {
            x: coord(left),
            y: coord(upper),
            width: coord(right - left),
            height: coord(lower - upper),
        },
        graphic: true,
    }
}

/// An object with the objects that the holder map places inside it, in page order (`itemOf`).
fn item_of<'a>(
    page: &'a HmiPage,
    holder: &HashMap<String, String>,
    object: &'a HmiObject,
) -> Item<'a> {
    Item {
        object,
        inside: page
            .objects
            .iter()
            .filter(|other| holder.get(&other.id).is_some_and(|id| *id == object.id))
            .map(|other| item_of(page, holder, other))
            .collect(),
    }
}

/// A block at the width between the margins: as designed when it fits; else an arranged container
/// rearranges its objects, and any other block scales uniformly (`layOut`).
fn layout(block: &Block<'_>, inner: f64) -> Laid {
    let objects: Vec<&HmiObject> = match &block.container {
        Some(container) => flatten(container),
        None => block.items.iter().flat_map(flatten).collect(),
    };
    if block.rect.width <= inner {
        return scaled(&objects, block.rect, 1.0);
    }
    if let Some(container) = block
        .container
        .as_ref()
        .filter(|container| container.object.arrange.is_some())
    {
        return arranged(container, inner);
    }
    scaled(
        &objects,
        block.rect,
        scale_for(&objects, inner / block.rect.width),
    )
}

/// The objects scaled by `scale` about the box's corner, each to whole pixels (`scaled`).
fn scaled(objects: &[&HmiObject], rect: Rect, scale: f64) -> Laid {
    let relative = Relative::Scaled {
        corner: [rect.x, rect.y],
        scale,
    };
    let placed: HashMap<String, Placed> = objects
        .iter()
        .map(|object| {
            (
                object.id.clone(),
                Placed {
                    at: relative.apply(object_at(object)),
                    size: [
                        js_round(f64::from(object.size[0]) * scale),
                        js_round(f64::from(object.size[1]) * scale),
                    ],
                },
            )
        })
        .collect();
    Laid {
        width: js_round(rect.width * scale),
        height: js_round(rect.height * scale),
        objects: placed,
        relative,
    }
}

/// The scale wanted, but not smaller than keeps every control at its touch size, and never larger
/// than 1 (`scaleFor`).
fn scale_for(objects: &[&HmiObject], wanted: f64) -> f64 {
    let least = objects
        .iter()
        .filter(|object| CONTROLS.contains(&object.kind.as_str()))
        .map(|object| {
            (TOUCH / f64::from(object.size[0]))
                .max(TOUCH / f64::from(object.size[1]))
                .min(1.0)
        })
        .fold(0.0, f64::max);
    wanted.max(least).min(1.0)
}

/// An arranged container at the width between the margins: its objects in reading order inside its
/// padding, in a row, a column or a grid. Each keeps its size unless it is wider than the room, and
/// what lies inside one moves with it (`arranged`).
fn arranged(container: &Item<'_>, width: f64) -> Laid {
    let object = container.object;
    let gap = object.gap.map_or(SPACE, f64::from);
    let upper = if has_label(object) { TITLE } else { SPACE };
    let room = width - 2.0 * SPACE;
    let mut items: Vec<&Item<'_>> = container.inside.iter().collect();
    items.sort_by(|first, second| {
        first.object.at[1]
            .cmp(&second.object.at[1])
            .then(first.object.at[0].cmp(&second.object.at[0]))
    });
    let pieces = shrunk_pieces(&items, room);
    let mut corners: Vec<[f64; 2]> = Vec::with_capacity(pieces.len());
    let height = match object.arrange {
        Some(HmiArrange::Grid) => {
            let cell_width = pieces.iter().map(|piece| piece.width).fold(0.0, f64::max);
            let cell_height = pieces.iter().map(|piece| piece.height).fold(0.0, f64::max);
            let columns = ((room + gap) / (cell_width + gap)).floor().max(1.0);
            let mut index: f64 = 0.0;
            for _ in &pieces {
                corners.push([
                    (index % columns) * (cell_width + gap),
                    (index / columns).floor() * (cell_height + gap),
                ]);
                index += 1.0;
            }
            if pieces.is_empty() {
                0.0
            } else {
                (count_f64(pieces.len()) / columns).ceil() * (cell_height + gap) - gap
            }
        }
        Some(HmiArrange::Column) => {
            let mut height: f64 = 0.0;
            for piece in &pieces {
                corners.push([0.0, height]);
                height += piece.height + gap;
            }
            (height - gap).max(0.0)
        }
        Some(HmiArrange::Row) | None => {
            let (mut x, mut y, mut line): (f64, f64, f64) = (0.0, 0.0, 0.0);
            for piece in &pieces {
                if x > 0.0 && x + piece.width > room {
                    y = y + line + gap;
                    x = 0.0;
                    line = 0.0;
                }
                corners.push([x, y]);
                x += piece.width + gap;
                line = line.max(piece.height);
            }
            y + line
        }
    };
    let total = upper + height + SPACE;
    let mut objects: HashMap<String, Placed> = HashMap::new();
    objects.insert(
        object.id.clone(),
        Placed {
            at: [0.0, 0.0],
            size: [width, total],
        },
    );
    for (piece, corner) in pieces.iter().zip(&corners) {
        for (id, where_) in &piece.objects {
            objects.insert(
                id.clone(),
                Placed {
                    at: [
                        where_.at[0] + (SPACE + corner[0]),
                        where_.at[1] + (upper + corner[1]),
                    ],
                    size: where_.size,
                },
            );
        }
    }
    let origin = box_of(object);
    Laid {
        width,
        height: total,
        objects,
        relative: Relative::Shifted {
            corner: [origin.x, origin.y],
        },
    }
}

/// Each item of an arranged container laid out on its own, shrunk to the room when it is wider than
/// the room (the pieces of `arranged`).
fn shrunk_pieces(items: &[&Item<'_>], room: f64) -> Vec<Laid> {
    items
        .iter()
        .map(|item| {
            let rect = box_of(item.object);
            let objects = flatten(item);
            let factor = if rect.width > room {
                scale_for(&objects, room / rect.width)
            } else {
                1.0
            };
            scaled(&objects, rect, factor)
        })
        .collect()
}

/// An item and everything inside it, the item first (`flatten`).
fn flatten<'a>(item: &Item<'a>) -> Vec<&'a HmiObject> {
    let mut objects = vec![item.object];
    for inner in &item.inside {
        objects.extend(flatten(inner));
    }
    objects
}

/// An object's box as the layout places it: its position and its design size (`boxOf`).
fn box_of(object: &HmiObject) -> Rect {
    Rect {
        x: coord(object.at[0]),
        y: coord(object.at[1]),
        width: f64::from(object.size[0]),
        height: f64::from(object.size[1]),
    }
}

/// An object's position as exact pixels.
fn object_at(object: &HmiObject) -> [f64; 2] {
    [coord(object.at[0]), coord(object.at[1])]
}

/// Whether a container holds an object wholly and is larger than it (`holds`).
fn holds(container: &HmiObject, object: &HmiObject) -> bool {
    contains(box_of(container), box_of(object)) && area(object) < area(container)
}

/// An object's design area in square pixels.
fn area(object: &HmiObject) -> u64 {
    u64::from(object.size[0]) * u64::from(object.size[1])
}

/// Whether a kind is a container the layout keeps together: a panel or a group.
fn is_container(kind: &str) -> bool {
    matches!(kind, "panel" | "group")
}

/// Whether a kind belongs to the process graphic: equipment, a junction, a line or an image.
fn is_graphic(kind: &str) -> bool {
    symbol_geometry(kind).is_some() || matches!(kind, "junction" | "line" | "image")
}

/// Whether an object has a label. An empty label is not one, as in `adaptive.ts`.
fn has_label(object: &HmiObject) -> bool {
    object
        .label
        .as_deref()
        .is_some_and(|label| !label.is_empty())
}

/// The drawn box of an object at an exact placement: the rotation of `drawnBox` in `editing.ts`
/// applied to the laid position, as `geometry::drawn_box` applies it to a design position.
fn drawn_placed(placed: Placed, rotation: u16) -> Rect {
    super::geometry::box_of(placed.at, placed.size, rotation)
}

/// The order of two coordinates in pixels, which the layout's whole coordinates never leave
/// unordered.
fn compare_pixels(first: f64, second: f64) -> Ordering {
    first.partial_cmp(&second).unwrap_or(Ordering::Equal)
}

/// A count of objects as a float.
#[allow(clippy::cast_precision_loss)] // A page holds far fewer objects than 2^53.
fn count_f64(count: usize) -> f64 {
    count as f64
}

/// A whole pixel. The layout rounds every position it produces, so the conversion is exact.
#[allow(clippy::cast_possible_truncation)] // The value is a whole number of pixels.
fn whole(value: f64) -> i64 {
    value as i64
}

/// A whole size in pixels, zero where the layout gives a negative one (an arranged container
/// narrower than its margins, whose exact box is in `AdaptedPage::boxes`).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Whole, and not negative.
fn whole_size(value: f64) -> u32 {
    value.max(0.0) as u32
}

#[cfg(test)]
#[path = "adaptive_tests.rs"]
mod tests;
