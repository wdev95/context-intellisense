//! The equation written by a pen: an SVG where a pen tip goes over each glyph's outline, rule and
//! line in the order the layout set them, and each one fills in once the pen has gone around it.
//! SMIL only, no script and no CSS: a browser plays it by itself, inline or opened, in a loop.

use std::fmt::Write;

use crate::font::{Font, MathFont};
use crate::layout::{Item, MathBox};
use crate::VERSION;

use super::svg::{escape, n};

/// How the pen writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Pen {
    /// Seconds the pen takes to write the whole equation. `None`: from how much ink it has, at a
    /// steady hand (between 1.5 and 12 seconds).
    pub seconds: Option<f64>,
    /// Seconds the finished equation stays before the pen starts over.
    pub hold: f64,
    /// The color of the pen's tip.
    pub tip: String,
}

impl Default for Pen {
    fn default() -> Pen {
        Pen { seconds: None, hold: 1.6, tip: "#C8102E".into() }
    }
}

/// How much faster the pen travels lifted than writing.
const LIFTED: f64 = 3.0;
/// Ems of outline the pen goes over in a second, when no time is asked for.
const EMS_PER_SECOND: f64 = 9.0;
/// Seconds a glyph takes to fill in once its outline is drawn.
const FILL: f64 = 0.25;

type Point = (f64, f64);

/// What the pen draws for one item of the layout: the lines it goes over and the element that fills
/// in after them, already placed.
struct Piece {
    strokes: Vec<Vec<Point>>,
    element: String,
}

/// An affine map `[a b c d e f]`, as SVG's `matrix()`.
#[derive(Clone, Copy)]
struct Map([f64; 6]);

