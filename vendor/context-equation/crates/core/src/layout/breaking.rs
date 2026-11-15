//! L-BRK-1 and L-BRK-2: automatic line breaking, a pass over the formula's top-level row after it
//! is composed (spec/rules.md).
//!
//! A line may break only between the atoms of the top-level row: after a relation first, and only
//! if relations alone are not enough, after `+`, `−`, `±` and `∓` too. Fractions, roots,
//! `\left…\right`, scripts, groups and tables are atoms of that row, so nothing inside them ever
//! breaks. The operator is not repeated on the next line (as TeX and MathJax). The recursive
//! layout is untouched: this pass composes the row's children one by one, as `compose_row` does.

use super::{classes, compose, em, mu_space, Context, MathBox, Style, R};
use crate::output::svg::n;
use crate::rules;
use crate::tree::{Class, Node, TableKind};

/// The formula as drawn: one box, and its lines when it was broken.
#[derive(Debug, Clone, Default)]
pub struct Formula {
    /// Everything, in final coordinates: the baseline at y = 0 is the first line's, the height is
    /// what rises above it and the depth covers every line below.
    pub math_box: MathBox,
    /// The lines, each with its items in final coordinates; empty when the formula is one line.
    pub lines: Vec<MathBox>,
}

/// One child of the top-level row, composed, with where the row set it.
struct Piece {
    x: f32,
    b: MathBox,
    class: Option<Class>,
    /// 1: a relation; 2: `+`, `−`, `±`, `∓` between operands; 0: no break after it.
    tier: u8,
}

/// The fixed indent when the continuation cannot align under the right-hand side (2 em).
const FIXED_INDENT: f32 = 2.0;
/// Rows set as LaTeX sets the lines of a paragraph or of an alignment: a strut of 0.84 em above
/// the baseline and 0.36 em below (`\baselineskip` 12pt at 10pt), plus `\jot` (0.3 em) between
/// the lines of a display formula, as amsmath's `multline` and `split` do.
const STRUT_HEIGHT: f32 = 0.84;
const STRUT_DEPTH: f32 = 0.36;
const JOT: f32 = 0.3;

/// The formula `node`, composed by `compose_list` into `math_box`, broken into lines no wider than
/// `max_width` em when it is wider than that. A pass of its own, after the recursive layout: it
/// adds nothing to the stack of deep formulas.
#[inline(never)]
pub fn break_lines(node: &Node, math_box: MathBox, e: Style, ctx: &mut Context, max_width: Option<f32>) -> R<Formula> {
    let Some(max_em) = max_width.filter(|m| m.is_finite() && *m > 0.0) else {
        return Ok(Formula { math_box, lines: Vec::new() });
    };
    let max = em(e, max_em);
    if math_box.width <= max {
        return Ok(Formula { math_box, lines: Vec::new() });
    }
    let children = match node {
        Node::Row { children } if children.len() > 1 && !children.iter().any(is_alignment) => children,
        _ => {
            let what = if is_alignment(node) || node.children().iter().any(is_alignment) || matches!(node.single(), Node::Table { .. }) { "the table" } else { "the formula" };
            overflow(ctx, max_em, what, math_box.width - max, e);
            return Ok(Formula { math_box, lines: Vec::new() });
        }
    };
    let (pieces, total) = pieces(children, e, ctx)?;
    let indent = indent(&pieces, max, e);
    let relations: Vec<usize> = (0..pieces.len()).filter(|&i| pieces[i].tier == 1).collect();
    let all: Vec<usize> = (0..pieces.len()).filter(|&i| pieces[i].tier > 0).collect();
    let mut plan = plan_with(&pieces, total, &relations, max, max - indent);
    if !fits(&pieces, total, &plan, max, max - indent) {
        plan = plan_with(&pieces, total, &all, max, max - indent);
    }
    if plan.len() < 2 {
        overflow(ctx, max_em, "the formula", math_box.width - max, e);
        return Ok(Formula { math_box, lines: Vec::new() });
    }
    let broken_after: Vec<String> = plan[..plan.len() - 1].iter().map(|&(_, last)| symbol(children, last)).collect();
    ctx.registry.note(rules::L_BRK_1, format!("broken into {} lines within {}em, after {}", plan.len(), n(max_em), broken_after.join(", ")));
    for (k, &(first, last)) in plan.iter().enumerate() {
        let width = end(&pieces, total, last) - pieces[first].x + if k > 0 { indent } else { 0.0 };
        if width > max + 0.5 {
            overflow(ctx, max_em, &format!("line {}", k + 1), width - max, e);
        }
    }
    Ok(stack(&pieces, &plan, indent, e))
}

