//! The layout of commutative diagrams and of broken lines, in the cases the other suites leave:
//! rows of different lengths, labels on a vertical arrow, dotted arrows broken by their label,
//! curves between entries too wide for them to be cut at the borders, and a formula wider than the
//! line with nowhere to break.

use context_equation_core::font::FallbackFont;
use context_equation_core::layout::{compose, Item, MathBox, Style};
use context_equation_core::rules::Registry;
use context_equation_core::tree::Node;
use context_equation_core::{convert, typeset, Options};

fn boxed(tex: &str) -> MathBox {
    typeset(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn lines(b: &MathBox) -> usize {
    b.items.iter().filter(|i| matches!(i, Item::Line { .. })).count()
}

/// Every coordinate of the drawing is a number.
fn finite(b: &MathBox) -> bool {
    b.items.iter().all(|i| match i {
        Item::Line { x1, y1, x2, y2, .. } => [x1, y1, x2, y2].iter().all(|v| v.is_finite()),
        _ => true,
    }) && b.width.is_finite()
}

fn identifier(name: &str) -> Node {
    Node::Identifier { name: name.into(), variant: Default::default() }
}

fn compose_tree(node: &Node) -> MathBox {
    let mut registry = Registry::default();
    let fallbacks: Vec<FallbackFont> = Vec::new();
    let mut ctx = context_equation_core::layout::Context { locale: Default::default(), registry: &mut registry, strict: true, kerning: Default::default(), fallbacks: &fallbacks };
    compose(node, Style::display(), &mut ctx).unwrap()
}

#[test]
fn a_short_row_is_padded_with_empty_cells() {
    // the reader pads the rows; a tree built otherwise may not be: the layout pads it too
    let ragged = Node::ArrowDiagram { cells: vec![vec![identifier("A"), identifier("B")], vec![identifier("C")]], arrows: vec![] };
    let padded = Node::ArrowDiagram { cells: vec![vec![identifier("A"), identifier("B")], vec![identifier("C"), Node::row(vec![])]], arrows: vec![] };
    let (a, b) = (compose_tree(&ragged), compose_tree(&padded));
    assert_eq!((a.width, a.height, a.depth), (b.width, b.height, b.depth));
    assert_eq!(a.items.len(), b.items.len());
}

#[test]
fn a_label_on_a_vertical_arrow_opens_the_rows() {
    // a tall label on the arrow (description) needs room between the rows; beside it, it does not
    let on = boxed(r#"\begin{tikzcd} A \arrow[d, "\displaystyle\frac{a}{b}" description] \\ B \end{tikzcd}"#);
    let beside = boxed(r#"\begin{tikzcd} A \arrow[d, "\displaystyle\frac{a}{b}"] \\ B \end{tikzcd}"#);
    assert!(on.height + on.depth > beside.height + beside.depth, "{} {}", on.height + on.depth, beside.height + beside.depth);
}

#[test]
fn a_dotted_arrow_leaves_out_the_dots_under_its_label() {
    let on = boxed(r"\xymatrix{A \ar@{.>}[r]|f & B}");
    let above = boxed(r"\xymatrix{A \ar@{.>}[r]^f & B}");
    assert!(lines(&on) < lines(&above), "{} {}", lines(&on), lines(&above));
}

#[test]
fn curves_between_entries_too_wide_to_cut_at_their_borders() {
    // between two very wide entries the curve's samples all fall inside one entry or the other: the
    // arrow is drawn from border to border instead, and nothing is lost
    // (tall too, for the 2-cell's curves, which bow above and below)
    let wide = r"\rule{200em}{8em}";
    for arrow in [r"\ar@(r,l)[r]", r"\rtwocell"] {
        let tex = format!(r"\xymatrix{{{wide} {arrow} & {wide}}}");
        let with = boxed(&tex);
        let without = boxed(&format!(r"\xymatrix{{{wide} & {wide}}}"));
        assert!(finite(&with), "{tex}");
        assert!(lines(&with) > lines(&without), "{tex}");
        assert_eq!(with.width, without.width, "{tex}");
    }
}

#[test]
fn a_path_through_its_own_entry() {
    // '[] is a waypoint at the arrow's own entry: a first segment that goes nowhere
    let b = boxed(r"\xymatrix{A \ar'[][r] & B}");
    assert!(finite(&b));
    assert!(lines(&b) > lines(&boxed(r"\xymatrix{A & B}")));
}

#[test]
fn a_formula_with_nowhere_to_break_is_drawn_wider() {
    // letters only, with no relation nor operator: no break point, so one line wider than asked
    let options = Options { max_width: Some(2.0), ..Default::default() };
    let out = convert(r"abcdefghijklmnop", &options).unwrap();
    assert!(out.applied.iter().any(|a| a.rule == "L-BRK-2" && a.description.contains("the formula cannot be broken within 2em")), "{:?}", out.applied);
    assert!(!out.svg.contains("data-line"));
}
