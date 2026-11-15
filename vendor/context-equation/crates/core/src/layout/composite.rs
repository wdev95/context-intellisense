//! #176: the stmaryrd and mathabx symbols Unicode has no character for (`crate::composite`), drawn
//! from the math font's own glyphs as the packages build them: overlapped (`\llceil` is two ⌈ a
//! kern apart, as stmaryaj.mf draws it), turned (`\Yleft` is ⅄ turned, as stmaryjg.mf's Y forms are
//! one drawing in four directions), mirrored (`\bignplus` is ⨄ upside down) or completed with
//! strokes at the font's thickness where the package draws a stroke the font has no glyph for (the
//! vee in `\varovee`, the slanted bar of `\trianglelefteqslant`, the curved arms of `\bigcurlyvee`
//! in the box of ⋁). The numbers come from the packages' METAFONT sources (stmaryjg.mf,
//! stmaryaj.mf) with cmsy10's parameters, as shares of the font's own glyphs.
//!
//! Out of line and cold: these run only for the markers, off the paths the deep recursions take.

use ttf_parser::GlyphId;

use crate::composite::{self, *};
use crate::font::Font;
use crate::Error;

use super::{em, glyph, k, large_operator, stretch, Item, Level, MathBox, Style};

type R<T> = Result<T, Error>;

/// The font's rule thickness, the pen stmaryrd draws its strokes with (`rule.nib`).
fn rule(e: Style) -> f32 {
    k(e, |c| c.fraction_rule_thickness())
}

fn axis(e: Style) -> f32 {
    k(e, |c| c.axis_height())
}

/// Where the ink of a box is: (x0, y0, x1, y1).
fn ink(b: &MathBox, e: Style) -> (f32, f32, f32, f32) {
    b.ink(e.font()).unwrap_or((0.0, -b.depth, b.width, b.height))
}

fn line(b: &mut MathBox, (x1, y1): (f32, f32), (x2, y2): (f32, f32), thickness: f32) {
    b.items.push(Item::Line { x1, y1, x2, y2, thickness, color: None });
}

/// The box as a symbol of its own: no italic correction, no accent point.
fn finish(mut b: MathBox) -> MathBox {
    b.italic = 0.0;
    b.accent_point = None;
    b.single_glyph = false;
    b
}

/// A marker drawn as a symbol (`Operator`): the font has no glyph for it. Anything else the font
/// lacks stays a missing glyph.
#[cold]
#[inline(never)]
pub(super) fn symbol(c: char, e: Style) -> R<MathBox> {
    let b = match c {
        LLCEIL | RRCEIL | LLFLOOR | RRFLOOR => doubled(glyph(composite::doubled(c).unwrap_or(' '), e)?, e),
        VAROCIRCLE | VAROLESSTHAN | VAROGREATERTHAN | VAROVEE | VAROWEDGE => circled(c, e)?,
        // stmaryjg.mf: Y_down is Y_up upside down, Y_left and Y_right are it turned a quarter, centered
        // on the axis
        YDOWN => turned(glyph('⅄', e)?, 180.0, false, e),
        YLEFT => turned(glyph('⅄', e)?, 90.0, true, e),
        YRIGHT => turned(glyph('⅄', e)?, -90.0, true, e),
        SHORTRIGHTARROW => short_arrow(c, e)?,
        SHORTLEFTARROW => {
            let mut b = short_arrow(c, e)?;
            b.mirror();
            b
        }
        // stmaryjg.mf draws the short vertical arrows as the horizontal one turned: the same head,
        // 12u long, centered on the axis
        SHORTUPARROW => turned(short_arrow(c, e)?, 90.0, true, e),
        SHORTDOWNARROW => turned(short_arrow(c, e)?, -90.0, true, e),
        // stmaryaj.mf's nne_arrow: the arrow from (u, -d) to (8u, h), 7u across and 16u up (asc_height
        // plus asc_depth), with the head of the others: → turned to that angle, centered on the axis
        NNEARROW => turned(glyph('→', e)?, 16f32.atan2(7.0).to_degrees(), true, e),
        ARROWNOT => arrow_not(e)?,
        MAPSFROMCHAR => maps_from_char(e)?,
        TRIANGLELEFTEQSLANT => triangle_eq_slant(e)?,
        INPLUS => in_plus(e)?,
        BOND_DASHED | BOND_DASHED_OVER_SINGLE | BOND_DASHED_OVER_DOUBLE | BOND_DASHED_BETWEEN => bond_lines(c, e)?,
        BOND_FOUR_DOTS => four_dots(e)?,
        _ => return Err(Error::MissingGlyph { character: c }),
    };
    Ok(finish(b))
}

