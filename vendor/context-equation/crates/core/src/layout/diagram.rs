//! Commutative diagrams with free arrows (tikz-cd, xymatrix): the cells sit on a grid, as in amscd,
//! and each arrow goes from the border of one cell to the border of another, in any direction.
//! Everything is drawn with straight strokes (`Item::Line`): a bent arrow is a cubic Bézier sampled
//! finely, as are the hooks and the curved barbs of the heads.

use crate::tree::{ArrowHead, ArrowLine, ArrowTail, DiagramArrow, LabelSide, Node};
use crate::Error;

use super::{em, k, Context, Item, Level, MathBox, Style};

type R<T> = Result<T, Error>;
type P = (f32, f32);

/// xy-pic's `@(d1,d2)` puts the curve's control points 3 pc from the entries, in the two directions
/// (xyarrow.tex, `\ar@curveinout`): 3.6 em at 10 pt.
const XY_CURVE_REACH: f32 = 3.6;

/// Points per bent arrow, per hook and per barb: enough for a smooth stroke at any display size.
const CURVE_STEPS: usize = 40;
const BARB_STEPS: usize = 8;

#[derive(Clone, Copy)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Rect {
    fn contains(&self, p: P) -> bool {
        p.0 >= self.x0 && p.0 <= self.x1 && p.1 >= self.y0 && p.1 <= self.y1
    }
}

fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

fn mul(a: P, f: f32) -> P {
    (a.0 * f, a.1 * f)
}

fn length(a: P) -> f32 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}

fn unit(a: P) -> P {
    let l = length(a);
    if l < 1e-6 {
        (1.0, 0.0)
    } else {
        (a.0 / l, a.1 / l)
    }
}

/// The left normal of a direction (y points up).
fn left(d: P) -> P {
    (-d.1, d.0)
}

fn rotate(d: P, radians: f32) -> P {
    let (s, c) = radians.sin_cos();
    (d.0 * c - d.1 * s, d.0 * s + d.1 * c)
}

/// Where the ray from `p` (inside `r`) along `d` leaves the rectangle.
fn exit(r: Rect, p: P, d: P) -> P {
    let mut t = f32::INFINITY;
    if d.0 > 1e-6 {
        t = t.min((r.x1 - p.0) / d.0);
    } else if d.0 < -1e-6 {
        t = t.min((r.x0 - p.0) / d.0);
    }
    if d.1 > 1e-6 {
        t = t.min((r.y1 - p.1) / d.1);
    } else if d.1 < -1e-6 {
        t = t.min((r.y0 - p.1) / d.1);
    }
    // (`d` has a length of one, so one side stops it; nowhere for a ray that never leaves)
    add(p, mul(d, if t.is_finite() { t.max(0.0) } else { 0.0 }))
}

/// xy-pic's `@(d1,d2)` (reference manual, note 24p): the cubic from the center `p` of the source to the
/// center `q` of the target whose control points are `reach` from them, in direction d1 from `p` and
/// d2 from `q`, so it leaves `p` in direction d1 and enters `q` from direction d2; cut at the borders
/// of the two entries (the same one for a loop).
fn in_out_curve(p: P, q: P, (d1, d2): (i16, i16), reach: f32, rf: Rect, rt: Rect) -> Vec<P> {
    let towards = |degrees: i16| {
        let (s, c) = (degrees as f32).to_radians().sin_cos();
        (c, s)
    };
    let (c1, c2) = (add(p, mul(towards(d1), reach)), add(q, mul(towards(d2), reach)));
    let full: Vec<P> = (0..=CURVE_STEPS * 2)
        .map(|s| {
            let t = s as f32 / (CURVE_STEPS * 2) as f32;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            (w[0] * p.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * q.0, w[0] * p.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * q.1)
        })
        .collect();
    let first = full.iter().position(|x| !rf.contains(*x)).unwrap_or(0);
    let last = full.iter().rposition(|x| !rt.contains(*x)).unwrap_or(full.len() - 1);
    if first >= last {
        Vec::new()
    } else {
        full[first..=last].to_vec()
    }
}