impl Map {
    const IDENTITY: Map = Map([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    fn apply(&self, (x, y): Point) -> Point {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }

    fn then(&self, inner: [f32; 6]) -> Map {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = inner.map(f64::from);
        Map([a * p + c * q, b * p + d * q, a * r + c * s, b * r + d * s, a * t + c * u + e, b * t + d * u + f])
    }

    fn is_identity(&self) -> bool {
        self.0.iter().zip(Map::IDENTITY.0).all(|(v, w)| (v - w).abs() < 1e-9)
    }

    /// The element inside the map, as a group when the map does something.
    fn wrap(&self, element: String) -> String {
        if self.is_identity() {
            return element;
        }
        let [a, b, c, d, e, f] = self.0.map(|v| n(v as f32));
        format!(r#"<g transform="matrix({a} {b} {c} {d} {e} {f})">{element}</g>"#)
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn length(line: &[Point]) -> f64 {
    line.windows(2).map(|w| distance(w[0], w[1])).sum()
}

fn color_fill(color: &Option<String>) -> String {
    color.as_ref().map(|c| format!(r#" fill="{}""#, escape(c))).unwrap_or_default()
}

/// The pieces of the items, in the order the layout set them, inside turned boxes too.
fn pieces(items: &[Item], map: Map, id: &str, f: &Font, out: &mut Vec<Piece>) {
    let step = f.units_per_em() / 120.0;
    for item in items {
        match item {
            Item::Glyph { id: g, x, y, scale, color, flip, font } => {
                let (x, y, s) = (f64::from(*x), f64::from(*y), f64::from(*scale));
                let sx = if *flip { -s } else { s };
                // a fallback glyph's units are its font's: the step is the math font's, in them
                let gf = crate::font::glyph_font(*font, f);
                let strokes = gf
                    .contours(*g, step * gf.units_per_em() / f.units_per_em() / scale.max(0.1))
                    .into_iter()
                    .map(|c| c.into_iter().map(|(px, py)| map.apply((x + sx * f64::from(px), y + s * f64::from(py)))).collect())
                    .collect();
                let element = format!(
                    r##"<use href="#{id}-{}" transform="translate({} {}) scale({} {})"{}/>"##,
                    crate::output::svg::glyph_name(*font, *g),
                    n(x as f32),
                    n(y as f32),
                    n(sx as f32),
                    n(s as f32),
                    color_fill(color)
                );
                out.push(Piece { strokes, element: map.wrap(element) });
            }
            Item::Rule { x, y, width, height, color } => {
                let (x0, y0, x1, y1) = (f64::from(*x), f64::from(*y), f64::from(x + width), f64::from(y + height));
                // a thin rule is one line along it, as a hand draws a fraction bar
                let line: Vec<Point> = if width >= height {
                    let m = (y0 + y1) / 2.0;
                    vec![(x0, m), (x1, m)]
                } else {
                    let m = (x0 + x1) / 2.0;
                    vec![(m, y1), (m, y0)]
                };
                let element = format!(r#"<rect x="{}" y="{}" width="{}" height="{}"{}/>"#, n(*x), n(*y), n(*width), n(*height), color_fill(color));
                out.push(Piece { strokes: vec![line.into_iter().map(|p| map.apply(p)).collect()], element: map.wrap(element) });
            }
            Item::Line { x1, y1, x2, y2, thickness, color } => {
                let line = vec![map.apply((f64::from(*x1), f64::from(*y1))), map.apply((f64::from(*x2), f64::from(*y2)))];
                let element = format!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2),
                    escape(color.as_deref().unwrap_or("currentColor")),
                    n(*thickness)
                );
                out.push(Piece { strokes: vec![line], element: map.wrap(element) });
            }
            Item::Ellipse { cx, cy, rx, ry, thickness, color } => {
                let (cx, cy, rx, ry) = (f64::from(*cx), f64::from(*cy), f64::from(*rx), f64::from(*ry));
                // from the top, counterclockwise, as a circle is written
                let ring: Vec<Point> = (0..=64)
                    .map(|k| {
                        let a = std::f64::consts::FRAC_PI_2 + k as f64 / 64.0 * std::f64::consts::TAU;
                        map.apply((cx + rx * a.cos(), cy + ry * a.sin()))
                    })
                    .collect();
                let element = format!(
                    r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                    n(cx as f32),
                    n(cy as f32),
                    n(rx as f32),
                    n(ry as f32),
                    escape(color.as_deref().unwrap_or("currentColor")),
                    n(*thickness)
                );
                out.push(Piece { strokes: vec![ring], element: map.wrap(element) });
            }
            Item::Transformed { matrix, items } => pieces(items, map.then(*matrix), id, f, out),
            // the pen draws stroke by stroke: the attributes of a group (\class…) are the plain SVG's
            Item::Group { items, .. } => pieces(items, map, id, f, out),
        }
    }
}

/// The glyph outlines the pieces use, once each.
fn defs(items: &[Item], id: &str, f: &Font, seen: &mut std::collections::HashSet<crate::output::svg::GlyphKey>, s: &mut String) {
    for item in items {
        match item {
            Item::Glyph { id: g, font, .. } if seen.insert((font.map(crate::font::FallbackFont::tag), g.0)) => {
                let _ = write!(s, r#"<path id="{id}-{}" d="{}"/>"#, crate::output::svg::glyph_name(*font, *g), crate::font::glyph_font(*font, f).outline(*g));
            }
            Item::Transformed { items, .. } | Item::Group { items, .. } => defs(items, id, f, seen, s),
            _ => {}
        }
    }
}

/// Unique ids per equation, as in the plain SVG, but apart from them: both can share a page.
fn id_prefix(source: &str, font: MathFont) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let salt = font.id_salt().map(u64::to_le_bytes);
    for b in source.bytes().chain(VERSION.bytes()).chain(*b"pen").chain(salt.into_iter().flatten()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("context-equation-pen-{:08x}", h as u32)
}

/// The SVG of the pen writing `math_box`. `speech` is its accessible name.
pub fn generate(math_box: &MathBox, font: MathFont, source: &str, speech: &str, pen: &Pen) -> String {
    let f: &Font = font.get();
    let em = f64::from(f.units_per_em());
    let id = id_prefix(source, font);
    let mut all = Vec::new();
    pieces(&math_box.items, Map::IDENTITY, &id, f, &mut all);
    for p in &mut all {
        p.strokes.retain(|s| s.len() > 1 && length(s) > 1e-6);
    }

    // the pen's time: each stroke as long as its length, travels three times as fast
    let order: Vec<&Vec<Point>> = all.iter().flat_map(|p| &p.strokes).collect();
    let drawn: f64 = order.iter().map(|s| length(s)).sum();
    let lifted: f64 = order.windows(2).map(|w| distance(w[0][w[0].len() - 1], w[1][0])).sum();
    let seconds = pen.seconds.filter(|s| *s > 0.0 && s.is_finite()).unwrap_or_else(|| (drawn / em / EMS_PER_SECOND).clamp(1.5, 12.0));
    let per_unit = seconds / (drawn + lifted / LIFTED).max(1e-9);
    let cycle = seconds + FILL + pen.hold.max(0.0);
    let key = |t: f64| format!("{:.5}", (t / cycle).clamp(0.0, 1.0));

    // the view as the plain SVG's: the box and the ink that passes it
    let (mut left, mut right, mut top, mut bottom) = (0.0f32, math_box.width, math_box.height, math_box.depth);
    if let Some((x0, y0, x1, y1)) = math_box.ink(f) {
        (left, right, top, bottom) = (left.min(x0), right.max(x1), top.max(y1), bottom.max(-y0));
    }
    let (width, height) = (right - left, top + bottom);
    let ink = em * 0.028;
    let (tip, tip_up) = (em * 0.05, em * 0.08);

    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}em" height="{}em" style="vertical-align: {}em" role="img" aria-label="{}" data-context-equation-version="{}" data-context-equation-pen="{}">"#,
        n(left - tip_up as f32),
        n(-top - tip_up as f32),
        n(width + 2.0 * tip_up as f32),
        n(height + 2.0 * tip_up as f32),
        n((width + 2.0 * tip_up as f32) / em as f32),
        n((height + 2.0 * tip_up as f32) / em as f32),
        n((-bottom - tip_up as f32) / em as f32),
        escape(speech),
        VERSION,
        n(cycle as f32),
    );
    s.push_str("<style>svg:root{color:#111}</style><defs>");
    defs(&math_box.items, &id, f, &mut Default::default(), &mut s);
    s.push_str("</defs>");
    let _ = write!(s, r#"<g fill="currentColor" stroke="none" transform="scale(1,-1)">"#);

    let line = |points: &[Point]| {
        let mut d = String::with_capacity(points.len() * 12);
        for (k, p) in points.iter().enumerate() {
            let _ = write!(d, "{}{} {}", if k == 0 { "M" } else { " L" }, n(p.0 as f32), n(p.1 as f32));
        }
        d
    };
    let repeat = format!(r#" dur="{}s" repeatCount="indefinite""#, n(cycle as f32));

    let (mut t, mut at): (f64, Option<Point>) = (0.0, None);
    // the tip's way and when it is where: (route length so far, time) at each stroke's ends
    let mut route: Vec<Point> = Vec::new();
    let mut marks: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    let mut sizes: Vec<(f64, f64)> = Vec::new();
    let mut along = 0.0;
    for piece in &all {
        let mut strokes = String::new();
        let mut end = t;
        for stroke in &piece.strokes {
            if let Some(p) = at {
                let travel = distance(p, stroke[0]);
                t += travel * per_unit / LIFTED;
                along += travel;
                marks.push((along, t));
            }
            let from = t;
            t += length(stroke) * per_unit;
            along += length(stroke);
            marks.push((along, t));
            sizes.push((from, tip));
            sizes.push((t, tip_up));
            route.extend_from_slice(stroke);
            at = Some(stroke[stroke.len() - 1]);
            end = t;
            let _ = write!(
                strokes,
                r#"<path d="{}" pathLength="1" stroke-dasharray="1 2" stroke-dashoffset="1.01"><animate attributeName="stroke-dashoffset" values="1.01;1.01;0;0" keyTimes="0;{};{};1"{repeat}/></path>"#,
                line(stroke),
                key(from),
                key(t)
            );
        }
        if piece.strokes.is_empty() {
            s.push_str(&piece.element);
            continue;
        }
        // the outline goes as the inside fills in
        let (a, b) = (key(end), key(end + FILL));
        let _ = write!(
            s,
            r#"<g fill="none" stroke="currentColor" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" opacity="1">{strokes}<animate attributeName="opacity" values="1;1;0;0" keyTimes="0;{a};{b};1"{repeat}/></g>"#,
            n(ink as f32)
        );
        let _ = write!(s, r#"<g opacity="0">{}<animate attributeName="opacity" values="0;0;1;1" keyTimes="0;{a};{b};1"{repeat}/></g>"#, piece.element);
    }

    // the tip: down while writing, larger lifted, resting at the end
    if !route.is_empty() {
        let total = along.max(1e-9);
        let mut times = vec!["0".to_string()];
        let mut points = vec!["0".to_string()];
        for &(l, when) in &marks[1..] {
            times.push(key(when));
            points.push(format!("{:.5}", (l / total).min(1.0)));
        }
        times.push("1".into());
        points.push("1".into());
        let mut radii = vec![n(tip_up as f32)];
        let mut radius_times = vec!["0".to_string()];
        for &(when, r) in &sizes {
            radii.push(n(r as f32));
            radius_times.push(key(when));
        }
        radii.push(n(tip_up as f32));
        radius_times.push("1".into());
        // keyTimes must not go back: equal ones are kept, a later one wins
        let _ = write!(
            s,
            r#"<circle r="{}" fill="{}" opacity="0.85"><animate attributeName="r" calcMode="discrete" values="{}" keyTimes="{}"{repeat}/><animateMotion path="{}" keyPoints="{}" keyTimes="{}" calcMode="linear"{repeat}/></circle>"#,
            n(tip_up as f32),
            escape(&pen.tip),
            radii.join(";"),
            radius_times.join(";"),
            line(&route),
            points.join(";"),
            times.join(";")
        );
    }
    s.push_str("</g></svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pen_of(tex: &str) -> String {
        crate::pen(tex, &crate::Options::default(), &Pen::default()).unwrap()
    }

    #[test]
    fn each_glyph_is_drawn_then_filled() {
        let svg = pen_of("x+y");
        assert_eq!(svg.matches("<use ").count(), 3);
        assert!(svg.matches("<path d=").count() >= 3);
        assert!(svg.contains("<animateMotion") && !svg.contains("<script"));
    }

    #[test]
    fn times_never_go_back() {
        let svg = pen_of(r"\frac{a^2+\sqrt{b}}{\cos\theta} = \sum_{k=1}^{n} k");
        for times in svg.split(r#"keyTimes=""#).skip(1).map(|r| r.split('"').next().unwrap()) {
            let v: Vec<f64> = times.split(';').map(|x| x.parse().unwrap()).collect();
            assert!(v.windows(2).all(|w| w[0] <= w[1]) && v[0] == 0.0 && *v.last().unwrap() == 1.0, "{times}");
        }
    }

    #[test]
    fn the_time_asked_for_is_kept() {
        let svg = crate::pen("x", &crate::Options::default(), &Pen { seconds: Some(4.0), hold: 1.0, ..Pen::default() }).unwrap();
        assert!(svg.contains(r#"data-context-equation-pen="5.25""#), "{}", &svg[..300]);
    }
}
