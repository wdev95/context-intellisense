//! From the tree to a drawing (spec/rules.md): atoms placed by their bonds' directions, labels
//! typeset by contextEquation and attached by the right piece, bonds cut at the labels, double bonds inside
//! their ring, wedges, hashes, aromatic circles, Lewis marks; then the compounds of a scheme in a
//! row (or along each arrow's direction), the reaction arrows with their labels and the curved
//! arrows of a mechanism.

use std::collections::BTreeMap;

use super::draw::*;
use super::model::*;
use super::scheme::R;
use crate::layout::{Context, MathBox, Style};

/// One em in font units.
const EM: f32 = 1000.0;
/// C-LAY-1: a standard bond is 2.2 em (chemfig's default is 3 em; the ACS style, 1.44 em).
pub const BOND: f32 = 2.2 * EM;
/// C-LAY-2: strokes 0.055 em wide; the second line of a double bond 0.22 em away.
pub const STROKE: f32 = 0.055 * EM;
const DOUBLE: f32 = 0.22 * EM;
const TRIPLE: f32 = 0.2 * EM;
/// Half the width of a wedge at its wide end.
const WEDGE: f32 = 0.11 * EM;
/// A bond stops this far from the letters of a label.
const LABEL_MARGIN: f32 = 0.09 * EM;
/// Half the height of a label's letters, around the atom's position.
const LETTER_HALF: f32 = 0.36 * EM;

/// The equation's layout context, and the style text is set in inside the drawing.
pub struct Ctx<'a, 'r> {
    pub layout: &'a mut Context<'r>,
    pub style: Style,
}

impl Ctx<'_, '_> {
    /// LaTeX read and composed like any part of the equation, in text style.
    pub fn typeset(&mut self, tex: &str) -> R<MathBox> {
        if tex.trim().is_empty() {
            return Ok(MathBox::empty());
        }
        let node = crate::input::latex::read_with(tex, self.layout.locale, self.layout.registry, self.layout.strict)?;
        crate::layout::compose(&node, self.style, self.layout)
    }

    /// Text over or under an arrow, or a compound name: LaTeX text with `$…$` for math.
    pub fn text(&mut self, tex: &str) -> R<MathBox> {
        self.typeset(&super::scheme::text_to_tex(tex))
    }
}

/// A scheme as a box of the equation: drawn at text size in units of 1000 per em, centered on the math
/// axis, then scaled to the style (a structure in a subscript is a script-size structure).
pub fn compose(s: &Scheme, e: Style, layout: &mut Context) -> R<MathBox> {
    let font = e.font();
    let per_em = font.units_per_em() / EM;
    let text = Style { level: crate::layout::Level::Text, cramped: false, font: e.font };
    let drawing = {
        let mut ctx = Ctx { layout, style: text };
        scheme(s, &mut ctx)?
    };
    let mut b = drawing.to_box();
    let bounds = drawing.bounds();
    let axis = font.c(|c| c.axis_height()) / per_em;
    let dy = axis - (bounds.y0 + bounds.y1) / 2.0;
    b.shift_vertical(dy);
    b.height = bounds.y1 + dy;
    b.depth = -(bounds.y0 + dy);
    let f = e.scale() * per_em;
    if (f - 1.0).abs() > 1e-6 {
        b.scale_box(f);
    }
    Ok(b)
}

/// Where each atom ended up, for the mechanism arrows.
pub struct Hooks {
    pub points: BTreeMap<String, (P, Rect)>,
}

/// One label piece, typeset, with its horizontal place inside the label.
struct Piece {
    math_box: MathBox,
    /// From the left edge of the label to the piece's left edge.
    x: f32,
}

struct Label {
    pieces: Vec<Piece>,
}

impl Label {
    fn center(&self, k: usize) -> f32 {
        self.pieces.get(k).map(|p| p.x + p.math_box.width / 2.0).unwrap_or(0.0)
    }
}