/// Greedy, as far along each line as fits: the last candidate whose line is within the width, or,
/// when none is, the first one after (that line overflows).
fn plan_with(pieces: &[Piece], total: f32, candidates: &[usize], first: f32, rest: f32) -> Vec<(usize, usize)> {
    let last = pieces.len() - 1;
    let mut lines = Vec::new();
    let mut start = 0;
    loop {
        let width = if lines.is_empty() { first } else { rest };
        let x0 = pieces[start].x;
        if total - x0 <= width {
            lines.push((start, last));
            return lines;
        }
        let mut after = candidates.iter().copied().filter(|&c| c >= start && c < last);
        let fitting = after.clone().take_while(|&c| end(pieces, total, c) - x0 <= width).last();
        let Some(c) = fitting.or_else(|| after.next()) else {
            lines.push((start, last));
            return lines;
        };
        lines.push((start, c));
        start = next_start(pieces, c);
    }
}

fn fits(pieces: &[Piece], total: f32, plan: &[(usize, usize)], first: f32, rest: f32) -> bool {
    plan.iter().enumerate().all(|(k, &(a, b))| end(pieces, total, b) - pieces[a].x <= if k == 0 { first } else { rest })
}

/// Where a line that ends with piece `i` ends: the last line takes the row's closing kern too.
fn end(pieces: &[Piece], total: f32, i: usize) -> f32 {
    if i + 1 == pieces.len() {
        total
    } else {
        pieces[i].x + pieces[i].b.width
    }
}

/// The first piece of the line after a break at `i`: spaces at the start of a line are dropped,
/// as TeX discards glue after a break.
fn next_start(pieces: &[Piece], i: usize) -> usize {
    (i + 1..pieces.len()).find(|&j| pieces[j].class.is_some()).unwrap_or(i + 1)
}

/// Continuation lines start under the right-hand side of the first relation; with no relation, or
/// one more than half the width in, 2 em in.
fn indent(pieces: &[Piece], max: f32, e: Style) -> f32 {
    let rhs = pieces.iter().position(|p| p.class == Some(Class::Rel)).map(|r| next_start(pieces, r)).filter(|&j| j < pieces.len());
    match rhs.map(|j| pieces[j].x) {
        Some(x) if x <= max / 2.0 => x,
        _ => em(e, FIXED_INDENT),
    }
}

/// The lines one under the other; continuation lines indented.
fn stack(pieces: &[Piece], plan: &[(usize, usize)], indent: f32, e: Style) -> Formula {
    let jot = if e.level == super::Level::Display { em(e, JOT) } else { 0.0 };
    let mut formula = Formula::default();
    let mut y = 0.0;
    let mut previous_depth = 0.0f32;
    for (k, &(first, last)) in plan.iter().enumerate() {
        let shift = if k > 0 { indent } else { 0.0 } - pieces[first].x;
        let mut line = MathBox::empty();
        for p in &pieces[first..=last] {
            line.place(&p.b, p.x + shift, 0.0);
            line.width = line.width.max(p.x + shift + p.b.width);
        }
        if k > 0 {
            y -= previous_depth.max(em(e, STRUT_DEPTH)) + line.height.max(em(e, STRUT_HEIGHT)) + jot;
        }
        previous_depth = line.depth;
        let mut placed = MathBox::empty();
        placed.place(&line, 0.0, y);
        formula.math_box.place(&line, 0.0, y);
        formula.math_box.width = formula.math_box.width.max(line.width);
        formula.lines.push(placed);
    }
    if let Some(first) = formula.lines.first() {
        // the baseline is the first line's: only it rises above
        formula.math_box.height = formula.math_box.height.max(first.height);
    }
    formula
}