/// #267: mhchem's bonds of lines (mhchem.sty's `\mhchem@bond` table): the font's − as wide and as thick
/// as it is, one dashed line on its middle (`~`), or raised and lowered 0.1 em for two lines (`~-`: `\raisebox{.1em}` the dashed one,
/// `\raisebox{-.1em}` the other) and 0.2 em for three (`~=`, `-~-`). A dashed line is mhchem's
/// `\tripledash`: three dashes, each a quarter of the line, with equal gaps between them.
fn bond_lines(c: char, e: Style) -> R<MathBox> {
    let minus = glyph('−', e)?;
    let (x0, y0, x1, y1) = ink(&minus, e);
    let (t, middle, length) = (y1 - y0, (y0 + y1) / 2.0, x1 - x0);
    // (raise in em, dashed)
    let rows: &[(f32, bool)] = match c {
        BOND_DASHED => &[(0.0, true)],
        BOND_DASHED_OVER_SINGLE => &[(0.1, true), (-0.1, false)],
        BOND_DASHED_OVER_DOUBLE => &[(0.2, true), (0.0, false), (-0.2, false)],
        _ => &[(0.2, false), (0.0, true), (-0.2, false)],
    };
    let mut b = MathBox { width: minus.width, ..MathBox::default() };
    for &(raise, dashed) in rows {
        let y = middle + em(e, raise) - t / 2.0;
        b.height = b.height.max(y + t);
        b.depth = b.depth.max(-y);
        if dashed {
            let dash = length / 4.0;
            let gap = (length - 3.0 * dash) / 2.0;
            for k in 0..3 {
                b.items.push(Item::Rule { x: x0 + k as f32 * (dash + gap), y, width: dash, height: t, color: None });
            }
        } else {
            b.items.push(Item::Rule { x: x0, y, width: length, height: t, color: None });
        }
    }
    Ok(b)
}

/// #267: mhchem's `\bond{....}`, `{\cdot}{\cdot}{\cdot}{\cdot}`: four of the font's ⋅ side by side.
fn four_dots(e: Style) -> R<MathBox> {
    let dot = glyph('⋅', e)?;
    let mut b = MathBox { width: 4.0 * dot.width, height: dot.height, depth: dot.depth, ..MathBox::default() };
    for k in 0..4 {
        b.place(&dot, k as f32 * dot.width, 0.0);
    }
    Ok(b)
}

/// A doubled ceiling or floor as a delimiter, `target` tall: the single one grown, twice.
#[cold]
#[inline(never)]
pub(super) fn doubled_delimiter(single: char, target: f32, e: Style) -> R<MathBox> {
    Ok(finish(doubled(stretch::vertical(single, target, e)?, e)))
}

/// stmaryaj.mf's ll_ceil and its kin: two stems `spread` apart (compute_spread(.45x_height, …),
/// with math_spread 0: 0.45 of the x-height) under one bar. Two of the font's ⌈ that far apart.
fn doubled(single: MathBox, e: Style) -> MathBox {
    let kern = 0.45 * e.font().x_height() * e.scale();
    let mut b = single.clone();
    b.place(&single, kern, 0.0);
    b.width = single.width + kern;
    b
}