fn polyline_length(path: &[P]) -> f32 {
    path.windows(2).map(|w| length(sub(w[1], w[0]))).sum()
}

/// The point at arc length `s` along the path, and the direction there.
fn point_at(path: &[P], s: f32) -> (P, P) {
    let mut left_over = s.max(0.0);
    for w in path.windows(2) {
        let seg = length(sub(w[1], w[0]));
        if left_over <= seg || seg < 1e-6 && left_over <= 0.0 {
            let d = unit(sub(w[1], w[0]));
            return (add(w[0], mul(d, left_over)), d);
        }
        left_over -= seg;
    }
    let n = path.len();
    (path[n - 1], unit(sub(path[n - 1], path[n.saturating_sub(2)])))
}

/// The part of the path between arc lengths `a` and `b`.
fn slice(path: &[P], a: f32, b: f32) -> Vec<P> {
    let mut out = vec![point_at(path, a).0];
    let mut s = 0.0;
    for w in path.windows(2) {
        let seg = length(sub(w[1], w[0]));
        s += seg;
        if s > a && s < b {
            out.push(w[1]);
        }
    }
    out.push(point_at(path, b).0);
    out
}

/// The same path at a constant distance to its left (negative: to its right).
fn offset(path: &[P], distance: f32) -> Vec<P> {
    (0..path.len())
        .map(|i| {
            let before = if i > 0 { unit(sub(path[i], path[i - 1])) } else { unit(sub(path[1], path[0])) };
            let after = if i + 1 < path.len() { unit(sub(path[i + 1], path[i])) } else { before };
            let n = unit(add(left(before), left(after)));
            add(path[i], mul(n, distance))
        })
        .collect()
}

/// A finely sampled copy of the path, one point every `step`.
fn resample(path: &[P], step: f32) -> Vec<P> {
    let total = polyline_length(path);
    let n = ((total / step).ceil() as usize).max(1);
    (0..=n).map(|i| point_at(path, total * i as f32 / n as f32).0).collect()
}

struct Pen {
    items: Vec<Item>,
    thickness: f32,
    /// Bounding box of everything drawn so far.
    bounds: Option<Rect>,
}

impl Pen {
    fn grow(&mut self, r: Rect) {
        self.bounds = Some(match self.bounds {
            None => r,
            Some(b) => Rect { x0: b.x0.min(r.x0), y0: b.y0.min(r.y0), x1: b.x1.max(r.x1), y1: b.y1.max(r.y1) },
        });
    }

    fn point(&mut self, p: P) {
        let h = self.thickness;
        self.grow(Rect { x0: p.0 - h, y0: p.1 - h, x1: p.0 + h, y1: p.1 + h });
    }

    fn line(&mut self, a: P, b: P) {
        self.point(a);
        self.point(b);
        self.items.push(Item::Line { x1: a.0, y1: a.1, x2: b.0, y2: b.1, thickness: self.thickness, color: None });
    }

    fn stroke(&mut self, path: &[P]) {
        for w in path.windows(2) {
            self.line(w[0], w[1]);
        }
    }

    /// A round dot: a stroke shorter than its width, which the round cap fills.
    fn dot(&mut self, p: P) {
        let w = self.thickness * 0.02;
        self.line((p.0 - w, p.1), (p.0 + w, p.1));
    }
}