pub fn molecule(m: &Molecule, ctx: &mut Ctx, hooks: &mut Hooks) -> R<Drawing> {
    // the labels, piece by piece, upright as chemfig prints them
    let mut labels: Vec<Option<Label>> = Vec::with_capacity(m.atoms.len());
    for atom in &m.atoms {
        if atom.pieces.is_empty() {
            labels.push(None);
            continue;
        }
        let mut pieces = Vec::new();
        let mut x = 0.0;
        for p in &atom.pieces {
            let math_box = ctx.typeset(&format!("\\mathrm{{{p}}}"))?;
            let w = math_box.width;
            pieces.push(Piece { math_box, x });
            x += w;
        }
        labels.push(Some(Label { pieces }));
    }
    // the point of each piece, relative to its atom's position (the anchor piece's center)
    let offset = |atom: AtomId, k: usize| -> f32 {
        match &labels[atom] {
            Some(l) => l.center(k) - l.center(m.atoms[atom].anchor),
            None => 0.0,
        }
    };

    // C-LAY-3: each atom from the bond that first reaches it
    let mut pos: Vec<Option<P>> = vec![None; m.atoms.len()];
    if !pos.is_empty() {
        pos[0] = Some((0.0, 0.0));
    }
    for b in &m.bonds {
        let (Some(from), None) = (pos[b.from], pos[b.to]) else { continue };
        let start = (from.0 + offset(b.from, b.departure), from.1);
        let end = add(start, mul(direction(b.angle), BOND * b.length));
        pos[b.to] = Some((end.0 - offset(b.to, b.arrival), end.1));
    }
    let pos: Vec<P> = pos.into_iter().map(|p| p.unwrap_or((0.0, 0.0))).collect();
    let point = |atom: AtomId, k: usize| -> P { (pos[atom].0 + offset(atom, k), pos[atom].1) };
    // the box a bond stops at: the letters of one piece
    // (`margin`: the space between the bond and the letters, C-LAY-3, or the bond's own #(…))
    let stop = |atom: AtomId, k: usize, margin: f32| -> Option<Rect> {
        let l = labels[atom].as_ref()?;
        let piece = l.pieces.get(k)?;
        let left = pos[atom].0 - l.center(m.atoms[atom].anchor) + piece.x;
        let y = pos[atom].1;
        // \chemskipalign: the baseline on the bond's line, the letters' own height above it
        let (below, above) = if m.atoms[atom].skip_align { (piece.math_box.depth, piece.math_box.height) } else { (LETTER_HALF, LETTER_HALF) };
        Some(Rect { x0: left - margin, y0: y - below - margin, x1: left + piece.math_box.width + margin, y1: y + above + margin })
    };
    let centers: Vec<P> = m
        .rings
        .iter()
        .map(|r| {
            let n = r.atoms.len().max(1) as f32;
            let s = r.atoms.iter().fold((0.0, 0.0), |acc, a| add(acc, pos[*a]));
            mul(s, 1.0 / n)
        })
        .collect();

    let mut d = Drawing::default();
    for b in &m.bonds {
        let (mut a, mut z) = (point(b.from, b.departure), point(b.to, b.arrival));
        let dir = unit(sub(z, a));
        if let Some(r) = stop(b.from, b.departure, b.start_gap.map_or(LABEL_MARGIN, |g| g * EM)) {
            a = r.exit(a, dir);
        }
        if let Some(r) = stop(b.to, b.arrival, b.end_gap.map_or(LABEL_MARGIN, |g| g * EM)) {
            z = r.exit(z, mul(dir, -1.0));
        }
        if length(sub(z, a)) < 1.0 || dir.0 * (z.0 - a.0) + dir.1 * (z.1 - a.1) <= 0.0 {
            continue;
        }
        bond(&mut d, b, a, z, b.ring.map(|r| centers[r]));
        // a node on the bond, of no size, where a mechanism arrow can point
        if let Some((name, coeff)) = &b.hook {
            let at = add(a, mul(sub(z, a), *coeff));
            hooks.points.insert(name.clone(), (at, Rect::around(at, 0.0)));
        }
    }
    // C-LAY-5: an aromatic ring's circle, at 0.6 of its inner radius
    for (r, c) in m.rings.iter().zip(&centers) {
        if r.aromatic && r.atoms.len() >= 3 {
            let mean = r.atoms.iter().map(|a| length(sub(pos[*a], *c))).sum::<f32>() / r.atoms.len() as f32;
            let inner = mean * (std::f32::consts::PI / r.size as f32).cos();
            match (r.arc, r.dashed) {
                (None, false) => d.circle(*c, inner * 0.6, STROKE, false),
                (arc, dashed) => {
                    let (from, to) = arc.unwrap_or((0.0, 360.0));
                    d.arc(*c, inner * 0.6, from, to, STROKE, dashed.then_some((0.12 * EM, 0.09 * EM)));
                }
            }
        }
    }
    let half_letter = LETTER_HALF;
    for (id, atom) in m.atoms.iter().enumerate() {
        let here = pos[id];
        let mut around = Rect::around(here, 0.12 * EM);
        if let Some(l) = &labels[id] {
            let left = here.0 - l.center(atom.anchor);
            // C-LAY-4: the letters' middle on the atom's position; C-LAY-8: with \chemskipalign, the baseline
            let baseline = if atom.skip_align { here.1 } else { here.1 - half_letter + 0.02 * EM };
            if atom.attributes.is_empty() {
                for p in &l.pieces {
                    d.math(p.math_box.clone(), (left + p.x, baseline));
                }
            } else {
                // a marked atom: its pieces in one group that carries the mark
                let mut whole = crate::layout::MathBox::empty();
                for p in &l.pieces {
                    whole.place(&p.math_box, p.x, 0.0);
                    whole.width = whole.width.max(p.x + p.math_box.width);
                }
                whole.group(atom.attributes.clone());
                d.math(whole, (left, baseline));
            }
            let a = l.pieces.get(atom.anchor).map(|p| (left + p.x, left + p.x + p.math_box.width)).unwrap_or((here.0, here.0));
            around = Rect { x0: a.0, y0: here.1 - half_letter, x1: a.1, y1: here.1 + half_letter };
        } else if !atom.attributes.is_empty() {
            // a marked vertex without a label (a carbon of the skeleton): an empty group on the vertex
            let mut point = crate::layout::MathBox::empty();
            point.group(atom.attributes.clone());
            d.math(point, here);
        }
        for mark in &atom.lewis {
            lewis(&mut d, around, *mark);
        }
        // C-LAY-9: \charge, around the box of its piece (an empty label: around the vertex)
        for charge in &atom.charges {
            let piece_box = labels[id].as_ref().and_then(|l| {
                let p = l.pieces.get(charge.piece)?;
                let x = here.0 - l.center(atom.anchor) + p.x;
                let baseline = if atom.skip_align { here.1 } else { here.1 - half_letter + 0.02 * EM };
                Some(Rect { x0: x, y0: baseline - p.math_box.depth, x1: x + p.math_box.width, y1: baseline + p.math_box.height })
            });
            let piece_box = piece_box.unwrap_or(Rect::around(here, 0.0));
            charge_mark(&mut d, ctx, piece_box, charge)?;
        }
        if let Some(h) = &atom.hook {
            hooks.points.insert(h.clone(), (here, around));
        }
    }
    Ok(d)
}