/// stmaryjg.mf's begincircle forms: the font's ○ (its circled operators are circular and drawn with
/// the rule's thickness, as stmaryrd's are) with the inside stmaryaj.mf draws, from the points of the
/// circle's middle line (z2 at 0°, z1 at 45°, z8 at 90°… z3 at 315°), with the same pen.
fn circled(c: char, e: Style) -> R<MathBox> {
    let mut b = glyph('○', e)?;
    let (x0, y0, x1, y1) = ink(&b, e);
    let t = rule(e);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let r = (x1 - x0 - t) / 2.0;
    let point = |degrees: f32| {
        let (sin, cos) = degrees.to_radians().sin_cos();
        (cx + r * cos, cy + r * sin)
    };
    let path: &[f32] = match c {
        // z1--z6--z3, z5--z2--z7, z1--z4--z7, z3--z8--z5
        VAROLESSTHAN => &[45.0, 180.0, 315.0],
        VAROGREATERTHAN => &[225.0, 0.0, 135.0],
        VAROVEE => &[45.0, 270.0, 135.0],
        VAROWEDGE => &[315.0, 90.0, 225.0],
        _ => &[],
    };
    for pair in path.windows(2) {
        line(&mut b, point(pair[0]), point(pair[1]), t);
    }
    if c == VAROCIRCLE {
        // var_o_circle: the inner circle through the midpoints between the center and the circle
        b.items.push(Item::Ellipse { cx, cy, rx: r / 2.0, ry: r / 2.0, thickness: t, color: None });
    }
    Ok(b)
}

/// The box turned `degrees` counterclockwise around the middle of its ink, which stays where it was
/// or, with `on_axis`, goes to the math axis. The side bearings stay those of the box.
fn turned(mut b: MathBox, degrees: f32, on_axis: bool, e: Style) -> MathBox {
    let (x0, y0, x1, y1) = ink(&b, e);
    let bearing = (x0 + (b.width - x1)) / 2.0;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let snap = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (sin, cos) = (snap(sin), snap(cos));
    let (hw, hh) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let (w2, h2) = (cos.abs() * hw + sin.abs() * hh, sin.abs() * hw + cos.abs() * hh);
    let (nx, ny) = (bearing + w2, if on_axis { axis(e) } else { cy });
    b.transform([cos, sin, -sin, cos, nx - cos * cx + sin * cy, ny - sin * cx - cos * cy]);
    b.width = 2.0 * (bearing + w2);
    b.height = (ny + h2).max(0.0);
    b.depth = (h2 - ny).max(0.0);
    b
}

/// stmaryjg.mf's short_right_arrow: the head of → on a shaft 12u long where →'s is 16u, three
/// quarters of it. The head is the one the font assembles long arrows with (its MATH table's part),
/// the shaft a rule as thick as the assembly's extender.
fn short_arrow(c: char, e: Style) -> R<MathBox> {
    let f = e.font();
    let s = e.scale();
    let missing = Error::MissingGlyph { character: c };
    let arrow = glyph('→', e)?;
    let parts = f.glyph('→').and_then(|id| f.assembly(id, false)).ok_or(missing.clone())?;
    let head = parts.iter().rev().find(|p| !p.extender).ok_or(missing.clone())?;
    let shaft = parts.iter().find(|p| p.extender).ok_or(missing)?;
    let (x0, _, x1, _) = ink(&arrow, e);
    let length = 0.75 * (x1 - x0);
    let (hx0, _, hx1, _) = f.limits(head.id);
    let (_, sy0, _, sy1) = f.limits(shaft.id);
    let hx = x0 + length - hx1 * s;
    // the shaft runs under the head's own stub, never to its tip
    let shaft_end = hx + (hx0 + (hx1 - hx0) / 2.0) * s;
    let mut b = MathBox { width: length + x0 + (arrow.width - x1), height: arrow.height, depth: arrow.depth, ..MathBox::default() };
    b.items.push(Item::Rule { x: x0, y: sy0 * s, width: (shaft_end - x0).max(0.0), height: (sy1 - sy0) * s, color: None });
    b.items.push(Item::Glyph { id: head.id, x: hx, y: 0.0, scale: s, color: None, flip: false, font: None });
    Ok(b)
}

