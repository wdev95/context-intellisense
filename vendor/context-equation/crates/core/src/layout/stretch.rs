//! Glyphs that grow: tall parentheses, radicals, braces and long arrows.
//! First try the font's ready-made variants; if none is enough, assemble from parts.

use crate::font::{Font, Part};
use crate::Error;

use super::{glyph_id, k, Item, MathBox, Style};

type R<T> = Result<T, Error>;

/// Vertical glyph with a total height of at least `target` (already at the style's scale).
pub fn vertical(c: char, target: f32, e: Style) -> R<MathBox> {
    let f = e.font();
    let s = e.scale();
    if f.glyph(c).is_none() {
        // #176: stmaryrd's doubled ceilings and floors, two of the single ones grown
        if let Some(single) = crate::composite::doubled(c) {
            return super::composite::doubled_delimiter(single, target, e);
        }
        if let Some(parts) = moustache(c, f) {
            // as cmex draws it: the two halves at least, joined by extenders as tall as needed
            let least: f32 = parts.iter().filter(|p| !p.extender).map(|p| p.advance).sum::<f32>() - f.min_overlap();
            return Ok(assemble(&parts, (target / s).max(least), s, true, f));
        }
    }
    let id = f.glyph(c).ok_or(Error::MissingGlyph { character: c })?;
    let base = glyph_id(id, s, f);
    if base.total() >= target {
        return Ok(base);
    }
    let variants = f.variants(id, true);
    for (v, measure) in &variants {
        if measure * s >= target {
            return Ok(glyph_id(*v, s, f));
        }
    }
    if let Some(parts) = f.assembly(id, true) {
        return Ok(assemble(&parts, target / s, s, true, f));
    }
    // \lgroup and \rgroup, which the fonts have at one size only: grown as cmex grows them
    if variants.is_empty() && matches!(c, '⟮' | '⟯') {
        if let Some(parts) = moustache(c, f) {
            return Ok(assemble(&parts, target / s, s, true, f));
        }
    }
    Ok(variants.last().map_or(base, |(v, _)| glyph_id(*v, s, f)))
}

/// `\lmoustache` (⎰) and `\rmoustache` (⎱) in a font without them (Latin Modern Math), and a tall
/// `\lgroup` (⟮) and `\rgroup` (⟯): built as TeX's cmex builds them, from the halves of the braces and
/// their extender, bottom to top. The connectors are the ones the braces' own assemblies give these
/// parts.
fn moustache(c: char, f: &Font) -> Option<Vec<Part>> {
    let (bottom, top) = match c {
        '⎰' => ('⎭', '⎧'),
        '⎱' => ('⎩', '⎫'),
        '⟮' => ('⎩', '⎧'),
        '⟯' => ('⎭', '⎫'),
        _ => return None,
    };
    let known: Vec<Part> = ['{', '}'].iter().filter_map(|b| f.glyph(*b).and_then(|id| f.assembly(id, true))).flatten().collect();
    let part = |c: char| {
        let id = f.glyph(c)?;
        known.iter().find(|p| p.id == id).copied()
    };
    // (the braces' extender is a glyph of their own, its stem where the halves have theirs)
    let extender = known.iter().find(|p| p.extender).copied()?;
    Some(vec![part(bottom)?, extender, part(top)?])
}

/// Horizontal glyph with a width of at least `target`.
pub fn horizontal(c: char, target: f32, e: Style) -> R<MathBox> {
    let f = e.font();
    let s = e.scale();
    let id = f.glyph(c).ok_or(Error::MissingGlyph { character: c })?;
    let width = |g| {
        let (x0, _, x1, _) = f.limits(g);
        (x1 - x0) * s
    };
    if width(id) >= target {
        return Ok(glyph_id(id, s, f));
    }
    let variants = f.variants(id, false);
    for (v, _) in &variants {
        if width(*v) >= target {
            return Ok(glyph_id(*v, s, f));
        }
    }
    if let Some(parts) = f.assembly(id, false) {
        return Ok(assemble(&parts, target / s, s, false, f));
    }
    if let Some(built) = composite_arrow(c, id, target, e)? {
        return Ok(built);
    }
    let widest = variants.last().map_or(id, |(v, _)| *v);
    if let Some(built) = tiled_group(c, widest, target, e) {
        return Ok(built);
    }
    Ok(glyph_id(widest, s, f))
}