fn bond(d: &mut Drawing, b: &Bond, a: P, z: P, ring_center: Option<P>) {
    let dir = unit(sub(z, a));
    let n = left(dir);
    let len = length(sub(z, a));
    match b.kind {
        BondKind::Single => d.line(a, z, STROKE),
        BondKind::Double | BondKind::DoubleLeft | BondKind::DoubleRight => {
            // C-LAY-6: in a ring the second line goes inside, shortened at both ends; chemfig's offset
            // bonds put it on the side they name, in a ring or not
            let side = match (b.kind, ring_center) {
                (BondKind::DoubleLeft, _) => 1.0,
                (BondKind::DoubleRight, _) => -1.0,
                (_, Some(c)) if (c.0 - a.0) * n.0 + (c.1 - a.1) * n.1 >= 0.0 => 1.0,
                (_, Some(_)) => -1.0,
                (_, None) => {
                    let off = mul(n, DOUBLE / 2.0);
                    d.line(add(a, off), add(z, off), STROKE);
                    d.line(sub(a, off), sub(z, off), STROKE);
                    return;
                }
            };
            d.line(a, z, STROKE);
            let off = mul(n, DOUBLE * side);
            let trim = (0.16 * len).min(0.4 * EM);
            d.line(add(add(a, off), mul(dir, trim)), sub(add(z, off), mul(dir, trim)), STROKE);
        }
        BondKind::Triple => {
            let off = mul(n, TRIPLE);
            d.line(a, z, STROKE);
            d.line(add(a, off), add(z, off), STROKE);
            d.line(sub(a, off), sub(z, off), STROKE);
        }
        BondKind::Wedge | BondKind::WedgeBack | BondKind::Hollow | BondKind::HollowBack => {
            let (narrow, wide) = if matches!(b.kind, BondKind::Wedge | BondKind::Hollow) { (a, z) } else { (z, a) };
            let points = vec![narrow, add(wide, mul(n, WEDGE)), sub(wide, mul(n, WEDGE))];
            d.polygon(points, matches!(b.kind, BondKind::Wedge | BondKind::WedgeBack), STROKE * 0.8);
        }
        BondKind::Hash | BondKind::HashBack => {
            let (narrow, wide) = if b.kind == BondKind::Hash { (a, z) } else { (z, a) };
            let along = unit(sub(wide, narrow));
            let count = ((len / (0.13 * EM)).floor() as usize).max(3);
            for i in 0..=count {
                let t = i as f32 / count as f32;
                let p = add(narrow, mul(along, len * t));
                let half = (WEDGE * t).max(STROKE * 0.6);
                d.line(add(p, mul(n, half)), sub(p, mul(n, half)), STROKE * 0.8);
            }
        }
        BondKind::Dashed => d.dashed(a, z, STROKE, (0.12 * EM, 0.09 * EM)),
        BondKind::Invisible => {}
    }
}

/// C-LAY-7: a Lewis mark beside the atom, in its direction.
fn lewis(d: &mut Drawing, around: Rect, mark: Lewis) {
    let dir = direction(mark.angle);
    let c = around.center();
    let at = add(c, mul(dir, around.reach(dir) + 0.17 * EM + 0.08 * EM * dir.1.abs()));
    lewis_mark(d, at, dir, mark.mark);
}

/// chemfig's `extra sep`: the border charges are placed on is 1.5 pt outside the atom's box.
const CHARGE_SEP: f32 = 0.15 * EM;