/// stmaryaj.mf's arrow_not: no width; a slash 4u across, as tall as the arrow's head, over the middle
/// of the 18u arrow that follows (the middle of the font's →).
fn arrow_not(e: Style) -> R<MathBox> {
    let arrow = glyph('→', e)?;
    let (_, y0, _, y1) = ink(&arrow, e);
    let t = rule(e);
    let a = axis(e);
    let (cx, dx, dy) = (arrow.width / 2.0, arrow.width / 9.0, (y1 - y0 - t) / 2.0);
    let mut b = MathBox { width: 0.0, height: a + dy + t / 2.0, depth: (dy + t / 2.0 - a).max(0.0), ..MathBox::default() };
    line(&mut b, (cx - dx, a - dy), (cx + dx, a + dy), t);
    Ok(b)
}

/// stmaryaj.mf's maps_from_char: no width; the stem of ↤ and a stub of its shaft, at the tail of the
/// ← before it (u from its end, the font's ← side bearing), as tall as the arrow's head.
fn maps_from_char(e: Style) -> R<MathBox> {
    let arrow = glyph('←', e)?;
    let (_, y0, x1, y1) = ink(&arrow, e);
    let t = rule(e);
    let a = axis(e);
    let x = -(arrow.width - x1 + t / 2.0);
    let hh = (y1 - y0 - t) / 2.0;
    let mut b = MathBox { width: 0.0, height: a + hh + t / 2.0, depth: (hh + t / 2.0 - a).max(0.0), ..MathBox::default() };
    line(&mut b, (x, a - hh), (x, a + hh), t);
    // the stub: half a unit (an 18u arrow's 36th) toward the shaft
    line(&mut b, (x, a), (x - arrow.width / 36.0, a), t);
    Ok(b)
}

/// stmaryaj.mf's triangle_left_eq_slant: ⊲ with its lower side drawn again `spread'` (0.45 of the
/// x-height) lower, from under the right corner to under the point; the whole centered on the axis.
fn triangle_eq_slant(e: Style) -> R<MathBox> {
    let mut b = glyph('⊲', e)?;
    let (x0, y0, x1, y1) = ink(&b, e);
    let t = rule(e);
    let drop = 0.45 * e.font().x_height() * e.scale();
    let point = (x0 + t / 2.0, (y0 + y1) / 2.0 - drop);
    let corner = (x1 - t / 2.0, y0 + t / 2.0 - drop);
    line(&mut b, corner, point, t);
    let bottom = corner.1 - t / 2.0;
    b.depth = b.depth.max(-bottom);
    b.shift_vertical(axis(e) - (y1 + bottom) / 2.0);
    Ok(b)
}

/// stmaryaj.mf's in_plus: ∈ with a vertical bar at .47 of the way from the ends of its bars to its
/// back, three rule thicknesses short of the top and bottom bars (in cmsy10, 0.223 of the height).
fn in_plus(e: Style) -> R<MathBox> {
    let mut b = glyph('∈', e)?;
    let (x0, y0, x1, y1) = ink(&b, e);
    let t = rule(e);
    let (right, left, top, bottom) = (x1 - t / 2.0, x0 + t / 2.0, y1 - t / 2.0, y0 + t / 2.0);
    let x = right + 0.47 * (left - right);
    let inset = 0.223 * (top - bottom);
    line(&mut b, (x, top - inset), (x, bottom + inset), t);
    Ok(b)
}

/// A marker drawn as a large operator (`LargeOperator`), at the size the style asks for.
#[cold]
#[inline(never)]
pub(super) fn large(c: char, e: Style) -> R<MathBox> {
    let b = match c {
        BIGBOX => big_box(e)?,
        BIGBOXPLUS => {
            let mut b = big_box(e)?;
            let t = stem(&b, false, e).unwrap_or(rule(e));
            let (x0, y0, x1, y1) = ink(&b, e);
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            line(&mut b, (x0 + t / 2.0, cy), (x1 - t / 2.0, cy), t);
            line(&mut b, (cx, y0 + t / 2.0), (cx, y1 - t / 2.0), t);
            b
        }
        BIGPARALLEL => big_parallel(e)?,
        // stmaryaj.mf's big_n_plus is the cap of ⋂ with a plus inside: ⨄ (\biguplus) upside down
        BIGNPLUS => {
            let mut b = large_operator('⨄', e)?;
            let (_, y0, _, y1) = ink(&b, e);
            let (w, h, d) = (b.width, b.height, b.depth);
            b.transform([1.0, 0.0, 0.0, -1.0, 0.0, y0 + y1]);
            (b.width, b.height, b.depth) = (w, h, d);
            b
        }
        BIGCURLYVEE | BIGCURLYWEDGE => curly(c, e)?,
        BIGOMINUS | BIGOSLASH => big_circled(c, e)?,
        _ => return Err(Error::MissingGlyph { character: c }),
    };
    Ok(b)
}