/// The children of the top-level row, composed one by one and set where `compose_row` sets them
/// (the same spacing, the same italic corrections, in the same order of additions). Also returns
/// the row's width, with the italic correction that closes it.
fn pieces(children: &[Node], e: Style, ctx: &mut Context) -> R<(Vec<Piece>, f32)> {
    let cl = classes(children, ctx.locale, Some(ctx.registry), false);
    let mu = em(e, 1.0 / 18.0);
    let mut pieces = Vec::with_capacity(children.len());
    let mut x = 0.0;
    let mut previous: Option<Class> = None;
    let mut pending_italic = 0.0;
    for (node, class) in children.iter().zip(&cl) {
        let b = compose(node, e, ctx)?;
        if let (Some(a), Some(c)) = (previous, class) {
            x += mu_space(a, *c, e.is_script()) * mu;
        }
        if pending_italic > 0.0 {
            x += pending_italic;
        }
        pending_italic = if matches!(
            node,
            Node::Identifier {
                variant: crate::tree::Variant::Italic | crate::tree::Variant::BoldItalic | crate::tree::Variant::SansSerifItalic | crate::tree::Variant::SansSerifBoldItalic,
                ..
            }
        ) {
            b.italic
        } else {
            0.0
        };
        let width = b.width;
        pieces.push(Piece { x, b, class: *class, tier: 0 });
        x += width;
        if class.is_some() {
            previous = *class;
        }
    }
    if pending_italic > 0.0 {
        x += pending_italic;
    }
    for (i, piece) in pieces.iter_mut().enumerate() {
        piece.tier = tier(children, &cl, i);
    }
    Ok((pieces, x))
}

/// Whether a line may break after child `i`, and how gladly: 1 after a relation, 2 after a binary
/// `+`, `−`, `±`, `∓`; 0 never (at the end, or before another relation, as TeX's `\relpenalty`).
fn tier(children: &[Node], cl: &[Option<Class>], i: usize) -> u8 {
    let Some(next) = (i + 1..cl.len()).find_map(|j| cl[j]) else {
        return 0;
    };
    match (cl[i], &children[i]) {
        (Some(Class::Rel), _) if next != Class::Rel => 1,
        (Some(Class::Bin), Node::Operator { symbol: '+' | '−' | '-' | '±' | '∓', .. }) => 2,
        _ => 0,
    }
}

/// An alignment the author already broke into lines (`align`, `gather`, `aligned`…).
fn is_alignment(node: &Node) -> bool {
    match node {
        Node::Table { shape, .. } => matches!(shape, TableKind::Aligned | TableKind::Centered),
        Node::Row { children } if children.len() == 1 => is_alignment(&children[0]),
        _ => false,
    }
}

/// How a break point reads in a note: the operator, or "a relation" for an arrow (or another
/// relation that is not an operator). A break point is a relation or one of `tier`'s operators, and
/// `classes` changes the class of operators only, so any other node there is a relation.
fn symbol(children: &[Node], i: usize) -> String {
    match &children[i] {
        Node::Operator { symbol, .. } => format!("'{symbol}'"),
        _ => "a relation".to_string(),
    }
}

/// L-BRK-2: what cannot be broken is drawn as it is, wider than asked.
fn overflow(ctx: &mut Context, max_em: f32, what: &str, excess: f32, e: Style) {
    let by = excess / em(e, 1.0);
    ctx.registry.note(rules::L_BRK_2, format!("{what} cannot be broken within {}em: drawn {}em wider", n(max_em), n((by * 100.0).ceil() / 100.0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Registry;

    /// The pieces, put back on one line, are the row `compose_list` draws: the same items, at the
    /// same places, and the same size.
    #[test]
    fn the_pieces_are_the_row() {
        for tex in [r"f(x) = a_0 + a_1 x + a_2 x^2", r"\int f\,dx = -F + C", r"]a, b[ = \{ x \mid a < x < b \}", r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}", r"a\quad b f"] {
            let mut registry = Registry::default();
            let tree = crate::input::latex::read(tex, crate::Locale::PtBr, &mut registry).unwrap();
            let mut ctx = Context { locale: crate::Locale::PtBr, registry: &mut registry, strict: false, kerning: Default::default(), fallbacks: &[] };
            let e = Style::display();
            let whole = super::super::compose_list(&tree, e, &mut ctx).unwrap();
            let (pieces, total) = pieces(tree.children(), e, &mut ctx).unwrap();
            let mut again = MathBox::empty();
            for p in &pieces {
                again.place(&p.b, p.x, 0.0);
            }
            assert_eq!(again.items, whole.items, "{tex}");
            assert_eq!((total, again.height, again.depth), (whole.width, whole.height, whole.depth), "{tex}");
        }
    }
}
