//! The LaTeX packages contextEquation reads through its own commands, in the forms the other suites leave:
//! tikz-cd and xymatrix arrows with their rarer keys, switches and recoveries; mathtools' boxes,
//! limits and environments at their edges; amsmath's root moves and KaTeX's `darray`; siunitx's
//! real-only complex numbers; xcolor mixes with an unknown color.

use context_equation_core::tree::{ArrowHead, ArrowLine, ArrowTail, DiagramArrow, LabelSide, Node};
use context_equation_core::{convert, Options};

fn strict(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options::default()).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    out
}

/// The tolerant output of an input strict mode refuses, with the warning that names `problem`;
/// strict mode's error says the same.
fn recovered(tex: &str, problem: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"));
    assert!(out.warnings.iter().any(|w| w.description.contains(problem)), "{tex}: {:?}", out.warnings);
    let error = convert(tex, &Options { strict: true, ..Default::default() }).expect_err(tex);
    assert!(error.to_string().contains(problem), "{tex}: {error}");
    out
}

fn diagram_of(out: &context_equation_core::Output) -> (Vec<Vec<Node>>, Vec<DiagramArrow>) {
    match out.tree.single() {
        Node::ArrowDiagram { cells, arrows } => (cells.clone(), arrows.clone()),
        other => panic!("{other:?}"),
    }
}

fn arrows(tex: &str) -> Vec<DiagramArrow> {
    diagram_of(&strict(tex)).1
}

fn arrow(tex: &str) -> DiagramArrow {
    let all = arrows(tex);
    assert_eq!(all.len(), 1, "{tex}: {all:?}");
    all[0].clone()
}

fn identifier(name: &str) -> Node {
    Node::Identifier { name: name.into(), variant: Default::default() }
}

#[test]
fn tikzcd_grids_options_and_cells() {
    // an empty last row, after a trailing \\, is not a row
    let (cells, _) = diagram_of(&strict(r"\begin{tikzcd} A \arrow[r] & B \\ & \\ \end{tikzcd}"));
    assert_eq!(cells.len(), 1);
    // the environment's options, with braces and brackets inside, are read and left out
    let (cells, list) = diagram_of(&strict(r"\begin{tikzcd}[row sep={1em}, column sep=[2em]] A \arrow[r] & B \end{tikzcd}"));
    assert_eq!(cells, vec![vec![identifier("A"), identifier("B")]]);
    assert_eq!(list.len(), 1);
    // a space between \arrow and its options
    assert_eq!(arrow(r"\begin{tikzcd} A \arrow [r] & B \end{tikzcd}").to, (0, 1));
    // a control symbol in a cell stays in the cell
    let (cells, _) = diagram_of(&strict(r"\begin{tikzcd} A\, \arrow[r] & B \end{tikzcd}"));
    assert!(matches!(&cells[0][0], Node::Row { children } if matches!(children[1], Node::Space { .. })), "{:?}", cells[0][0]);
    // \arrow with no options has no direction
    let out = recovered(r"\begin{tikzcd} A \arrow & B \end{tikzcd}", r"arrow without a direction: \arrow[]");
    assert!(diagram_of(&out).1.is_empty());
}

