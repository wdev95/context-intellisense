//! Matrices, cases, alignments and `array`, with TeX's measures:
//! rows with a strut of 0.84 em above and 0.36 em below (`\arraystretch` 1, `\baselineskip` 12pt),
//! columns separated by 2 × `\arraycolsep` (1 em), `\jot` of 0.3 em between alignment rows.
//! `\arraystretch` makes the strut of `array` and of the matrices taller, as in LaTeX; the dashed
//! rules (`:`, `\hdashline`) follow arydshln: dashes and gaps of 4pt.

use crate::tree::{Node, TableKind, TableRule};

/// The rules of a table and the stretch of its rows.
pub struct Lines<'a> {
    /// Boundaries with a `|` rule (0 = before the first column).
    pub verticals: &'a [usize],
    /// Boundaries with a dashed `:` rule.
    pub dashed: &'a [usize],
    pub horizontals: &'a [TableRule],
    /// `\arraystretch`.
    pub stretch: f32,
    /// `\arraycolsep`, in thousandths of an em: half the space between columns of an array or a matrix.
    pub colsep: u16,
}

/// A composed cell: starting column, how many columns it spans, its own alignment.
struct Cell {
    column: usize,
    span: usize,
    alignment: Option<char>,
    math_box: MathBox,
    node: Node,
}
use crate::Error;

use super::{compose_cell, em, k, stretch, Context, Item, Level, MathBox, Style};

type R<T> = Result<T, Error>;