/// `\overgroup` (⏠) and `\undergroup` (⏡) wider than the font's widest size (STIX Two Math has
/// sizes but no assembly for them; Latin Modern Math has one). The glyph is a flat stroke between
/// two slanted corners, and is drawn again from its own outline at the width asked: the stroke as
/// a rule, each corner as a line from the foot of the glyph to where the stroke begins, as thick
/// as the glyph's.
fn tiled_group(c: char, id: ttf_parser::GlyphId, target: f32, e: Style) -> Option<MathBox> {
    if !matches!(c, '⏠' | '⏡') {
        return None;
    }
    let f = e.font();
    let s = e.scale();
    let (x0, y0, x1, y1) = f.limits(id);
    if x1 <= x0 || (x1 - x0) * s >= target {
        return None;
    }
    // the stroke, where the outline crosses the middle of the glyph
    let (bottom, top) = crossings(f, id, (x0 + x1) / 2.0)?;
    let thickness = top - bottom;
    let points: Vec<(f32, f32)> = f.contours(id, 4.0).into_iter().flatten().collect();
    // the foot: the end of the corner, at the far left; the corner goes up (⏠) or down (⏡) to the stroke
    let foot: Vec<&(f32, f32)> = points.iter().filter(|p| p.0 < x0 + thickness / 2.0).collect();
    let foot = (foot.iter().map(|p| p.0).sum::<f32>() / foot.len() as f32, foot.iter().map(|p| p.1).sum::<f32>() / foot.len() as f32);
    let up = foot.1 < bottom;
    // the stroke's outer edge, and where it starts on the left
    let outer = if up { top } else { bottom };
    let start = points.iter().filter(|p| (p.1 - outer).abs() < 1.0).map(|p| p.0).fold(f32::INFINITY, f32::min);
    let inner = points.iter().filter(|p| (p.1 - (bottom + top - outer)).abs() < 1.0).map(|p| p.0).fold(f32::INFINITY, f32::min);
    if !start.is_finite() || !inner.is_finite() {
        return None;
    }
    let joint = ((start + inner) / 2.0, (bottom + top) / 2.0);
    let (dx, dy) = (joint.0 - foot.0, joint.1 - foot.1);
    let length = dx.hypot(dy);
    // the corner's own thickness, across it; the line's round cap stops at the foot
    let across = thickness * dx.abs() / length;
    let foot = (foot.0 + dx / length * across / 2.0, foot.1 + dy / length * across / 2.0);
    let left = |x: f32| (x - x0) * s;
    let right = |x: f32| target - (x - x0) * s;
    let mut math_box = MathBox::empty();
    for side in [&left as &dyn Fn(f32) -> f32, &right] {
        math_box.items.push(Item::Line { x1: side(foot.0), y1: foot.1 * s, x2: side(joint.0), y2: joint.1 * s, thickness: across * s, color: None });
    }
    math_box.items.push(Item::Rule { x: left(start), y: bottom * s, width: right(start) - left(start), height: thickness * s, color: None });
    math_box.width = target;
    math_box.height = (y1 * s).max(0.0);
    math_box.depth = (-y0 * s).max(0.0);
    Some(math_box)
}