/// stmaryaj.mf's big_box is big_sq_cap with its bottom closed: ⨅ and ⨆ (\bigsqcap and \bigsqcup, the
/// same legs) drawn over each other.
fn big_box(e: Style) -> R<MathBox> {
    let mut b = large_operator('⨅', e)?;
    let cup = large_operator('⨆', e)?;
    let w = b.width.max(cup.width);
    let (left, right) = ((w - b.width) / 2.0, (w - cup.width) / 2.0);
    let mut out = MathBox { width: w, ..MathBox::default() };
    out.place(&b, left, 0.0);
    out.place(&cup, right, 0.0);
    b.items = out.items;
    (b.width, b.height, b.depth) = (w, out.height, out.depth);
    Ok(b)
}

/// stmaryaj.mf's big_parallel: the two legs of big_sq_cap, 6u apart where those are 13u apart (8u and
/// 18u in display), as tall and as thick as ⨅'s, with its side bearings.
fn big_parallel(e: Style) -> R<MathBox> {
    let cap = large_operator('⨅', e)?;
    let t = stem(&cap, false, e).unwrap_or(rule(e));
    let (x0, y0, x1, y1) = ink(&cap, e);
    let share = if e.level == Level::Display { 8.0 / 18.0 } else { 6.0 / 13.0 };
    let apart = (x1 - x0 - t) * share;
    let mut b = MathBox { width: 2.0 * x0 + apart + t, height: cap.height, depth: cap.depth, ..MathBox::default() };
    for x in [x0, x0 + apart] {
        b.items.push(Item::Rule { x, y: y0, width: t, height: y1 - y0, color: None });
    }
    Ok(b)
}

/// \bigcurlyvee and \bigcurlywedge, as stmaryaj.mf draws big_curly_vee: from the top corners of
/// big_vee's box, leaving at 30° below the horizontal, curving down into its bottom point, with the
/// pen of its strokes. Here in the box of the font's ⋁ at the size the style asks for (⋏ is ⋎ upside
/// down in it), each arm a cubic curve with those two directions, as thick as ⋁'s strokes.
fn curly(c: char, e: Style) -> R<MathBox> {
    let vee = large_operator('⋁', e)?;
    let (x0, y0, x1, y1) = ink(&vee, e);
    let cx = (x0 + x1) / 2.0;
    // the stroke: its width across a line halfway up, times the cosine of its slant
    let across = crossing(&vee, false, (y0 + y1) / 2.0, e).map(|xs| xs[1] - xs[0]);
    let slant = ((cx - x0) / (y1 - y0)).atan();
    let t = across.map_or(rule(e), |w| w * slant.cos());
    let (top, bottom) = (y1 - t / 2.0, y0 + t / 2.0);
    let wedge = c == BIGCURLYWEDGE;
    // z1{dir -30}..{down}z3: the arm from the left corner; the right one is its mirror image
    let start = (x0 + t / 2.0, top);
    let end = (cx, bottom);
    let reach = 0.4 * (end.0 - start.0).hypot(end.1 - start.1);
    let (sin, cos) = (-30f32).to_radians().sin_cos();
    let c1 = (start.0 + reach * cos, start.1 + reach * sin);
    let c2 = (end.0, end.1 + reach);
    let at = |u: f32| {
        let v = 1.0 - u;
        let x = v * v * v * start.0 + 3.0 * v * v * u * c1.0 + 3.0 * v * u * u * c2.0 + u * u * u * end.0;
        let y = v * v * v * start.1 + 3.0 * v * v * u * c1.1 + 3.0 * v * u * u * c2.1 + u * u * u * end.1;
        (x, if wedge { y0 + y1 - y } else { y })
    };
    let mut b = MathBox { width: vee.width, height: vee.height, depth: vee.depth, ..MathBox::default() };
    const STEPS: usize = 24;
    for i in 0..STEPS {
        let (p, q) = (at(i as f32 / STEPS as f32), at((i + 1) as f32 / STEPS as f32));
        line(&mut b, p, q, t);
        line(&mut b, (2.0 * cx - p.0, p.1), (2.0 * cx - q.0, q.1), t);
    }
    Ok(b)
}