/// C-LAY-9: a `\charge` (chemfig manual, section 5.8). Its position is the point at its angle on the
/// border of the piece's box grown by 1.5 pt, then its offset further out; the content's center goes
/// there, or, with `anchor`, the point of the content's border at that angle.
fn charge_mark(d: &mut Drawing, ctx: &mut Ctx, piece_box: Rect, charge: &Charge) -> R<()> {
    let grown = Rect { x0: piece_box.x0 - CHARGE_SEP, y0: piece_box.y0 - CHARGE_SEP, x1: piece_box.x1 + CHARGE_SEP, y1: piece_box.y1 + CHARGE_SEP };
    let dir = direction(charge.angle);
    let at = add(grown.center(), mul(dir, grown.reach(dir) + charge.offset * EM));
    match &charge.content {
        ChargeContent::Mark { mark } => {
            // a mark has no border to anchor: its center, as the shortcuts draw it
            lewis_mark(d, at, dir, *mark);
        }
        ChargeContent::Text { tex } => {
            let m = ctx.text(tex)?;
            let r = Rect { x0: 0.0, y0: -m.depth, x1: m.width, y1: m.height };
            let center = match charge.anchor {
                Some(a) => {
                    let ad = direction(a);
                    sub(at, mul(ad, r.reach(ad)))
                }
                None => at,
            };
            let origin = sub(center, r.center());
            d.math(m, origin);
        }
    }
    Ok(())
}

/// A Lewis mark centered at `at`, turned to face the direction `dir` it lies in from its atom.
fn lewis_mark(d: &mut Drawing, at: P, dir: P, mark: LewisMark) {
    let n = left(dir);
    let dot = 0.05 * EM;
    match mark {
        LewisMark::Pair => {
            d.circle(add(at, mul(n, 0.09 * EM)), dot, 0.0, true);
            d.circle(sub(at, mul(n, 0.09 * EM)), dot, 0.0, true);
        }
        LewisMark::Radical => d.circle(at, dot, 0.0, true),
        LewisMark::Line => d.line(add(at, mul(n, 0.15 * EM)), sub(at, mul(n, 0.15 * EM)), STROKE),
        LewisMark::Vacancy => {
            let h = 0.1 * EM;
            let r = |u: f32, v: f32| add(at, add(mul(n, u), mul(dir, v)));
            d.polygon(vec![r(-h, -h), r(h, -h), r(h, h), r(-h, h)], false, STROKE * 0.8);
        }
    }
}

/// Head of an arrow at `tip`, pointing along `u`: two curved barbs, or one for a half head.
fn head(d: &mut Drawing, tip: P, u: P, half: Option<f32>) {
    let (h, w) = (0.3 * EM, 0.14 * EM);
    let n = left(u);
    let sides: &[f32] = match half {
        Some(s) => &[s][..],
        None => &[1.0, -1.0][..],
    };
    for &side in sides {
        let corner = add(sub(tip, mul(u, h)), mul(n, w * side));
        let control = add(sub(tip, mul(u, h * 0.45)), mul(n, w * side * 0.18));
        // a quadratic as a cubic
        let c1 = add(tip, mul(sub(control, tip), 2.0 / 3.0));
        let c2 = add(corner, mul(sub(control, corner), 2.0 / 3.0));
        d.curve(tip, c1, c2, corner, STROKE);
    }
}

/// Where an arrow leaves from when it names one (section 6.7): a compound, or the box around a
/// `\subscheme`'s compounds (section 6.8).
fn arrow_source(arrow: &Arrow, placed: &[Rect]) -> Option<Rect> {
    if let Some((first, end)) = arrow.from_box {
        let inside = placed.get(first..end.min(placed.len()))?;
        return inside.iter().copied().reduce(Rect::union);
    }
    arrow.from.and_then(|k| placed.get(k).copied())
}

/// C-SCH-2: a reaction arrow from `a` to `b`.
fn reaction_arrow(d: &mut Drawing, kind: ArrowKind, a: P, b: P) {
    let u = unit(sub(b, a));
    let n = left(u);
    let len = length(sub(b, a));
    let gap = 0.075 * EM;
    match kind {
        ArrowKind::Forward | ArrowKind::NoReaction => {
            d.line(a, b, STROKE);
            head(d, b, u, None);
            if kind == ArrowKind::NoReaction {
                let m = mul(add(a, b), 0.5);
                let s = 0.18 * EM;
                d.line(add(m, add(mul(n, s), mul(u, -s * 0.6))), add(m, add(mul(n, -s), mul(u, s * 0.6))), STROKE);
                d.line(add(m, add(mul(n, s), mul(u, s * 0.6))), add(m, add(mul(n, -s), mul(u, -s * 0.6))), STROKE);
            }
        }
        ArrowKind::Backward => {
            d.line(a, b, STROKE);
            head(d, a, mul(u, -1.0), None);
        }
        ArrowKind::Resonance => {
            d.line(a, b, STROKE);
            head(d, b, u, None);
            head(d, a, mul(u, -1.0), None);
        }
        ArrowKind::Equilibrium | ArrowKind::EquilibriumRight | ArrowKind::EquilibriumLeft => {
            // two harpoons: the upper one forward with its barb up, the lower one back with its barb down
            let short = 0.22 * len;
            let (fa, fb) = if kind == ArrowKind::EquilibriumLeft { (add(a, mul(u, short)), sub(b, mul(u, short))) } else { (a, b) };
            let (ba, bb) = if kind == ArrowKind::EquilibriumRight { (add(a, mul(u, short)), sub(b, mul(u, short))) } else { (a, b) };
            let up = mul(n, gap);
            d.line(add(fa, up), add(fb, up), STROKE);
            head(d, add(fb, up), u, Some(1.0));
            d.line(sub(ba, up), sub(bb, up), STROKE);
            head(d, sub(ba, up), mul(u, -1.0), Some(1.0));
        }
        ArrowKind::Retro => {
            // an open double arrow: two shafts and a wide head
            let off = mul(n, 0.07 * EM);
            let end = sub(b, mul(u, 0.12 * EM));
            d.line(add(a, off), add(end, off), STROKE);
            d.line(sub(a, off), sub(end, off), STROKE);
            let (h, w) = (0.28 * EM, 0.22 * EM);
            d.line(b, add(sub(b, mul(u, h)), mul(n, w)), STROKE);
            d.line(b, sub(sub(b, mul(u, h)), mul(n, w)), STROKE);
        }
        ArrowKind::Invisible => {}
    }
}