#[allow(clippy::too_many_arguments)]
pub fn compose(kind: TableKind, rows: &[Vec<Node>], columns: &[char], lines: &Lines, between: &[(usize, Node)], intertexts: &[usize], e: Style, ctx: &mut Context) -> R<MathBox> {
    use TableKind::*;
    let cell_style = match kind {
        SmallMatrix => Style { level: Level::Script, cramped: e.cramped, font: e.font },
        Substack => e,
        Aligned | Centered => Style { level: Level::Display, cramped: false, font: e.font },
        _ => e.not_display(),
    };
    let span = |node: &Node| {
        if let Node::Multicolumn { columns, .. } = node {
            *columns
        } else {
            1
        }
    };
    let ncol = rows.iter().enumerate().filter(|(i, _)| !intertexts.contains(i)).map(|(_, l)| l.iter().map(span).sum::<usize>()).max().unwrap_or(0).max(columns.len()).max(1);
    let alignment = |j: usize| -> char {
        match kind {
            Array => columns.get(j).copied().unwrap_or('c'),
            Cases | RCases => 'l',
            Aligned => {
                if j.is_multiple_of(2) {
                    'r'
                } else {
                    'l'
                }
            }
            Substack => columns.first().copied().unwrap_or('c'),
            _ => columns.first().copied().unwrap_or('c'),
        }
    };

    // compose the cells, tracking each one's column (\multicolumn spans several)
    let mut cells: Vec<Vec<Cell>> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let mut c = Vec::new();
        let mut j = 0;
        for node in row {
            let initial_ord = kind == Aligned && j % 2 == 1 && !intertexts.contains(&i);
            let (n, alignment, content) = match node {
                Node::Multicolumn { columns, alignment, base } => (*columns, Some(*alignment), base.as_ref()),
                other => (1, None, other),
            };
            let math_box = if intertexts.contains(&i) { super::compose(content, e.not_display(), ctx)? } else { compose_cell(content, cell_style, ctx, initial_ord)? };
            c.push(Cell { column: j, span: n, alignment, math_box, node: content.clone() });
            j += n;
        }
        cells.push(c);
    }

    // row and column measures
    let scale = cell_style.scale();
    let horizontals = lines.horizontals;
    let (strut_height, strut_depth) = match kind {
        SmallMatrix => (em(cell_style, 0.85), em(cell_style, 0.35)),
        Substack => (em(cell_style, 0.6), em(cell_style, 0.2)),
        // alignments space their rows with \jot, and a diagram is not an array: \arraystretch is not theirs
        Aligned | Centered | Diagram => (0.84 * 1000.0 * scale, 0.36 * 1000.0 * scale),
        _ => (0.84 * 1000.0 * scale * lines.stretch, 0.36 * 1000.0 * scale * lines.stretch),
    };
    let jot = if matches!(kind, Aligned | Centered) { em(e, 0.3) } else { 0.0 };
    let mut heights = Vec::new();
    let mut depths = Vec::new();
    // a row of `\shortvdotswithin` (its ⋮ and empty cells) has no strut
    let short = |row: &[Cell]| {
        let mut filled = row.iter().filter(|c| !matches!(&c.node, Node::Row { children } if children.is_empty())).peekable();
        filled.peek().is_some() && filled.all(|c| matches!(c.node, Node::VdotsWithin { short: true, .. }))
    };
    for row in &cells {
        let (strut_height, strut_depth) = if short(row) { (0.0, 0.0) } else { (strut_height, strut_depth) };
        heights.push(row.iter().map(|c| c.math_box.height).fold(strut_height, f32::max));
        depths.push(row.iter().map(|c| c.math_box.depth).fold(strut_depth, f32::max));
    }
    let mut widths = vec![0.0f32; ncol];
    for (i, row) in cells.iter().enumerate() {
        if intertexts.contains(&i) {
            continue;
        }
        for c in row.iter().filter(|c| c.span == 1) {
            widths[c.column] = widths[c.column].max(c.math_box.width);
        }
    }
    // commutative diagram arrows are at least 2.5 em long (\minCDarrowwidth)
    if kind == Diagram {
        for (j, l) in widths.iter_mut().enumerate() {
            if j % 2 == 1 {
                *l = l.max(em(e, 2.5));
            }
        }
    }

    // space between columns, per boundary (0 = before the first, ncol = after the last)
    let half_sep = match kind {
        Array => em(e, f32::from(lines.colsep) / 1000.0),
        Cases | RCases => em(e, 0.5),
        Aligned => 0.0,
        SmallMatrix => em(e, 0.2),
        Substack => 0.0,
        Diagram => em(e, 0.25),
        // the matrices of amsmath are arrays: \arraycolsep is theirs too
        Matrix | PMatrix | BMatrix | BraceMatrix | VMatrix | VVMatrix => em(e, f32::from(lines.colsep) / 1000.0),
        _ => em(e, 0.5),
    };
    let thickness = em(e, 0.04);
    // \bordermatrix: parentheses around the rows and columns from the second on
    let border = if kind == BorderMatrix && cells.len() > 1 && ncol > 1 {
        let core: f32 = (1..cells.len()).map(|i| heights[i] + depths[i]).sum();
        Some((stretch::delimiter('(', core, e)?, stretch::delimiter(')', core, e)?))
    } else {
        None
    };
    let mut boundary = vec![0.0f32; ncol + 1];
    let mut boundary_content: Vec<Option<MathBox>> = vec![None; ncol + 1];
    for (j, f) in boundary.iter_mut().enumerate() {
        let outer = j == 0 || j == ncol;
        *f = match kind {
            Array => 2.0 * half_sep - if outer { half_sep } else { 0.0 },
            Aligned => {
                if outer {
                    0.0
                } else if j % 2 == 0 {
                    em(e, 2.0)
                } else {
                    0.0
                }
            }
            _ => {
                if outer {
                    0.0
                } else {
                    2.0 * half_sep
                }
            }
        };
    }
    for (j, node) in between {
        if *j <= ncol {
            let c = super::compose(node, cell_style, ctx)?;
            boundary[*j] = c.width;
            boundary_content[*j] = Some(c);
        }
    }
    for j in lines.verticals.iter().chain(lines.dashed) {
        if *j <= ncol {
            boundary[*j] += thickness;
        }
    }
    if let Some((open, close)) = &border {
        boundary[1] = open.width + em(e, 0.4);
        boundary[ncol] += close.width;
    }

    // x position of each column
    let mut x_col = vec![0.0f32; ncol];
    let mut x = 0.0;
    let mut x_boundary = vec![0.0f32; ncol + 1];
    for j in 0..ncol {
        x_boundary[j] = x;
        x += boundary[j];
        x_col[j] = x;
        x += widths[j];
    }
    x_boundary[ncol] = x;
    // multi-column cells that do not fit widen the last column they span
    let mut total_width = x + boundary[ncol];
    for row in &cells {
        for c in row.iter().filter(|c| c.span > 1) {
            let end = (c.column + c.span).min(ncol) - 1;
            let space = x_col[end] + widths[end] - x_col[c.column.min(ncol - 1)];
            if c.math_box.width > space {
                let extra = c.math_box.width - space;
                widths[end] += extra;
                for x in x_col.iter_mut().skip(end + 1) {
                    *x += extra;
                }
                for f in x_boundary.iter_mut().skip(end + 1) {
                    *f += extra;
                }
                total_width += extra;
            }
        }
    }
    for (i, row) in cells.iter().enumerate() {
        if intertexts.contains(&i) {
            if let Some(c) = row.first() {
                total_width = total_width.max(c.math_box.width);
            }
        }
    }

    // y position of each row (top to bottom), counting the thickness of \hline rules
    let draws_rule = |i: usize| horizontals.iter().any(|t| t.row == i);
    let mut y = 0.0;
    let mut bases = Vec::new();
    let mut y_rule = vec![0.0f32; cells.len() + 1];
    for i in 0..cells.len() {
        y_rule[i] = y;
        if draws_rule(i) {
            y -= thickness;
        }
        if i > 0 {
            y -= jot;
        }
        y -= heights[i];
        bases.push(y);
        y -= depths[i];
    }
    y_rule[cells.len()] = y;
    if draws_rule(cells.len()) {
        y -= thickness;
    }
    let total_height = -y;

    // center on the math axis
    let axis = k(e, |c| c.axis_height());
    let top = axis + total_height / 2.0;

    let mut body = MathBox::space(total_width);
    for (i, row) in cells.iter().enumerate() {
        for c in row {
            if intertexts.contains(&i) {
                body.place(&c.math_box, 0.0, top + bases[i]);
                continue;
            }
            let j = c.column.min(ncol - 1);
            let end = (c.column + c.span).min(ncol) - 1;
            let space = x_col[end] + widths[end] - x_col[j];
            // horizontal diagram arrows stretch to the column width
            let math_box = match (&c.node, kind) {
                (Node::Arrow { symbol, above, below }, Diagram) => super::compose_wide_arrow(*symbol, above.as_deref(), below.as_deref(), space, cell_style, ctx)?,
                _ => c.math_box.clone(),
            };
            let leftover = space - math_box.width;
            let dx = match c.alignment.unwrap_or_else(|| alignment(j)) {
                'l' => 0.0,
                'r' => leftover,
                _ => leftover / 2.0,
            };
            body.place(&math_box, x_col[j] + dx, top + bases[i]);
        }
    }
    for (j, c) in boundary_content.iter().enumerate() {
        if let Some(c) = c {
            for (i, _) in cells.iter().enumerate() {
                body.place(c, x_boundary[j], top + bases[i]);
            }
        }
    }
    // vertical rules per table row: a merged cell (\multicolumn) interrupts the rule
    let (dash, gap) = (em(e, 0.4), em(e, 0.4));
    for (j, dashed) in lines.verticals.iter().map(|j| (j, false)).chain(lines.dashed.iter().map(|j| (j, true))) {
        if *j > ncol {
            continue;
        }
        // between columns the rule sits in the middle of the two \arraycolsep; on the outside, at
        // the edge, the \arraycolsep inside it (as LaTeX's array puts it)
        let xv = if boundary_content[*j].is_some() || *j == 0 {
            x_boundary[*j]
        } else if *j == ncol {
            x_boundary[*j] + boundary[*j] - thickness
        } else {
            x_boundary[*j] + boundary[*j] / 2.0 - thickness / 2.0
        };
        for (i, row) in cells.iter().enumerate() {
            let crosses = intertexts.contains(&i) || row.iter().any(|c| c.span > 1 && c.column < *j && *j < c.column + c.span);
            if crosses {
                continue;
            }
            // from the row's top rule to the next one, in the body's coordinates
            let (low, length) = (top + y_rule[i + 1], y_rule[i] - y_rule[i + 1]);
            if dashed {
                for (from, length) in dashes(length, dash, gap) {
                    body.items.push(Item::Rule { x: xv, y: low + from, width: thickness, height: length, color: None });
                }
            } else {
                body.items.push(Item::Rule { x: xv, y: low, width: thickness, height: length, color: None });
            }
        }
    }
    for t in horizontals {
        let from = t.from.min(ncol.saturating_sub(1));
        let to = t.to.min(ncol - 1);
        let (x0, x1) = if t.to == usize::MAX && t.from == 0 { (0.0, total_width) } else { (x_col[from] - boundary[from] / 2.0, x_col[to] + widths[to] + boundary[to + 1] / 2.0) };
        let yt = top + y_rule[t.row.min(cells.len())] - thickness;
        let (x0, width) = (x0.max(0.0), (x1 - x0.max(0.0)).min(total_width));
        if t.dashed {
            for (from, length) in dashes(width, dash, gap) {
                body.items.push(Item::Rule { x: x0 + from, y: yt, width: length, height: thickness, color: None });
            }
        } else {
            body.items.push(Item::Rule { x: x0, y: yt, width, height: thickness, color: None });
        }
    }
    if let Some((open, close)) = &border {
        // center the parentheses on the core (rows 2 onward)
        let top = top + y_rule[1];
        let low = top + y_rule[cells.len()];
        let axis = k(e, |c| c.axis_height());
        let dy = (top + low) / 2.0 - axis;
        body.place(open, x_col[1] - open.width - em(e, 0.1), dy);
        body.place(close, x_col[ncol - 1] + widths[ncol - 1] + em(e, 0.05), dy);
    }
    body.height = top;
    body.depth = total_height - top;

    // matrix delimiters
    let (open, close) = match kind {
        PMatrix => ('(', Some(')')),
        BMatrix => ('[', Some(']')),
        BraceMatrix => ('{', Some('}')),
        VMatrix => ('|', Some('|')),
        VVMatrix => ('‖', Some('‖')),
        Cases => ('{', None),
        RCases => ('}', None),
        _ => return Ok(body),
    };
    let half = (body.height - axis).max(body.depth + axis);
    let target = (2.0 * half * 0.901).max(2.0 * half - em(e, 0.5));
    if kind == RCases {
        // the mirror of cases: content, a little space, the brace on the right
        let mut math_box = MathBox::space(em(e, 0.12));
        math_box.append(&body);
        math_box.append(&MathBox::space(em(e, 0.17)));
        math_box.append(&stretch::delimiter(open, target, e)?);
        return Ok(math_box);
    }
    let mut math_box = stretch::delimiter(open, target, e)?;
    if kind == Cases {
        math_box.append(&MathBox::space(em(e, 0.17)));
    }
    math_box.append(&body);
    match close {
        Some(c) => math_box.append(&stretch::delimiter(c, target, e)?),
        None => math_box.append(&MathBox::space(em(e, 0.12))),
    }
    Ok(math_box)
}

/// The dashes of a dashed rule `length` long, as (start, length): as many as fit, the gaps
/// adjusted so the rule starts and ends with a dash.
fn dashes(length: f32, dash: f32, gap: f32) -> Vec<(f32, f32)> {
    if length <= dash {
        return vec![(0.0, length.max(0.0))];
    }
    let n = ((length + gap) / (dash + gap)).round().max(2.0) as usize;
    let gap = (length - n as f32 * dash) / (n - 1) as f32;
    if gap <= 0.0 {
        return vec![(0.0, length)];
    }
    (0..n).map(|i| (i as f32 * (dash + gap), dash)).collect()
}