/// mathabx's \bigominus and \bigoslash (mathtest.pdf): the circle of ⨁ (\bigoplus) at the size the
/// style asks for, as thick as its ring, with a bar across or a slash at 45°, as thick as ⨁'s bar.
fn big_circled(c: char, e: Style) -> R<MathBox> {
    let plus = large_operator('⨁', e)?;
    let (x0, y0, x1, y1) = ink(&plus, e);
    let (cx, cy, outer) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, (x1 - x0) / 2.0);
    // the ring, across a line halfway up its upper half (where it is 1/cos 30° wider than it is thick)
    let ring = crossing(&plus, false, cy + outer / 2.0, e).map(|xs| (xs[1] - xs[0]) * 0.75f32.sqrt()).unwrap_or(rule(e));
    // the bar: the third and fourth crossings of a vertical line halfway along its right half
    let bar = crossing(&plus, true, cx + outer / 2.0, e).filter(|ys| ys.len() >= 6).map(|ys| ys[3] - ys[2]).unwrap_or(rule(e));
    let r = outer - ring / 2.0;
    let mut b = MathBox { width: plus.width, height: plus.height, depth: plus.depth, ..MathBox::default() };
    b.items.push(Item::Ellipse { cx, cy, rx: r, ry: r, thickness: ring, color: None });
    let (from, to) = if c == BIGOSLASH { (225f32, 45f32) } else { (180.0, 0.0) };
    let at = |deg: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        (cx + r * cos, cy + r * sin)
    };
    line(&mut b, at(from), at(to), bar);
    Ok(b)
}

/// The thickness of the first stroke a line across the box's first glyph meets, halfway up (or
/// along, `vertical`): the leg of ⨅.
fn stem(b: &MathBox, vertical: bool, e: Style) -> Option<f32> {
    let (x0, y0, x1, y1) = ink(b, e);
    let at = if vertical { (x0 + x1) / 2.0 } else { (y0 + y1) / 2.0 };
    crossing(b, vertical, at, e).map(|c| c[1] - c[0])
}

/// Where a line across the first glyph of a box meets its outline, in the box's units, in order:
/// a horizontal line at height `at`, or a vertical one at `at` from the left (`vertical`).
fn crossing(b: &MathBox, vertical: bool, at: f32, e: Style) -> Option<Vec<f32>> {
    // (the first item of every box measured here is the glyph)
    let (id, gx, gy, scale) = b.items.iter().find_map(|i| if let Item::Glyph { id, x, y, scale, font: None, flip: false, .. } = *i { Some((id, x, y, scale)) } else { None })?;
    let local = if vertical { (at - gx) / scale } else { (at - gy) / scale };
    let found = crossings(e.font(), id, vertical, local);
    let offset = if vertical { gy } else { gx };
    (found.len() >= 2).then(|| found.into_iter().map(|v| offset + v * scale).collect())
}

/// The crossings of a glyph's outline with a horizontal line at height `at` (font units), or a
/// vertical one at `at` (`vertical`), in order.
fn crossings(f: &Font, id: GlyphId, vertical: bool, at: f32) -> Vec<f32> {
    let mut found = Vec::new();
    for contour in f.contours(id, 4.0) {
        for (a, b) in contour.iter().zip(contour.iter().cycle().skip(1)) {
            let ((pa, qa), (pb, qb)) = if vertical { ((a.0, a.1), (b.0, b.1)) } else { ((a.1, a.0), (b.1, b.0)) };
            if (pa - at) * (pb - at) <= 0.0 && pa != pb {
                found.push(qa + (qb - qa) * (at - pa) / (pb - pa));
            }
        }
    }
    found.sort_by(f32::total_cmp);
    found
}