/// Lowest and highest point where the outline of a glyph crosses the vertical line at `x` (font units).
fn crossings(f: &Font, id: ttf_parser::GlyphId, x: f32) -> Option<(f32, f32)> {
    let mut found = (f32::INFINITY, f32::NEG_INFINITY);
    for contour in f.contours(id, 4.0) {
        let closed = contour.iter().zip(contour.iter().cycle().skip(1));
        for (&(xa, ya), &(xb, yb)) in closed {
            if (xa - x) * (xb - x) <= 0.0 && xa != xb {
                let y = ya + (yb - ya) * (x - xa) / (xb - xa);
                found = (found.0.min(y), found.1.max(y));
            }
        }
    }
    (found.0 < found.1).then_some(found)
}

/// The narrowest a horizontal brace or bracket (⏞ ⏟ ⎴ ⎵) is drawn: the font's smallest variant
/// wider than its base glyph, the first one built to stretch. The base glyph is as wide as a
/// letter, so over one or two characters its tips and middle point crowd together; TeX gives the
/// brace a minimum width too (its pieces at their natural size). Zero when the font has no variants.
pub fn minimum_width(c: char, e: Style) -> f32 {
    let f = e.font();
    let Some(id) = f.glyph(c) else { return 0.0 };
    let width = |g| {
        let (x0, _, x1, _) = f.limits(g);
        x1 - x0
    };
    let base = width(id);
    let variants = f.variants(id, false);
    let wider = variants.iter().map(|(v, _)| width(*v)).find(|w| *w > base);
    wider.unwrap_or(0.0) * e.scale()
}

/// Arrows the font draws at one size only (STIX Two Math has no assembly for ⇌, ⇄ or ⟶): built
/// from arrows it can stretch, as `\xrightleftharpoons` and mhchem's `<=>` need. A pair is stacked
/// where the glyph has its two arrows (the upper one's top on the glyph's top, the lower one's
/// bottom on its bottom); a long arrow is its short one stretched. The second number is the length
/// of the upper and lower arrow, as a share of the whole (⥂ has a short lower arrow).
fn composite_arrow(c: char, id: ttf_parser::GlyphId, target: f32, e: Style) -> R<Option<MathBox>> {
    let (upper, lower, shares) = match c {
        '⇌' => ('⇀', Some('↽'), (1.0, 1.0)),
        '⇋' => ('↼', Some('⇁'), (1.0, 1.0)),
        '⇄' => ('→', Some('←'), (1.0, 1.0)),
        '⇆' => ('←', Some('→'), (1.0, 1.0)),
        '⇉' => ('→', Some('→'), (1.0, 1.0)),
        '⇇' => ('←', Some('←'), (1.0, 1.0)),
        '⥂' => ('→', Some('←'), (1.0, 0.6)),
        '⥄' => ('→', Some('←'), (0.6, 1.0)),
        '⟶' => ('→', None, (1.0, 1.0)),
        '⟵' => ('←', None, (1.0, 1.0)),
        '⟷' => ('↔', None, (1.0, 1.0)),
        '⟹' => ('⇒', None, (1.0, 1.0)),
        '⟸' => ('⇐', None, (1.0, 1.0)),
        '⟺' => ('⇔', None, (1.0, 1.0)),
        '⟼' => ('↦', None, (1.0, 1.0)),
        _ => return Ok(None),
    };
    let f = e.font();
    if [Some(upper), lower].into_iter().flatten().any(|p| f.glyph(p).and_then(|g| f.assembly(g, false)).is_none()) {
        return Ok(None);
    }
    let Some(lower) = lower else {
        return horizontal(upper, target, e).map(Some);
    };
    let s = e.scale();
    let (_, bottom, _, top) = f.limits(id);
    let a = horizontal(upper, target * shares.0, e)?;
    let b = horizontal(lower, target * shares.1, e)?;
    let width = a.width.max(b.width).max(target);
    let mut math_box = MathBox::space(width);
    let (_, a_top) = ink(&a, e);
    let (b_bottom, _) = ink(&b, e);
    let (mut up, mut down) = (top * s - a_top, bottom * s - b_bottom);
    // the stretchable harpoons have taller barbs than the ones inside ⇌: kept apart as the glyph
    // keeps them (and TeX's 2pt), shafts three rule thicknesses apart
    let apart = 3.0 * k(e, |c| c.fraction_rule_thickness());
    if up - down < apart {
        let extra = (apart - (up - down)) / 2.0;
        (up, down) = (up + extra, down - extra);
    }
    math_box.place(&a, (width - a.width) / 2.0, up);
    math_box.place(&b, (width - b.width) / 2.0, down);
    Ok(Some(math_box))
}