/// C-SCH-1: an arrow is 2.6 em, or longer for its labels: 1 em more than the wider one; for `-U>`,
/// long enough that the ends of its arc are 0.3 em more apart than half the labels' widths.
fn base_length(arrow: &Arrow, above: &Drawing, below: &Drawing) -> f32 {
    let (wa, wb) = (above.bounds.map_or(0.0, |r| r.width()), below.bounds.map_or(0.0, |r| r.width()));
    match arrow.arc {
        Some(arc) => {
            // the ends of the arc are 2·r·sin(half angle) apart, r a fraction of the length
            let spread = 2.0 * arc.radius.abs() * arc.half_angle.min(90.0).to_radians().sin();
            let needed = if spread > 1e-3 { ((wa + wb) / 2.0 + 0.3 * EM) / spread } else { 0.0 };
            (2.6 * EM).max(needed)
        }
        None => (2.6 * EM).max(wa.max(wb) + EM),
    }
}

/// A scheme's arrow from `a` to `b` with its labels: on each side (C-SCH-3), or, for `-U>`, at the ends
/// of its arc (C-SCH-5).
fn draw_arrow(out: &mut Drawing, arrow: &Arrow, a: P, b: P, above: &Drawing, below: &Drawing) {
    reaction_arrow(out, arrow.kind, a, b);
    match arrow.arc {
        Some(arc) => u_arc(out, arc, a, b, above, below),
        None => arrow_labels(out, a, b, above, below),
    }
}

/// C-SCH-5: the arc of chemfig's `-U>` (manual, sections 6.2 and 6.9, and its `\definearrow` of `-U>`).
/// A circle of radius `radius` × the arrow's length touches the arrow at its middle, on its left (on
/// its right for a negative radius); from there one half turns back by `half_angle` toward the label
/// above, drawn when that label is given, and the other forward to the label below, drawn when that
/// label is given and ending in a head. Each label sits beyond its end of the arc, 3 pt away.
fn u_arc(out: &mut Drawing, arc: UArc, a: P, b: P, above: &Drawing, below: &Drawing) {
    let u = unit(sub(b, a));
    let theta = u.1.atan2(u.0).to_degrees();
    let radius = arc.radius * length(sub(b, a));
    let middle = mul(add(a, b), 0.5);
    let center = add(middle, mul(left(u), radius));
    let at = |phi: f32| add(center, mul(direction(phi), radius));
    let start = theta - 90.0;
    let ends = [start - arc.half_angle, start + arc.half_angle];
    let drawn = [above.bounds.is_some(), below.bounds.is_some()];
    for (k, &end) in ends.iter().enumerate() {
        if !drawn[k] {
            continue;
        }
        // in pieces of at most 45°, each a cubic
        let steps = ((end - start).abs() / 45.0).ceil().max(1.0) as usize;
        let step = (end - start) / steps as f32;
        for i in 0..steps {
            let (p0, p1) = (start + step * i as f32, start + step * (i + 1) as f32);
            let kappa = 4.0 / 3.0 * (step.to_radians() / 4.0).tan() * radius;
            let t0 = direction(p0 + 90.0);
            let t1 = direction(p1 + 90.0);
            out.curve(at(p0), add(at(p0), mul(t0, kappa)), sub(at(p1), mul(t1, kappa)), at(p1), STROKE);
        }
        if k == 1 {
            let tangent = mul(direction(end + 90.0), (step * radius).signum());
            head(out, at(end), unit(tangent), None);
        }
    }
    // the labels: above at the first end and below at the second (swapped below the arrow), on the
    // arc's side of the arrow, 3 pt from the ends
    let (first, second) = if radius > 0.0 { (above, below) } else { (below, above) };
    let (p, q) = (at(ends[0]), at(ends[1]));
    let n = mul(left(u), radius.signum());
    for (label, point) in [(first, p), (second, q)] {
        if label.bounds.is_none() {
            continue;
        }
        let r = label.bounds();
        let center = add(point, mul(n, 0.3 * EM + r.reach(n)));
        out.place(label, sub(center, r.center()));
    }
}