#[test]
fn tikzcd_arrow_keys() {
    // an empty item between commas is nothing
    let a = arrow(r#"\begin{tikzcd} A \arrow[r,,"f"] & B \end{tikzcd}"#);
    assert_eq!(a.labels[0].node, identifier("f"));
    // the double arrows to the left and both ways
    let a = arrow(r"\begin{tikzcd} A \arrow[r, Leftarrow] & B \end{tikzcd}");
    assert_eq!((a.line, a.head, a.tail), (ArrowLine::Double, ArrowHead::None, ArrowTail::Head));
    let a = arrow(r"\begin{tikzcd} A \arrow[r, Leftrightarrow] & B \end{tikzcd}");
    assert_eq!((a.line, a.head, a.tail), (ArrowLine::Double, ArrowHead::Normal, ArrowTail::Head));
    let a = arrow(r"\begin{tikzcd} A \arrow[r, leftarrow] & B \end{tikzcd}");
    assert_eq!((a.line, a.head, a.tail), (ArrowLine::Solid, ArrowHead::None, ArrowTail::Head));
    // phantom after a label: no line, the label on where the arrow would be
    let a = arrow(r#"\begin{tikzcd} A \arrow[r, "f", phantom] & B \end{tikzcd}"#);
    assert_eq!((a.line, a.head, a.labels[0].side), (ArrowLine::None, ArrowHead::None, LabelSide::On));
    // swap after the labels sends them all to the right
    let a = arrow(r#"\begin{tikzcd} A \arrow[r, "f", "g", swap] & B \end{tikzcd}"#);
    assert!(a.labels.iter().all(|l| l.side == LabelSide::Right), "{a:?}");
    // pos= as a key of the label, and of the arrow for the labels without one
    let a = arrow(r#"\begin{tikzcd} A \arrow[r, "f" pos=0.3, "g", pos=0.8] & B \end{tikzcd}"#);
    assert_eq!((a.labels[0].position, a.labels[1].position), (0.3, 0.8));
}

#[test]
fn tikzcd_from_and_to_name_the_ends() {
    let ends = |tex: &str| {
        let a = arrow(tex);
        (a.from, a.to)
    };
    // from= alone: from that cell to the current one
    assert_eq!(ends(r"\begin{tikzcd} A & B \arrow[from=1-1] \end{tikzcd}"), ((0, 0), (0, 1)));
    // to= alone: from the current cell to that one
    assert_eq!(ends(r"\begin{tikzcd} A \arrow[to=1-2] & B \end{tikzcd}"), ((0, 0), (0, 1)));
    // both: the current cell is neither end, wherever the arrow is written
    assert_eq!(ends(r"\begin{tikzcd} A & B \arrow[from=1-1, to=1-2] \end{tikzcd}"), ((0, 0), (0, 1)));
    assert_eq!(ends(r"\begin{tikzcd} A & B \\ C & D \arrow[from=1-1, to=1-2] \end{tikzcd}"), ((0, 0), (0, 1)));
    // from= with a direction: the direction counts from the current cell, not from from=
    assert_eq!(ends(r"\begin{tikzcd} A & B \\ C \arrow[from=1-1, r] & D \end{tikzcd}"), ((0, 0), (1, 1)));
    assert_eq!(ends(r"\begin{tikzcd} A & B \\ C & D \arrow[u, from=2-1] \end{tikzcd}"), ((1, 0), (0, 1)));
    // a direction as the value of from= or to= counts from the current cell too
    assert_eq!(ends(r"\begin{tikzcd} A & B \arrow[from=l] \end{tikzcd}"), ((0, 0), (0, 1)));
    assert_eq!(ends(r"\begin{tikzcd} A & B \\ C \arrow[from=u, to=ur] & D \end{tikzcd}"), ((0, 0), (0, 1)));
    // a direction from the current cell that leaves the diagram
    recovered(r"\begin{tikzcd} A & B \arrow[from=1-1, uu] \end{tikzcd}", r"arrow pointing outside the diagram: \arrow[from=1-1, uu]");
    recovered(r"\begin{tikzcd} A \arrow[from=l] & B \end{tikzcd}", r"arrow pointing outside the diagram: \arrow[from=l]");
    // from= naming the current cell, with no target: an arrow to itself
    recovered(r"\begin{tikzcd} A \arrow[from=1-1] & B \end{tikzcd}", "arrow from a cell to itself");
    // a cell that is neither row-column nor a direction
    recovered(r"\begin{tikzcd} A \arrow[r, from=x] & B \end{tikzcd}", "arrow cell not in row-column form: from=x");
    // rows and columns count from 1
    recovered(r"\begin{tikzcd} A \arrow[r, from=0-1] & B \end{tikzcd}", "arrow cell not in row-column form: from=0-1");
}

#[test]
fn xymatrix_shifts_styles_and_waypoints() {
    // @<2pt>: moved 0.2 em to the left of its direction; an amount that is not one moves 2 pt
    assert_eq!(arrow(r"\xymatrix{A \ar@<2pt>[r] & B}").shift, 0.2);
    assert_eq!(arrow(r"\xymatrix{A \ar@<x>[r] & B}").shift, 0.2);
    // @-> without braces: a style of its own
    let a = arrow(r"\xymatrix{A \ar@->[r] & B}");
    assert_eq!((a.line, a.head), (ArrowLine::Solid, ArrowHead::Normal));
    // @{} with a label: no line, the label on the path
    let a = arrow(r"\xymatrix{A \ar@{}[r]^f & B}");
    assert_eq!((a.line, a.head, a.labels[0].side), (ArrowLine::None, ArrowHead::None, LabelSide::On));
    // the tails xy-pic writes before the line
    assert_eq!(arrow(r"\xymatrix{A \ar@{_{(}->}[r] & B}").tail, ArrowTail::HookRight);
    assert_eq!(arrow(r"\xymatrix{A \ar@{<->}[r] & B}").tail, ArrowTail::Head);
    assert_eq!(arrow(r"\xymatrix{A \ar@{>->}[r] & B}").tail, ArrowTail::Tail);
    // a waypoint after a space
    assert_eq!(arrow(r"\xymatrix{A \ar' [d][r] & B \\ C & D}").via, vec![(1, 0)]);
    // a waypoint that is not a direction: the path goes straight
    let out = recovered(r"\xymatrix{A \ar'[x][r] & B}", "unsupported xymatrix waypoint: '[x]");
    assert!(diagram_of(&out).1[0].via.is_empty());
    // a waypoint outside the diagram drops the path through the others, and the labels' segments
    let out = recovered(r"\xymatrix{A \ar'[u]'[u]^f[r] & B}", r"\ar going through a cell outside the diagram");
    let a = &diagram_of(&out).1[0];
    assert!(a.via.is_empty());
    assert_eq!(a.labels[0].segment, 0);
}

#[test]
fn xymatrix_labels_in_their_rarer_forms() {
    // ^(0.3): the label at that fraction of the arrow, the label after a space
    let a = arrow(r"\xymatrix{A \ar[r]^(0.3) f & B}");
    assert_eq!((a.labels[0].position, a.labels[0].node.clone()), (0.3, identifier("f")));
    // a control symbol as the label
    let a = arrow(r"\xymatrix{A \ar[r]^\, & B}");
    assert!(matches!(a.labels[0].node, Node::Space { .. }));
    // a position never closed is read to the end of the entry
    let out = convert(r"\xymatrix{A \ar[r]^(0.3 & B}", &Options::default()).unwrap();
    let a = &diagram_of(&out).1[0];
    assert_eq!((a.to, a.labels[0].position), ((0, 1), 0.3));
    // an @ before the direction with no style after it
    assert_eq!(arrow(r"\xymatrix{A \ar@[r] & B}").to, (0, 1));
    // a label mark at the very end: an empty label, and the arrow leaves the one-cell diagram
    let out = recovered(r"\xymatrix{A \ar[r]^}", "arrow to a cell outside the diagram");
    assert!(diagram_of(&out).1.is_empty());
}

#[test]
fn two_cells_switches_tips_and_nudges() {
    // <\omit> or \omit: no curves, the Arrow alone
    for tex in [r"\xymatrix{A \rtwocell<\omit> & B}", r"\xymatrix{A \rtwocell\omit & B}"] {
        let a = arrow(tex);
        assert!(a.across && a.line == ArrowLine::Double, "{tex}: {a:?}");
    }
    // ~: an object xy-pic changes, left out with its braced part, whatever comes before it
    for tex in [r"\xymatrix{A \rtwocell~{x} & B}", r"\xymatrix{A \rtwocell~*{x} & B}"] {
        let out = recovered(tex, "2-cell switch ~");
        assert_eq!(diagram_of(&out).1.len(), 3, "{tex}");
    }
    // a nudge that is not a number: the default curves
    let out = recovered(r"\xymatrix{A \rtwocell<x> & B}", "2-cell nudge <x>");
    assert_eq!(diagram_of(&out).1[0].sag, 1.75);
    // nudge 0 with an upper label only: one straight arrow with it
    let list = arrows(r"\xymatrix{A \rtwocell<0>^f & B}");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].labels.len(), 1);
    // the tips of the central Arrow: ' clockwise (the lower curve turned round), ` anticlockwise
    // (the upper one), " both ends, ! none; with them the Arrow itself is not drawn
    let list = arrows(r"\xymatrix{A \rtwocell{'} & B}");
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].from, list[1].from), ((0, 0), (0, 1)));
    let list = arrows(r"\xymatrix{A \rtwocell{`} & B}");
    assert_eq!((list[0].from, list[1].from), ((0, 1), (0, 0)));
    let list = arrows(r#"\xymatrix{A \rtwocell{"} & B}"#);
    assert!(list.iter().all(|a| a.tail == ArrowTail::Head && a.head == ArrowHead::Normal));
    let list = arrows(r"\xymatrix{A \rtwocell{!} & B}");
    assert!(list.iter().all(|a| a.head == ArrowHead::None));
    // a turned curve takes its label to the other side
    let list = arrows(r"\xymatrix{A \rtwocell^f_g{'} & B}");
    assert_eq!((list[1].from, list[1].labels[0].side), ((0, 1), LabelSide::Left));
    // {<2>\alpha}: the Arrow moved 2 dashes clockwise, with its label
    let list = arrows(r"\xymatrix{A \rtwocell{<2>\alpha} & B}");
    assert_eq!(list[2].sag, 1.0);
    assert_eq!(list[2].labels[0].node, identifier("α"));
    // nudge 0: one straight arrow with both labels
    let list = arrows(r"\xymatrix{A \rtwocell<0>^f_g & B}");
    assert_eq!(list[0].labels.len(), 2);
    assert_eq!(list[0].labels[1].side, LabelSide::Right);
    assert_eq!(list[0].sag, 0.0);
    // a control symbol as a label
    let list = arrows(r"\xymatrix{A \rtwocell^\alpha_\, & B}");
    assert!(matches!(list[1].labels[0].node, Node::Space { .. }));
}

