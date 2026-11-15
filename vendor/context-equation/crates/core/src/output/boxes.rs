//! The boxes of the groups an equation marks with `\htmlData`, `\htmlId`, `\class` and the like:
//! where each marked piece of the formula sits, so an editor can place a cursor, hit-test a click or
//! outline a selection over the SVG. In the SVG's own coordinates (those of its `viewBox`), y down.

use crate::layout::{Item, MathBox};

/// One marked group: its attributes as the SVG writes them and the box its content was set in (the
/// logical box: advance width, height and depth, not the ink).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupBox {
    pub attributes: Vec<(String, String)>,
    /// Left edge.
    pub x: f32,
    /// Top edge (the top of the height).
    pub y: f32,
    pub width: f32,
    /// Height plus depth.
    pub height: f32,
    /// Where the baseline is, as a y.
    pub baseline: f32,
    /// How many marked groups hold this one (0: none).
    pub level: usize,
}

impl GroupBox {
    pub fn to_json(&self) -> serde_json::Value {
        let attributes: serde_json::Map<String, serde_json::Value> = self.attributes.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect();
        let r = |v: f32| ((v * 100.0).round() / 100.0) as f64;
        serde_json::json!({
            "attributes": attributes,
            "x": r(self.x),
            "y": r(self.y),
            "width": r(self.width),
            "height": r(self.height),
            "baseline": r(self.baseline),
            "level": self.level,
        })
    }
}

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// `outer` after `inner`, both as SVG's `matrix(a b c d e f)`.
fn compose(outer: [f32; 6], inner: [f32; 6]) -> [f32; 6] {
    let [a, b, c, d, e, f] = outer;
    let [a2, b2, c2, d2, e2, f2] = inner;
    [a * a2 + c * b2, b * a2 + d * b2, a * c2 + c * d2, b * c2 + d * d2, a * e2 + c * f2 + e, b * e2 + d * f2 + f]
}

fn apply(m: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// The boxes of every marked group, outer before inner, in the order the layout set them. A broken
/// formula (`lines` not empty) is read from its lines, as the SVG draws it.
pub fn group_boxes(math_box: &MathBox, lines: &[MathBox]) -> Vec<GroupBox> {
    let mut out = Vec::new();
    if lines.is_empty() {
        walk(&math_box.items, IDENTITY, 0, &mut out);
    }
    for line in lines {
        walk(&line.items, IDENTITY, 0, &mut out);
    }
    out
}

fn walk(items: &[Item], m: [f32; 6], level: usize, out: &mut Vec<GroupBox>) {
    for item in items {
        match item {
            Item::Transformed { matrix, items } => walk(items, compose(m, *matrix), level, out),
            Item::Group { attributes, items, bounds: [x, base, w, h, d] } => {
                // the corners in the layout's coordinates (y up), through the transforms around them
                let corners = [apply(m, *x, base - d), apply(m, x + w, base - d), apply(m, *x, base + h), apply(m, x + w, base + h)];
                let left = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
                let right = corners.iter().map(|c| c.0).fold(f32::NEG_INFINITY, f32::max);
                let low = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
                let high = corners.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max);
                let (_, baseline) = apply(m, *x, *base);
                // the SVG flips y (scale(1,-1)): its y is the layout's y negated
                out.push(GroupBox { attributes: attributes.clone(), x: left, y: -high, width: right - left, height: high - low, baseline: -baseline, level });
                walk(items, m, level + 1, out);
            }
            _ => {}
        }
    }
}
