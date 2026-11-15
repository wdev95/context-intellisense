//! Issue #215: xymatrix paths through entries (`\ar'[d][dd]`), `\hole` and 2-cells (`\rtwocell`,
//! `\rrtwocell<5>^{f}_{g}`), as the xy-pic reference manual (notes 24d and 24j, section 25) draws them.

use context_equation_core::tree::{ArrowHead, ArrowLine, DiagramArrow, LabelSide, Node};
use context_equation_core::{convert, Input, Options};

fn strict(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options::default()).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    out
}

fn arrows(tex: &str) -> (Vec<Vec<Node>>, Vec<DiagramArrow>) {
    match strict(tex).tree.single() {
        Node::ArrowDiagram { cells, arrows } => (cells.clone(), arrows.clone()),
        other => panic!("{tex}: {other:?}"),
    }
}

#[test]
fn paths_through_entries() {
    // the issue's example: from A through C to E
    let (_, a) = arrows(r"\xymatrix{A \ar'[d][dd] & B \\ C & D \\ E}");
    assert_eq!((a[0].from, a[0].via.clone(), a[0].to), ((0, 0), vec![(1, 0)], (2, 0)));
    // several waypoints, each relative to the arrow's own entry
    let (_, a) = arrows(r"\xymatrix{A \ar'[r]'[rd][rdd] & \\ & \\ & D}");
    assert_eq!((a[0].via.clone(), a[0].to), (vec![(0, 1), (1, 1)], (2, 1)));
    // a label belongs to the segment it follows
    let (_, a) = arrows(r"\xymatrix{A \ar'[r]^a[rd]_b & \\ & D}");
    let segments: Vec<usize> = a[0].labels.iter().map(|l| l.segment).collect();
    assert_eq!(segments, [0, 1]);
    let (_, a) = arrows(r"\xymatrix{A \ar^f'[r][rd] & \\ & D}");
    assert_eq!(a[0].labels[0].segment, 1);
    // drawn as a corner, not as the straight arrow
    assert_ne!(strict(r"\xymatrix{A \ar'[d][dr] & B \\ & D}").svg, strict(r"\xymatrix{A \ar[dr] & B \\ & D}").svg);
    // the text and the speech read the arrow from its start to its end
    assert!(strict(r"\xymatrix{A \ar'[d][dd] & B \\ C & D \\ E}").text.ends_with("A → E"));
    // a waypoint outside the diagram: the arrow is left out
    let out = convert(r"\xymatrix{A \ar'[u][r] & B}", &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-7"), "{:?}", out.warnings);
    assert!(convert(r"\xymatrix{A \ar'[u][r] & B}", &Options { strict: true, ..Default::default() }).is_err());
}

#[test]
fn curves_with_in_and_out_directions() {
    // #256, the issue's example: a loop on A leaving up-left and coming back from up-right, labelled f,
    // with no target (the path's position is empty: the entry itself), and a curve from B to A
    let (_, a) = arrows(r"\xymatrix{ A \ar@(ul,ur)^{f} \ar[r] & B \ar@(rd,ul)[l] }");
    assert_eq!(a.len(), 3);
    assert_eq!((a[0].from, a[0].to, a[0].directions), ((0, 0), (0, 0), Some((135, 45))));
    assert_eq!(a[0].labels.len(), 1);
    assert_eq!((a[2].from, a[2].to, a[2].directions), ((0, 1), (0, 0), Some((315, 135))));
    // an explicit [] is the same loop; either order of a diagonal's letters (reference manual, note 4l)
    assert_eq!(arrows(r"\xymatrix{ A \ar@(lu,ru)[]^{f} }").1, arrows(r"\xymatrix{ A \ar@(ul,ur)^{f} }").1);
    // the curve is drawn: not the straight arrow, and a loop is drawn at all
    assert_ne!(strict(r"\xymatrix{ A \ar@(u,u)[r] & B }").svg, strict(r"\xymatrix{ A \ar[r] & B }").svg);
    assert!(strict(r"\xymatrix{ A \ar@(ul,dl)[] }").svg.contains("<line"));
    // the cell keeps no text of the arrow, and the text reads the loop
    let out = strict(r"\xymatrix{ A \ar@(ul,ur)^{f} \ar[r] & B \ar@(rd,ul)[l] }");
    assert_eq!(out.text, "A, B; A →[f] A; A → B; B → A");
    // the Stacks Project's long exact sequences: the connecting map from the end of a row to the start
    // of the next
    let (_, a) = arrows(r"\xymatrix{ 0 \ar[r] & A \ar[r] & B \ar@(rd, ul)[rdll] \\ & C \ar[r] & D }");
    assert_eq!((a[2].from, a[2].to), ((0, 2), (1, 1)));
    // a direction it cannot read: drawn straight with a warning, refused in strict mode
    let tex = r"\xymatrix{ A \ar@(v(1,2),ur)[r] & B }";
    let out = convert(tex, &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1"), "{:?}", out.warnings);
    assert!(matches!(out.tree.single(), Node::ArrowDiagram { arrows, .. } if arrows.len() == 1 && arrows[0].directions.is_none()));
    assert!(convert(tex, &Options { strict: true, ..Default::default() }).is_err());
}

#[test]
fn holes() {
    // as an entry, an empty cell
    let (cells, a) = arrows(r"\xymatrix{A \ar[r] & \hole & B}");
    assert_eq!(cells[0][1], Node::row(vec![]));
    assert_eq!(a[0].to, (0, 1));
    // as a break, a gap in the line with nothing in it
    let (_, a) = arrows(r"\xymatrix{A \ar[dr] & B \ar[dl]|\hole \\ C & D}");
    assert_eq!(a[1].labels[0].side, LabelSide::On);
    let crossing = strict(r"\xymatrix{A \ar[dr] & B \ar[dl]|\hole \\ C & D}");
    let plain = strict(r"\xymatrix{A \ar[dr] & B \ar[dl] \\ C & D}");
    assert_ne!(crossing.svg, plain.svg);
    assert_eq!((crossing.speech, crossing.text, crossing.braille), (plain.speech, plain.text, plain.braille));
}

#[test]
fn two_cells() {
    // \rtwocell: two curved arrows to the right, and the Arrow across them, clockwise
    let (_, a) = arrows(r"\xymatrix{A \rtwocell & B}");
    assert_eq!(a.len(), 3);
    assert_eq!((a[0].from, a[0].to, a[0].sag), ((0, 0), (0, 1), 1.75));
    assert_eq!((a[1].from, a[1].to, a[1].sag), ((0, 0), (0, 1), -1.75));
    assert!(a[2].across && a[2].line == ArrowLine::Double && a[2].head == ArrowHead::Normal);
    // the issue's other example: two columns, nudge 5, labels outside each curve
    let (_, a) = arrows(r"\xymatrix{A \rrtwocell<5>^{f}_{g} && B}");
    assert_eq!((a[0].to, a[0].sag, a[1].sag), ((0, 2), 2.5, -2.5));
    assert_eq!((a[0].labels[0].side, a[1].labels[0].side), (LabelSide::Left, LabelSide::Right));
    // the Arrow's label, its orientation and equality
    let (_, a) = arrows(r"\xymatrix{A \rtwocell^f_g{\alpha} & B}");
    assert!(a[2].across && a[2].labels.len() == 1);
    let (_, a) = arrows(r"\xymatrix{A \rtwocell^f_g{^\alpha} & B}");
    assert_eq!((a[2].from, a[2].to), ((0, 1), (0, 0)));
    let (_, a) = arrows(r"\xymatrix{A \rtwocell{=} & B}");
    assert_eq!(a[2].head, ArrowHead::None);
    let (_, a) = arrows(r"\xymatrix{A \rtwocell{\omit} & B}");
    assert_eq!(a.len(), 2);
    // nudge 0: a single straight arrow with both labels
    let (_, a) = arrows(r"\xymatrix{A \rtwocell<0>^f_g{\omit} & B}");
    assert_eq!((a.len(), a[0].sag, a[0].labels.len()), (1, 0.0, 2));
    // the halves, and other directions
    let (_, a) = arrows(r"\xymatrix{A \ruppertwocell^f{\alpha} \rlowertwocell_h{\beta} \ar[r]|g & B}");
    assert_eq!(a.iter().filter(|x| !x.across).count(), 3);
    let (_, a) = arrows(r"\xymatrix{A \dtwocell^f_g{\alpha} \\ B}");
    assert_eq!(a[0].to, (1, 0));
    // read as arrows; drawn
    let out = strict(r"\xymatrix{A \rtwocell^f_g{\alpha} & B}");
    assert!(out.text.contains("A →[f] B") && out.text.contains("A ⇒[α] B"), "{}", out.text);
    // what xy-pic draws otherwise is left out with a warning; a 2-cell out of the diagram too
    for tex in [r"\xymatrix{A \rtwocell^<3>f & B}", r"\xymatrix{A \rtwocell! & B}", r"\xymatrix{A \ltwocell & B}"] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(!out.warnings.is_empty(), "{tex}");
        assert!(convert(tex, &Options { strict: true, ..Default::default() }).is_err(), "{tex}");
    }
}

#[test]
fn mathml_keeps_the_fallback() {
    for tex in [r"\xymatrix{A \ar'[d][dd] & B \\ C & D \\ E}", r"\xymatrix{A \rtwocell^f_g{\alpha} & B}", r"\xymatrix{A \ar[r] & \hole & B}"] {
        let out = strict(tex);
        assert!(out.mathml.contains("<semantics>"), "{tex}: {}", out.mathml);
        assert!(convert(&out.mathml, &Options { input: Input::MathMl, ..Default::default() }).is_ok());
    }
}