/// Where a compound sits in a scheme, and what it holds.
struct Placed {
    drawing: Drawing,
}

pub fn scheme(s: &Scheme, ctx: &mut Ctx) -> R<Drawing> {
    let mut hooks = Hooks { points: BTreeMap::new() };
    let mut out = Drawing::default();
    // the cursor: the box of the last thing placed, and the direction the next one goes in
    let mut last: Option<Rect> = None;
    let mut pending: Option<(&Arrow, Drawing, Drawing)> = None;
    let mut plus_pending = false;
    let gap = 0.45 * EM;
    // the compounds placed so far, in order, each as the box of what chemfig takes for one compound,
    // what `\+` joins included (a \merge or a branching arrow names them by index), the first of the
    // group being filled, and where a \merge left the border of the next one, with its direction
    let mut placed_boxes: Vec<Rect> = Vec::new();
    let mut group_start = 0;
    let mut after_merge: Option<(P, P)> = None;
    for item in &s.items {
        match item {
            Item::Compound(c) => {
                let mut local = Hooks { points: BTreeMap::new() };
                let placed = compound(c, ctx, &mut local)?;
                let b = placed.drawing.bounds();
                let merged = after_merge.take().map(|(at, u)| sub(add(at, mul(u, b.reach(u))), b.center()));
                let target = match (&pending, last, plus_pending) {
                    _ if merged.is_some() => merged.unwrap_or_default(),
                    (Some((arrow, above, below)), Some(prev), _) => {
                        // C-SCH-1: the arrow leaves the previous compound along its direction
                        let u = direction(arrow.angle);
                        let prev = arrow_source(arrow, &placed_boxes).unwrap_or(prev);
                        let len = base_length(arrow, above, below) * arrow.length;
                        let start = add(prev.center(), mul(u, prev.reach(u) + gap));
                        // a compound moved by its style (xshift, yshift): the arrow reaches it there
                        let moved = arrow.shift.map_or((0.0, 0.0), |(x, y)| (x * EM, y * EM));
                        let end = add(add(start, mul(u, len)), moved);
                        draw_arrow(&mut out, arrow, start, end, above, below);
                        let mut center = add(sub(end, moved), mul(u, gap + b.reach(u)));
                        if plus_pending {
                            // a \+ right after the arrow belongs to the compound it brings (chemfig manual,
                            // section 6.12, "Alignement du signe +"): the sign first, then the compound
                            let plus = ctx.typeset("+")?;
                            let space = 0.35 * EM;
                            let half = (plus.width / 2.0, (plus.height + plus.depth) / 2.0);
                            let sign = Rect { x0: -half.0, y0: -half.1, x1: half.0, y1: half.1 };
                            let mid = add(sub(end, moved), mul(u, sign.reach(u)));
                            out.math(plus.clone(), (mid.0 - half.0, mid.1 - (plus.height - plus.depth) / 2.0));
                            center = add(mid, mul(u, sign.reach(u) + space + b.reach(u)));
                        }
                        sub(add(center, moved), b.center())
                    }
                    (None, Some(prev), true) => {
                        let plus = ctx.typeset("+")?;
                        let space = 0.35 * EM;
                        let x = prev.x1 + space;
                        let mid = prev.center().1;
                        let baseline = mid - (plus.height - plus.depth) / 2.0;
                        let w = plus.width;
                        out.math(plus, (x, baseline));
                        (x + w + space - b.x0, mid - b.center().1)
                    }
                    (None, Some(prev), false) => (prev.x1 + 0.5 * EM - b.x0, prev.center().1 - b.center().1),
                    _ => (-b.x0, -b.center().1),
                };
                out.place(&placed.drawing, target);
                for (k, (p, r)) in local.points {
                    hooks.points.insert(k, (add(p, target), r.shifted(target)));
                }
                last = Some(b.shifted(target));
                let r = b.shifted(target);
                if plus_pending && !placed_boxes.is_empty() {
                    let whole = placed_boxes[group_start..].iter().fold(r, |a, b| a.union(*b));
                    placed_boxes.push(whole);
                    placed_boxes[group_start..].iter_mut().for_each(|x| *x = whole);
                } else {
                    group_start = placed_boxes.len();
                    placed_boxes.push(r);
                }
                pending = None;
                plus_pending = false;
            }
            Item::Plus => plus_pending = true,
            Item::Merge(m) => {
                if let (Some((arrow, ab, be)), Some(prev)) = (pending.take(), last) {
                    // an arrow with no compound after it ends in empty space, as two arrows in a row do
                    let u = direction(arrow.angle);
                    let prev = arrow_source(arrow, &placed_boxes).unwrap_or(prev);
                    let start = add(prev.center(), mul(u, prev.reach(u) + gap));
                    let end = add(start, mul(u, 2.6 * EM * arrow.length));
                    draw_arrow(&mut out, arrow, start, end, &ab, &be);
                }
                let above = label_drawing(&m.above, ctx)?;
                let below = label_drawing(&m.below, ctx)?;
                if let Some((end, u)) = merge(&mut out, m, &placed_boxes, &above, &below) {
                    last = Some(Rect::around(end, 0.0));
                    after_merge = Some((add(end, mul(u, gap)), u));
                }
                plus_pending = false;
            }
            Item::Arrow(a) => {
                let above = label_drawing(&a.above, ctx)?;
                let below = label_drawing(&a.below, ctx)?;
                let source = arrow_source(a, &placed_boxes).or(last);
                if let (Some(target), Some(source)) = (a.to.and_then(|k| placed_boxes.get(k).copied()), source) {
                    // chemfig manual, section 6.7: an arrow between two compounds already placed, straight
                    // from border to border; the next arrow leaves from its target
                    let u = unit(sub(target.center(), source.center()));
                    let start = add(source.center(), mul(u, source.reach(u) + gap));
                    let end = sub(target.center(), mul(u, target.reach(u) + gap));
                    draw_arrow(&mut out, a, start, end, &above, &below);
                    last = Some(target);
                    continue;
                }
                if let (Some((arrow, ab, be)), Some(prev)) = (pending.take(), last) {
                    // two arrows in a row: the first one ends in empty space
                    let u = direction(arrow.angle);
                    let prev = arrow_source(arrow, &placed_boxes).unwrap_or(prev);
                    let start = add(prev.center(), mul(u, prev.reach(u) + gap));
                    let end = add(start, mul(u, 2.6 * EM * arrow.length));
                    draw_arrow(&mut out, arrow, start, end, &ab, &be);
                    last = Some(Rect::around(end, 0.0));
                }
                pending = Some((a, above, below));
            }
        }
    }
    if let (Some((arrow, above, below)), Some(prev)) = (pending, last) {
        let u = direction(arrow.angle);
        let prev = arrow_source(arrow, &placed_boxes).unwrap_or(prev);
        let start = add(prev.center(), mul(u, prev.reach(u) + gap));
        let end = add(start, mul(u, base_length(arrow, &above, &below) * arrow.length));
        draw_arrow(&mut out, arrow, start, end, &above, &below);
    }
    for m in &s.moves {
        electron_move(&mut out, m, &hooks)?;
    }
    Ok(out)
}

