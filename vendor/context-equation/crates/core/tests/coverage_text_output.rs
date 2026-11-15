//! Linear text of the arrows of a diagram: double lines that point back, and an arrow off the grid.

use context_equation_core::output::text::linear;
use context_equation_core::{convert, Locale, Node, Options};

fn text(tex: &str) -> String {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}")).text
}

#[test]
fn double_arrows_that_point_back_are_written_as_their_characters() {
    // tikz-cd's Leftarrow is ⇐, its Leftrightarrow ⇔
    assert_eq!(text(r"\begin{tikzcd} A \arrow[r, Leftarrow] & B \arrow[r, Leftrightarrow] & C \end{tikzcd}"), "A, B, C; A ⇐ B; B ⇔ C");
    assert_eq!(text(r"\begin{tikzcd} A \arrow[r, Longleftarrow] & B \end{tikzcd}"), "A, B; A ⇐ B");
}

#[test]
fn tikzcd_arrows_go_where_from_and_to_say() {
    // from= draws from that cell to the current one, unless to= names the target
    assert_eq!(text(r"\begin{tikzcd} A & B \arrow[from=1-1] \end{tikzcd}"), "A, B; A → B");
    assert_eq!(text(r"\begin{tikzcd} A & B \arrow[from=1-1, to=1-2] \end{tikzcd}"), "A, B; A → B");
    assert_eq!(text(r"\begin{tikzcd} A \arrow[to=1-2] & B \end{tikzcd}"), "A, B; A → B");
}

#[test]
fn an_arrow_to_a_cell_past_the_diagram_points_at_nothing() {
    // the reader leaves such an arrow out (E-REC-7)
    let r = convert(r"\begin{tikzcd} A \arrow[rr] & B \end{tikzcd}", &Options::default()).unwrap();
    assert_eq!(r.text, "A, B");
    assert!(r.warnings.iter().any(|w| w.rule == "E-REC-7"));
    // a tree changed by hand may still have one: its end is an empty cell
    let r = convert(r"\begin{tikzcd} A \arrow[r] & B \end{tikzcd}", &Options::default()).unwrap();
    let Some(Node::ArrowDiagram { cells, arrows }) = r.tree.children().first() else { panic!("{:?}", r.tree) };
    let mut arrow = arrows[0].clone();
    arrow.to = (0, 5);
    assert_eq!(linear(&arrow.as_row(cells), Locale::En), "A →");
}
