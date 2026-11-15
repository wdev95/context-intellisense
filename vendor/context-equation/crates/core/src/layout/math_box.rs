//! Box model: everything drawn has a width, a height above the baseline
//! and a depth below it. Coordinates in font units, y pointing up.

use ttf_parser::GlyphId;

use crate::font::{glyph_font, FallbackFont, Font};

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Glyph {
        id: GlyphId,
        x: f32,
        y: f32,
        scale: f32,
        color: Option<String>,
        /// Mirrored left to right around its origin (`\reflectbox`).
        flip: bool,
        /// The fallback font the glyph is from, for a character the math font lacks; `None`: the
        /// math font. `id` and `scale` are that font's (the scale takes its em to the math font's).
        font: Option<FallbackFont>,
    },
    /// Filled rectangle; (x, y) is the lower left corner.
    Rule { x: f32, y: f32, width: f32, height: f32, color: Option<String> },
    /// Straight segment (cancels).
    Line { x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: Option<String> },
    /// Ellipse outline (`\textcircled`).
    Ellipse { cx: f32, cy: f32, rx: f32, ry: f32, thickness: f32, color: Option<String> },
    /// Items drawn through an affine map `[a, b, c, d, e, f]`: (x, y) → (a·x + c·y + e, b·x + d·y + f),
    /// as SVG's `matrix()`. Turned and unevenly scaled boxes (`\rotatebox`, `\scalebox{2}[1]`).
    Transformed { matrix: [f32; 6], items: Vec<Item> },
    /// Items drawn inside a group that carries HTML attributes (`class`, `id`, `style`, `data-*`):
    /// `\class`, `\htmlId`… The names and values are already safe to write. `bounds` is the box the
    /// content was set in, as `[x, baseline, width, height, depth]`, (x, baseline) the left end of its
    /// baseline: the logical box, not the ink, so an editor knows where each piece sits.
    Group { attributes: Vec<(String, String)>, items: Vec<Item>, bounds: [f32; 5] },
}