/// An arrow's label drawn: text, or a structure at 0.85 of its size; nothing without one.
fn label_drawing(l: &Option<super::model::Label>, ctx: &mut Ctx) -> R<Drawing> {
    let mut d = Drawing::default();
    match l {
        None => {}
        Some(super::model::Label::Text { tex }) => {
            let m = ctx.text(tex)?;
            d.math(m, (0.0, 0.0));
        }
        Some(super::model::Label::Structure(m)) => {
            let mut ignored = Hooks { points: BTreeMap::new() };
            d = molecule(m, ctx, &mut ignored)?.scaled(0.85);
        }
    }
    Ok(d)
}

/// C-SCH-6: chemfig's `\merge` (manual, section 6.11, and its `\CF_merge`). From each compound joined, at
/// the anchor on its border (the direction's, or the angle given), a line along the direction to the
/// junction line, which stands across the direction `c1` × the space between compounds beyond the
/// farthest anchor and spans the anchors; from the point `c` along it (from its top, or its left end
/// for `^` and `v`), an arrow `c2` × that space long, ending where the next compound's border goes.
/// The space between compounds is contextEquation's: an arrow (2.6 em) and the gap on each side. Returns the
/// arrow's end and direction.
fn merge(out: &mut Drawing, m: &super::model::Merge, placed: &[Rect], above: &Drawing, below: &Drawing) -> Option<(P, P)> {
    let gap = 0.45 * EM;
    let sep = 2.6 * EM + 2.0 * gap;
    let u = direction(m.angle);
    let anchors: Vec<P> = m
        .from
        .iter()
        .filter_map(|&(k, angle)| {
            let r = placed.get(k)?;
            let d = angle.map_or(u, direction);
            Some(add(r.center(), mul(d, r.reach(d))))
        })
        .collect();
    if anchors.is_empty() {
        return None;
    }
    let along = |p: P| p.0 * u.0 + p.1 * u.1;
    let (c1, c2, c) = m.coefficients;
    let junction = anchors.iter().map(|p| along(*p)).fold(f32::MIN, f32::max) + c1 * sep;
    let horizontal = u.0.abs() > 0.5;
    for &a in &anchors {
        let start = add(a, mul(u, gap));
        let end = add(a, mul(u, junction - along(a)));
        if along(end) > along(start) {
            out.line(start, end, STROKE);
        }
    }
    let (xs, ys): (Vec<f32>, Vec<f32>) = anchors.iter().copied().unzip();
    let (x_min, x_max) = (xs.iter().copied().fold(f32::MAX, f32::min), xs.iter().copied().fold(f32::MIN, f32::max));
    let (y_min, y_max) = (ys.iter().copied().fold(f32::MAX, f32::min), ys.iter().copied().fold(f32::MIN, f32::max));
    // drawn from its top down (from its left end for ^ and v), as chemfig draws it
    let (first, last) = if horizontal { ((junction * u.0, y_max), (junction * u.0, y_min)) } else { ((x_min, junction * u.1), (x_max, junction * u.1)) };
    if length(sub(last, first)) > 1e-3 {
        out.line(first, last, STROKE);
    }
    let point = add(first, mul(sub(last, first), c));
    let end = add(point, mul(u, (c2 * sep - gap).max(0.3 * EM)));
    draw_arrow(out, &m.as_arrow(), point, end, above, below);
    Some((end, u))
}