#[test]
fn unterminated_groups_in_a_diagram_read_to_the_end() {
    // a 2-cell label mark at the end, and a label group never closed
    recovered(r"\xymatrix{A \rtwocell^", "'{' without a matching '}'");
    recovered(r"\xymatrix{A \ar@{.>}[r]^{f & B}", "'{' without a matching '}'");
}

#[test]
fn mathtools_boxes_limits_and_environments_at_their_edges() {
    // \mathmakebox without a width, or with one that is not a length: the content as it is
    assert_eq!(*strict(r"\mathmakebox{x}").tree.single(), identifier("x"));
    assert_eq!(*strict(r"\mathmakebox[foo]{x}").tree.single(), identifier("x"));
    // \adjustlimits where the input ends: not the two operators it takes
    recovered(r"\adjustlimits\lim", r"\adjustlimits takes two operators with limits");
    recovered(r"\adjustlimits\lim_", r"\adjustlimits takes two operators with limits");
    // an environment the equation defined itself is its own, not mathtools'
    let out = strict(r"\newenvironment{multlined}{[}{]} \begin{multlined} a \end{multlined}");
    assert_eq!(out.text, "[a]");
    // \Aboxed splits at the first & outside braces, past an escaped \&
    let out = strict(r"\begin{aligned}\Aboxed{a\&b + {c} &= e}\end{aligned}");
    assert!(out.mathml.contains(r#"<mphantom><mrow></mrow><mo>=</mo><mi>e</mi></mphantom>"#), "{}", out.mathml);
    // with no & at all, everything is the left side
    let out = strict(r"\begin{aligned}\Aboxed{x}\end{aligned}");
    assert!(out.mathml.contains("<mphantom></mphantom>"), "{}", out.mathml);
    // lgathered: a starred \\ and one with a space both end a line
    let out = strict(r"\begin{lgathered} a \\* b \\[2pt] c \end{lgathered}");
    let Node::Table { rows, .. } = out.tree.single() else { panic!("{:?}", out.tree) };
    assert_eq!(rows.len(), 3);
}

#[test]
fn root_moves_and_display_arrays() {
    // \leftroot with something that is not a number is left out
    let out = recovered(r"\sqrt[\leftroot{x} 3]{y}", r"\leftroot{x}: not a number");
    assert!(matches!(out.tree.single(), Node::Root { index: Some(i), .. } if matches!(**i, Node::Number { .. })), "{:?}", out.tree);
    // \uproot without braces
    let out = strict(r"\sqrt[\uproot 2 3]{y}");
    assert!(matches!(out.tree.single(), Node::Root { index: Some(i), .. } if matches!(**i, Node::Raised { .. })), "{:?}", out.tree);
    // darray: each cell in display style, past a \\ with a space
    let out = strict(r"\begin{darray}{cc} a & b \\[2pt] c & d \end{darray}");
    let Node::Table { rows, .. } = out.tree.single() else { panic!() };
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().flatten().all(|c| matches!(c, Node::Style { level: 0, .. })), "{rows:?}");
    // and past a plain \\
    let out = strict(r"\begin{darray}{c} a \\ b \end{darray}");
    let Node::Table { rows, .. } = out.tree.single() else { panic!() };
    assert!(rows.iter().flatten().all(|c| matches!(c, Node::Style { level: 0, .. })), "{rows:?}");
    // \skew takes a number of mu
    let error = convert(r"\skew{x}\hat{a}", &Options::default()).unwrap_err();
    assert!(error.to_string().contains(r#"\skew takes a number of mu, not "x""#), "{error}");
}

#[test]
fn complex_numbers_with_a_real_part_only() {
    let out = strict(r"\complexnum{5}");
    assert!(matches!(out.tree.single(), Node::Number { value, .. } if value == "5"));
    // a quantity with one part has no parentheses
    assert_eq!(strict(r"\complexqty{5}{\ohm}").text, "5 Ω");
}

#[test]
fn a_color_mix_with_an_unknown_color_is_kept_as_written() {
    // a mix of known colors is computed
    let out = strict(r"\color{red!50!blue} x");
    assert!(out.svg.contains("#800080"), "{}", out.svg);
    // with a color nobody defined, the mix cannot be computed: drawn without color
    let out = recovered(r"\color{red!40!nosuch} x", "invalid color: red!40!nosuch");
    assert_eq!(*out.tree.single(), identifier("x"));
}