impl Item {
    fn shift(&mut self, dx: f32, dy: f32) {
        match self {
            Item::Glyph { x, y, .. } | Item::Rule { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Item::Line { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Item::Ellipse { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Item::Transformed { matrix, .. } => {
                matrix[4] += dx;
                matrix[5] += dy;
            }
            Item::Group { items, bounds, .. } => {
                bounds[0] += dx;
                bounds[1] += dy;
                for item in items {
                    item.shift(dx, dy);
                }
            }
        }
    }

    fn colorize(&mut self, new: &str) {
        let color = match self {
            Item::Glyph { color, .. } | Item::Rule { color, .. } | Item::Line { color, .. } | Item::Ellipse { color, .. } => color,
            Item::Transformed { items, .. } | Item::Group { items, .. } => {
                for item in items {
                    item.colorize(new);
                }
                return;
            }
        };
        if color.is_none() {
            *color = Some(new.to_string());
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MathBox {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    pub items: Vec<Item>,
    /// Italic correction of the last glyph: how far right the superscript should move.
    pub italic: f32,
    /// Where an accent should be centered, when the box is a single glyph.
    pub accent_point: Option<f32>,
    /// The box is a single glyph (affects scripts and accents).
    pub single_glyph: bool,
}

impl MathBox {
    pub fn empty() -> MathBox {
        MathBox::default()
    }

    pub fn space(width: f32) -> MathBox {
        MathBox { width, ..MathBox::default() }
    }

    pub fn bar(width: f32, thickness: f32, base: f32) -> MathBox {
        MathBox {
            width,
            height: (base + thickness).max(0.0),
            depth: (-base).max(0.0),
            items: vec![Item::Rule { x: 0.0, y: base, width, height: thickness, color: None }],
            ..MathBox::default()
        }
    }

    /// Places `other` with its origin at (dx, dy) and updates height and depth. The width does not change.
    pub fn place(&mut self, other: &MathBox, dx: f32, dy: f32) {
        for item in &other.items {
            let mut i = item.clone();
            i.shift(dx, dy);
            self.items.push(i);
        }
        if !other.items.is_empty() || other.height > 0.0 || other.depth > 0.0 {
            self.height = self.height.max(other.height + dy);
            self.depth = self.depth.max(other.depth - dy);
        }
        self.single_glyph = false;
    }

    /// Appends `other` on the right, on the same baseline.
    pub fn append(&mut self, other: &MathBox) {
        let x = self.width;
        self.place(other, x, 0.0);
        self.width += other.width;
        self.italic = other.italic;
    }

    pub fn shift_vertical(&mut self, dy: f32) {
        for item in &mut self.items {
            item.shift(0.0, dy);
        }
        self.height += dy;
        self.depth -= dy;
    }

    /// Multiplies everything by `f`, from the origin (`\scalebox`).
    pub fn scale_box(&mut self, f: f32) {
        fn scale(items: &mut [Item], f: f32) {
            for item in items {
                scale_item(item, f);
            }
        }
        fn scale_item(item: &mut Item, f: f32) {
            match item {
                Item::Glyph { x, y, scale, .. } => {
                    *x *= f;
                    *y *= f;
                    *scale *= f;
                }
                Item::Rule { x, y, width, height, .. } => {
                    *x *= f;
                    *y *= f;
                    *width *= f;
                    *height *= f;
                }
                Item::Line { x1, y1, x2, y2, thickness, .. } => {
                    *x1 *= f;
                    *y1 *= f;
                    *x2 *= f;
                    *y2 *= f;
                    *thickness *= f;
                }
                Item::Ellipse { cx, cy, rx, ry, thickness, .. } => {
                    *cx *= f;
                    *cy *= f;
                    *rx *= f;
                    *ry *= f;
                    *thickness *= f;
                }
                Item::Transformed { matrix, .. } => {
                    for m in matrix.iter_mut() {
                        *m *= f;
                    }
                }
                Item::Group { items, bounds, .. } => {
                    for b in bounds.iter_mut() {
                        *b *= f;
                    }
                    scale(items, f);
                }
            }
        }
        scale(&mut self.items, f);
        self.width *= f;
        self.height *= f;
        self.depth *= f;
        self.italic *= f;
        self.accent_point = self.accent_point.map(|p| p * f);
        self.single_glyph = false;
    }

    /// Mirrors everything left to right inside the box's width (`\reflectbox`).
    pub fn mirror(&mut self) {
        fn mirror_items(items: &mut [Item], w: f32) {
            for item in items {
                mirror_item(item, w);
            }
        }
        fn mirror_item(item: &mut Item, w: f32) {
            match item {
                Item::Glyph { x, flip, .. } => {
                    *x = w - *x;
                    *flip = !*flip;
                }
                Item::Rule { x, width, .. } => *x = w - *x - *width,
                Item::Line { x1, x2, .. } => {
                    *x1 = w - *x1;
                    *x2 = w - *x2;
                }
                Item::Ellipse { cx, .. } => *cx = w - *cx,
                Item::Transformed { matrix, .. } => {
                    matrix[0] = -matrix[0];
                    matrix[2] = -matrix[2];
                    matrix[4] = w - matrix[4];
                }
                Item::Group { items, bounds, .. } => {
                    bounds[0] = w - bounds[0] - bounds[2];
                    mirror_items(items, w);
                }
            }
        }
        let w = self.width;
        mirror_items(&mut self.items, w);
        self.italic = 0.0;
        self.accent_point = self.accent_point.map(|p| w - p);
        self.single_glyph = false;
    }

    /// Draws everything through the affine map `m` (see [`Item::Transformed`]). The width, height and
    /// depth are the caller's to set.
    pub fn transform(&mut self, m: [f32; 6]) {
        let items = std::mem::take(&mut self.items);
        self.items.push(Item::Transformed { matrix: m, items });
        self.italic = 0.0;
        self.accent_point = None;
        self.single_glyph = false;
    }

    /// Puts everything inside a group with these HTML attributes (see [`Item::Group`]). The box
    /// keeps its size and its metrics.
    pub fn group(&mut self, attributes: Vec<(String, String)>) {
        let items = std::mem::take(&mut self.items);
        let bounds = [0.0, 0.0, self.width, self.height, self.depth];
        self.items.push(Item::Group { attributes, items, bounds });
    }

    pub fn colorize(&mut self, color: &str) {
        for item in &mut self.items {
            item.colorize(color);
        }
    }

    pub fn total(&self) -> f32 {
        self.height + self.depth
    }

    /// Where the ink really is, as (x_min, y_min, x_max, y_max), or `None` with nothing drawn. A
    /// glyph's outline may pass its advance (the hook of a bold f, the tail of ρ) and a cancel's line
    /// passes the box it crosses: the width and height say where the next box goes, not where the
    /// ink stops (S-SVG-1).
    pub fn ink(&self, f: &Font) -> Option<(f32, f32, f32, f32)> {
        let mut b = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        let mut add = |x0: f32, y0: f32, x1: f32, y1: f32| {
            b = (b.0.min(x0), b.1.min(y0), b.2.max(x1), b.3.max(y1));
        };
        for item in &self.items {
            match *item {
                Item::Glyph { id, x, y, scale, flip, font, .. } => {
                    let (x0, y0, x1, y1) = glyph_font(font, f).limits(id);
                    if x1 > x0 || y1 > y0 {
                        let (left, right) = if flip { (x - x1 * scale, x - x0 * scale) } else { (x + x0 * scale, x + x1 * scale) };
                        add(left, y + y0 * scale, right, y + y1 * scale);
                    }
                }
                Item::Rule { x, y, width, height, .. } => add(x, y, x + width, y + height),
                // round caps: half the thickness past each end
                Item::Line { x1, y1, x2, y2, thickness, .. } => {
                    let h = thickness / 2.0;
                    add(x1.min(x2) - h, y1.min(y2) - h, x1.max(x2) + h, y1.max(y2) + h);
                }
                Item::Ellipse { cx, cy, rx, ry, thickness, .. } => {
                    let h = thickness / 2.0;
                    add(cx - rx - h, cy - ry - h, cx + rx + h, cy + ry + h);
                }
                Item::Group { ref items, .. } => {
                    let inner = MathBox { items: items.clone(), ..MathBox::default() };
                    if let Some((x0, y0, x1, y1)) = inner.ink(f) {
                        add(x0, y0, x1, y1);
                    }
                }
                // the corners of the inner ink, mapped: a turned box's ink fits inside them
                Item::Transformed { ref matrix, ref items } => {
                    let inner = MathBox { items: items.clone(), ..MathBox::default() };
                    if let Some((x0, y0, x1, y1)) = inner.ink(f) {
                        let [a, bb, c, d, e, g] = *matrix;
                        for (x, y) in [(x0, y0), (x0, y1), (x1, y0), (x1, y1)] {
                            let (px, py) = (a * x + c * y + e, bb * x + d * y + g);
                            add(px, py, px, py);
                        }
                    }
                }
            }
        }
        (b.0 <= b.2).then_some(b)
    }
}