/// Lowest and highest ink of the glyphs in a box, from their outlines. A box's height and depth
/// never go below zero, so they hide where a glyph that floats above (⏞) or below (⏟) the baseline
/// really is.
pub fn ink(math_box: &MathBox, e: Style) -> (f32, f32) {
    let f = e.font();
    let mut bounds = (f32::INFINITY, f32::NEG_INFINITY);
    for item in &math_box.items {
        if let Item::Glyph { id, y, scale, font, .. } = item {
            let (_, y0, _, y1) = crate::font::glyph_font(*font, f).limits(*id);
            bounds = (bounds.0.min(y + y0 * scale), bounds.1.max(y + y1 * scale));
        }
    }
    if bounds.0 > bounds.1 {
        (-math_box.depth, math_box.height)
    } else {
        bounds
    }
}

/// Delimiter centered on the math axis, covering `target` of total height.
pub fn delimiter(c: char, target: f32, e: Style) -> R<MathBox> {
    let mut math_box = vertical(c, target, e)?;
    let axis = k(e, |c| c.axis_height());
    let offset = axis - (math_box.height - math_box.depth) / 2.0;
    math_box.shift_vertical(offset);
    math_box.single_glyph = false;
    Ok(math_box)
}

/// Assembles the parts (bottom to top or left to right), repeating the extenders
/// as few times as needed to reach the size, with overlap between connectors.
fn assemble(parts: &[Part], target: f32, s: f32, vertical: bool, f: &Font) -> MathBox {
    let overlap = f.min_overlap();
    let sequence = |n: usize| -> Vec<Part> { parts.iter().flat_map(|p| std::iter::repeat_n(*p, if p.extender { n } else { 1 })).collect() };
    let size = |seq: &[Part], ov: f32| -> f32 { seq.iter().map(|p| p.advance).sum::<f32>() - ov * seq.len().saturating_sub(1) as f32 };
    let mut n = 0;
    let mut seq = sequence(n);
    while size(&seq, overlap) < target && n < 200 {
        n += 1;
        seq = sequence(n);
    }
    // spread the leftover by increasing the overlap, never beyond the shortest connector
    let joined = seq.len().saturating_sub(1).max(1) as f32;
    let slack = seq.windows(2).map(|w| w[0].end.min(w[1].start)).fold(f32::INFINITY, f32::min);
    let excess = (size(&seq, overlap) - target).max(0.0);
    let ov = (overlap + excess / joined).min(slack.max(overlap));

    let mut math_box = MathBox::empty();
    let mut cursor = 0.0;
    for p in &seq {
        let (x0, y0, _, _) = f.limits(p.id);
        let (x, y) = if vertical { (0.0, (cursor - y0) * s) } else { ((cursor - x0) * s, 0.0) };
        math_box.items.push(Item::Glyph { id: p.id, x, y, scale: s, color: None, flip: false, font: None });
        let g = glyph_id(p.id, s, f);
        if vertical {
            math_box.width = math_box.width.max(g.width);
        } else {
            math_box.height = math_box.height.max(g.height);
            math_box.depth = math_box.depth.max(g.depth);
        }
        cursor += p.advance - ov;
    }
    let total = (cursor + ov) * s;
    if vertical {
        math_box.height = total;
        math_box.depth = 0.0;
    } else {
        math_box.width = total;
    }
    math_box
}