fn compound(c: &Compound, ctx: &mut Ctx, hooks: &mut Hooks) -> R<Placed> {
    let mut drawing = match &c.body {
        Body::Structure(m) => molecule(m, ctx, hooks)?,
        Body::Math { tex } => {
            let mut d = Drawing::default();
            let m = ctx.typeset(tex)?;
            d.math(m, (0.0, 0.0));
            d
        }
    };
    if let Some(name) = &c.name {
        // chemfig manual, section 6.3: `\\` breaks the name into lines, each centered under the compound
        let b = drawing.bounds();
        let mut y = b.y0 - 0.35 * EM;
        for line in split_lines(name) {
            let m = ctx.text(&line)?;
            let x = b.center().0 - m.width / 2.0;
            y -= m.height;
            drawing.math(m.clone(), (x, y));
            y -= m.depth + 0.15 * EM;
        }
    }
    Ok(Placed { drawing })
}

/// A compound's name split at its `\\` outside braces: its lines, from the top.
pub(super) fn split_lines(name: &str) -> Vec<String> {
    let c: Vec<char> = name.chars().collect();
    let (mut lines, mut line, mut level, mut i) = (Vec::new(), String::new(), 0i32, 0);
    while i < c.len() {
        match c[i] {
            '\\' if level == 0 && c.get(i + 1) == Some(&'\\') => {
                lines.push(std::mem::take(&mut line));
                i += 2;
                continue;
            }
            '\\' => {
                line.push('\\');
                if let Some(&next) = c.get(i + 1) {
                    line.push(next);
                    i += 1;
                }
            }
            '{' => {
                level += 1;
                line.push('{');
            }
            '}' => {
                level -= 1;
                line.push('}');
            }
            ch => line.push(ch),
        }
        i += 1;
    }
    lines.push(line);
    lines.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

/// The label above an arrow goes on its left side, the one below on its right (C-SCH-3).
fn arrow_labels(out: &mut Drawing, a: P, b: P, above: &Drawing, below: &Drawing) {
    let u = unit(sub(b, a));
    let n = left(u);
    let middle = mul(add(a, b), 0.5);
    for (label, side) in [(above, 1.0f32), (below, -1.0)] {
        if label.bounds.is_none() {
            continue;
        }
        let r = label.bounds();
        let nn = mul(n, side);
        let center = add(middle, mul(nn, 0.24 * EM + r.reach(nn)));
        out.place(label, sub(center, r.center()));
    }
}

/// C-SCH-4: a curved arrow from one hooked atom to another.
fn electron_move(out: &mut Drawing, m: &ElectronMove, hooks: &Hooks) -> R<()> {
    // the reader already dropped arrows to unknown hooks; a hook inside an arrow's label is not drawn
    let (Some((p0, r0)), Some((p3, r3))) = (hooks.points.get(&m.from), hooks.points.get(&m.to)) else {
        return Ok(());
    };
    let chord = sub(*p3, *p0);
    let dist = length(chord).max(EM);
    let out_dir = m.out_angle.map(direction).unwrap_or_else(|| rotate(unit(chord), m.bend));
    let in_dir = m.in_angle.map(direction).unwrap_or_else(|| rotate(unit(mul(chord, -1.0)), -m.bend));
    let (r1, r2) = m.reach.map(|(a, b)| (a * EM, b * EM)).unwrap_or((0.45 * dist, 0.45 * dist));
    // the arrow starts and ends just outside the atoms' letters
    let start = add(*p0, mul(out_dir, r0.reach(out_dir) + 0.12 * EM));
    let end = add(*p3, mul(in_dir, r3.reach(in_dir) + 0.12 * EM));
    let c1 = add(start, mul(out_dir, r1));
    let c2 = add(end, mul(in_dir, r2));
    out.curve(start, c1, c2, end, STROKE);
    let tangent = unit(sub(end, c2));
    head(out, end, tangent, if m.single { Some(1.0) } else { None });
    Ok(())
}