pub fn compose(cells: &[Vec<Node>], arrows: &[DiagramArrow], e: Style, ctx: &mut Context) -> R<MathBox> {
    let cell_style = e.not_display();
    let label_style = Style { level: if e.is_script() { Level::ScriptScript } else { Level::Script }, cramped: false, font: e.font };
    let nrow = cells.len();
    let ncol = cells.iter().map(Vec::len).max().unwrap_or(0);
    let mut boxes: Vec<Vec<MathBox>> = Vec::with_capacity(nrow);
    for row in cells {
        let mut out = Vec::with_capacity(ncol);
        for j in 0..ncol {
            out.push(match row.get(j) {
                Some(n) => super::compose(n, cell_style, ctx)?,
                None => MathBox::empty(),
            });
        }
        boxes.push(out);
    }
    let mut label_boxes: Vec<Vec<MathBox>> = Vec::with_capacity(arrows.len());
    for a in arrows {
        let mut out = Vec::new();
        for l in &a.labels {
            out.push(super::compose(&l.node, label_style, ctx)?);
        }
        label_boxes.push(out);
    }

    // rows and columns: the cells are centered in their column and share their row's baseline
    let (strut_height, strut_depth) = (em(cell_style, 0.7), em(cell_style, 0.25));
    let heights: Vec<f32> = boxes.iter().map(|r| r.iter().map(|b| b.height).fold(strut_height, f32::max)).collect();
    let depths: Vec<f32> = boxes.iter().map(|r| r.iter().map(|b| b.depth).fold(strut_depth, f32::max)).collect();
    let widths: Vec<f32> = (0..ncol).map(|j| boxes.iter().map(|r| r[j].width).fold(0.0, f32::max)).collect();
    // tikz-cd's default separations, widened where a label must fit beside a short arrow
    let mut column_gap = vec![em(e, 2.4); ncol.saturating_sub(1)];
    let mut row_gap = vec![em(e, 2.1); nrow.saturating_sub(1)];
    for (a, labels) in arrows.iter().zip(&label_boxes) {
        let widest = labels.iter().map(|b| b.width).fold(0.0, f32::max);
        let tallest = labels.iter().map(MathBox::total).fold(0.0, f32::max);
        if a.from.0 == a.to.0 && a.from.1.abs_diff(a.to.1) == 1 {
            let j = a.from.1.min(a.to.1);
            column_gap[j] = column_gap[j].max(widest + em(e, 1.4));
        }
        if a.from.1 == a.to.1 && a.from.0.abs_diff(a.to.0) == 1 && a.labels.iter().any(|l| l.side == LabelSide::On) {
            let i = a.from.0.min(a.to.0);
            row_gap[i] = row_gap[i].max(tallest + em(e, 1.2));
        }
    }
    let mut centers = Vec::with_capacity(ncol);
    let mut x = 0.0;
    for (j, w) in widths.iter().enumerate() {
        centers.push(x + w / 2.0);
        x += w + column_gap.get(j).copied().unwrap_or(0.0);
    }
    let mut bases = Vec::with_capacity(nrow);
    let mut y = 0.0;
    for i in 0..nrow {
        y -= heights[i];
        bases.push(y);
        y -= depths[i] + row_gap.get(i).copied().unwrap_or(0.0);
    }

    let axis = k(cell_style, |c| c.axis_height());
    let thickness = k(e, |c| c.fraction_rule_thickness()).max(em(e, 0.04));
    let mut pen = Pen { items: Vec::new(), thickness, bounds: None };
    let mut body = MathBox::empty();
    for (i, row) in boxes.iter().enumerate() {
        for (j, b) in row.iter().enumerate() {
            let x0 = centers[j] - b.width / 2.0;
            body.place(b, x0, bases[i]);
            if b.width > 0.0 {
                pen.grow(Rect { x0, y0: bases[i] - b.depth, x1: x0 + b.width, y1: bases[i] + b.height });
            }
        }
    }
    let anchor = |(i, j): (usize, usize)| -> P { (centers[j], bases[i] + axis) };
    let border = |(i, j): (usize, usize)| -> Rect {
        let b = &boxes[i][j];
        let (cx, cy) = anchor((i, j));
        if b.width == 0.0 && b.items.is_empty() {
            let p = em(e, 0.1);
            return Rect { x0: cx - p, y0: cy - p, x1: cx + p, y1: cy + p };
        }
        let p = em(e, 0.22);
        Rect { x0: cx - b.width / 2.0 - p, y0: bases[i] - b.depth - p, x1: cx + b.width / 2.0 + p, y1: bases[i] + b.height + p }
    };

    let barb = |double: bool| if double { (em(e, 0.34), em(e, 0.24)) } else { (em(e, 0.3), em(e, 0.15)) };
    for (a, labels) in arrows.iter().zip(&label_boxes) {
        let (p, q) = (anchor(a.from), anchor(a.to));
        let d = unit(sub(q, p));
        let shift = mul(left(d), em(e, a.shift));
        let (p, q) = (add(p, shift), add(q, shift));
        let theta = (a.bend as f32).to_radians();
        let out = rotate(d, theta);
        let arrival = rotate(d, -theta);
        let start = exit(border(a.from), p, out);
        let end = exit(border(a.to), q, mul(arrival, -1.0));
        // the straight segments of a path with waypoints, each between the borders it leaves and reaches
        let mut segments: Vec<(P, P)> = Vec::new();
        let path: Vec<P> = if a.across {
            // xy-pic's 2-cell Arrow: across the middle, clockwise, 0.6 em long, moved along itself by `sag`
            let along = mul(left(d), -1.0);
            let center = add(mul(add(p, q), 0.5), mul(along, em(e, a.sag)));
            vec![sub(center, mul(along, em(e, 0.3))), add(center, mul(along, em(e, 0.3)))]
        } else if a.sag != 0.0 {
            // a 2-cell curve: the quadratic through the middle moved `sag` to the left, cut at the borders
            let control = add(mul(add(p, q), 0.5), mul(left(d), em(e, a.sag)));
            let full: Vec<P> = (0..=CURVE_STEPS * 2)
                .map(|s| {
                    let t = s as f32 / (CURVE_STEPS * 2) as f32;
                    let u = 1.0 - t;
                    add(add(mul(p, u * u), mul(control, 2.0 * u * t)), mul(q, t * t))
                })
                .collect();
            let (rf, rt) = (border(a.from), border(a.to));
            let first = full.iter().position(|x| !rf.contains(*x)).unwrap_or(0);
            let last = full.iter().rposition(|x| !rt.contains(*x)).unwrap_or(full.len() - 1);
            if first >= last {
                vec![start, end]
            } else {
                full[first..=last].to_vec()
            }
        } else if let Some(directions) = a.directions {
            in_out_curve(p, q, directions, em(e, XY_CURVE_REACH), border(a.from), border(a.to))
        } else if !a.via.is_empty() {
            // xy-pic's straight segments (reference manual, note 24d): each one between the objects it
            // joins, so an entry with something in it interrupts the path; an empty one does not
            let mut stops = vec![a.from];
            stops.extend(&a.via);
            stops.push(a.to);
            let mut path = Vec::new();
            for w in stops.windows(2) {
                let (u, v) = (add(anchor(w[0]), shift), add(anchor(w[1]), shift));
                let dir = unit(sub(v, u));
                let empty = |c: (usize, usize)| boxes[c.0][c.1].width == 0.0 && boxes[c.0][c.1].items.is_empty();
                let s = if w[0] == a.from || !empty(w[0]) { exit(border(w[0]), u, dir) } else { u };
                let t = if w[1] == a.to || !empty(w[1]) { exit(border(w[1]), v, mul(dir, -1.0)) } else { v };
                segments.push((s, t));
                path.push(s);
                path.push(t);
            }
            path
        } else if a.bend == 0 {
            vec![start, end]
        } else {
            Vec::new()
        };
        let path: Vec<P> = if !path.is_empty() {
            path
        } else {
            let reach = length(sub(end, start)) * 0.3915 / theta.cos().max(0.2);
            let (c1, c2) = (add(start, mul(out, reach)), sub(end, mul(arrival, reach)));
            (0..=CURVE_STEPS)
                .map(|s| {
                    let t = s as f32 / CURVE_STEPS as f32;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    (w[0] * start.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * end.0, w[0] * start.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * end.1)
                })
                .collect()
        };
        let total = polyline_length(&path);
        // (a path of no length, which the borders an arrow leaves and reaches never give, draws nothing)
        let false = total < 1e-3 else { continue };

        // labels beside the arrow or on it, at their position along it (the middle by default)
        let mut breaks: Vec<Rect> = Vec::new();
        for (l, b) in a.labels.iter().zip(labels) {
            let (middle, along) = match segments.get(l.segment) {
                Some(&(s, t)) if !a.via.is_empty() => (add(s, mul(sub(t, s), l.position.clamp(0.0, 1.0))), unit(sub(t, s))),
                _ => point_at(&path, total * l.position.clamp(0.0, 1.0)),
            };
            let normal = left(along);
            let reach = normal.0.abs() * b.width / 2.0 + normal.1.abs() * b.total() / 2.0;
            // beside a 2-cell's Arrow, clear of its double head
            let gap = em(e, if a.across { 0.4 } else { 0.16 });
            let center = match l.side {
                LabelSide::Left => add(middle, mul(normal, gap + reach)),
                LabelSide::Right => sub(middle, mul(normal, gap + reach)),
                LabelSide::On => middle,
            };
            let x0 = center.0 - b.width / 2.0;
            let base = center.1 - (b.height - b.depth) / 2.0;
            body.place(b, x0, base);
            let r = Rect { x0, y0: base - b.depth, x1: x0 + b.width, y1: base + b.height };
            pen.grow(r);
            if l.side == LabelSide::On {
                let p = em(e, 0.12);
                breaks.push(Rect { x0: r.x0 - p, y0: r.y0 - p, x1: r.x1 + p, y1: r.y1 + p });
            }
        }

        // the line, shortened where a head or a tail must cover its end
        let double = a.line == ArrowLine::Double;
        let (h, _) = barb(double);
        let trim = |has: bool| {
            if !has {
                0.0
            } else if double {
                h * 0.55
            } else {
                thickness / 2.0
            }
        };
        let head_trim = trim(a.head != ArrowHead::None);
        let tail_trim = trim(a.tail == ArrowTail::Head);
        let body_path = slice(&path, tail_trim, (total - head_trim).max(tail_trim));
        // between two segments, the part inside a waypoint's entry is not drawn
        let gaps: Vec<(P, P)> = segments.windows(2).filter(|w| length(sub(w[1].0, w[0].1)) > 1e-3).map(|w| (w[0].1, w[1].0)).collect();
        let in_gap = |m: P| {
            gaps.iter().any(|&(g0, g1)| {
                let gl = length(sub(g1, g0));
                let t = ((m.0 - g0.0) * (g1.0 - g0.0) + (m.1 - g0.1) * (g1.1 - g0.1)) / (gl * gl);
                let closest = add(g0, mul(sub(g1, g0), t.clamp(0.0, 1.0)));
                (0.0..=1.0).contains(&t) && length(sub(m, closest)) < 1e-3
            })
        };
        let visible = |seg: (P, P)| {
            let m = mul(add(seg.0, seg.1), 0.5);
            !breaks.iter().any(|r| r.contains(m)) && !in_gap(m)
        };
        let draw = |pen: &mut Pen, line: &[P]| {
            let line = if breaks.is_empty() && gaps.is_empty() { line.to_vec() } else { resample(line, em(e, 0.04)) };
            for w in line.windows(2) {
                if visible((w[0], w[1])) {
                    pen.line(w[0], w[1]);
                }
            }
        };
        match a.line {
            ArrowLine::Solid => draw(&mut pen, &body_path),
            ArrowLine::Double => {
                let gap = em(e, 0.09);
                draw(&mut pen, &offset(&body_path, gap));
                draw(&mut pen, &offset(&body_path, -gap));
            }
            ArrowLine::Dashed => {
                let (on, off) = (em(e, 0.26), em(e, 0.16));
                let span = polyline_length(&body_path);
                let mut s = 0.0;
                while s < span {
                    draw(&mut pen, &slice(&body_path, s, (s + on).min(span)));
                    s += on + off;
                }
            }
            ArrowLine::Dotted => {
                let span = polyline_length(&body_path);
                let n = ((span / em(e, 0.2)).round() as usize).max(1);
                for s in 0..=n {
                    let (pt, _) = point_at(&body_path, span * s as f32 / n as f32);
                    if !breaks.iter().any(|r| r.contains(pt)) {
                        pen.dot(pt);
                    }
                }
            }
            ArrowLine::Squiggly => {
                let span = polyline_length(&body_path);
                let (amplitude, wave) = (em(e, 0.07), em(e, 0.3));
                let lead = em(e, 0.12).min(span / 4.0);
                let fine = resample(&body_path, em(e, 0.03));
                let mut s = 0.0;
                let mut wavy = Vec::with_capacity(fine.len());
                for (n, pt) in fine.iter().enumerate() {
                    if n > 0 {
                        s += length(sub(*pt, fine[n - 1]));
                    }
                    let (_, dir) = point_at(&body_path, s);
                    let k = if s < lead || s > span - lead { 0.0 } else { ((s - lead) / wave * std::f32::consts::TAU).sin() * amplitude };
                    wavy.push(add(*pt, mul(left(dir), k)));
                }
                draw(&mut pen, &wavy);
            }
            ArrowLine::None => {}
        }

        // heads and tails
        let (tip, tip_dir) = point_at(&path, total);
        let (root, root_dir) = point_at(&path, 0.0);
        let head = |pen: &mut Pen, tip: P, u: P| {
            let (h, w) = barb(double);
            let n = left(u);
            for side in [1.0f32, -1.0] {
                let corner = add(sub(tip, mul(u, h)), mul(n, w * side));
                let control = add(sub(tip, mul(u, h * 0.45)), mul(n, w * side * 0.18));
                let points: Vec<P> = (0..=BARB_STEPS)
                    .map(|s| {
                        let t = s as f32 / BARB_STEPS as f32;
                        let v = 1.0 - t;
                        (v * v * tip.0 + 2.0 * v * t * control.0 + t * t * corner.0, v * v * tip.1 + 2.0 * v * t * control.1 + t * t * corner.1)
                    })
                    .collect();
                pen.stroke(&points);
            }
        };
        match a.head {
            ArrowHead::Normal => head(&mut pen, tip, tip_dir),
            ArrowHead::TwoHeads => {
                head(&mut pen, tip, tip_dir);
                head(&mut pen, sub(tip, mul(tip_dir, em(e, 0.14))), tip_dir);
            }
            ArrowHead::None => {}
        }
        match a.tail {
            ArrowTail::None => {}
            ArrowTail::Head => head(&mut pen, root, mul(root_dir, -1.0)),
            ArrowTail::Tail => head(&mut pen, add(root, mul(root_dir, h * 0.8)), root_dir),
            ArrowTail::Bar => {
                let (_, w) = barb(double);
                let n = left(root_dir);
                pen.line(add(root, mul(n, w)), sub(root, mul(n, w)));
            }
            ArrowTail::Hook | ArrowTail::HookRight => {
                let side = if a.tail == ArrowTail::Hook { 1.0 } else { -1.0 };
                let n = mul(left(root_dir), side);
                let r = em(e, 0.1);
                let center = add(root, mul(n, r));
                let points: Vec<P> = (0..=CURVE_STEPS / 2)
                    .map(|s| {
                        let phi = std::f32::consts::PI * s as f32 / (CURVE_STEPS / 2) as f32;
                        add(center, add(mul(n, -r * phi.cos()), mul(root_dir, -r * phi.sin())))
                    })
                    .collect();
                pen.stroke(&points);
            }
        }
    }

    // the whole diagram centered on the math axis, its left edge at x = 0
    let bounds = pen.bounds.unwrap_or(Rect { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 });
    body.items.extend(pen.items);
    let dy = k(e, |c| c.axis_height()) - (bounds.y0 + bounds.y1) / 2.0;
    let mut math_box = MathBox::space(bounds.x1 - bounds.x0);
    math_box.place(&body, -bounds.x0, dy);
    math_box.height = bounds.y1 + dy;
    math_box.depth = -(bounds.y0 + dy);
    Ok(math_box)
}
