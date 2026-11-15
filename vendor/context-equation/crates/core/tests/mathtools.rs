//! mathtools' commands and environments (issue #169): each one written through the commands contextEquation
//! already reads, so the tree, the drawing and every output follow from those.

use context_equation_core::tree::{Class, FractionStyle, Node, TableKind};
use context_equation_core::{convert, Input, Options};

fn en(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn strict(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// The tree read back from the MathML is the tree.
fn round_trips(tex: &str) {
    let first = en(tex);
    assert!(!first.mathml.contains("<semantics>"), "{tex}: a fallback: {}", first.mathml);
    let back = convert(&first.mathml, &Options { input: Input::MathMl, ..Default::default() }).unwrap();
    assert_eq!(back.tree, first.tree, "{tex}: {}", first.mathml);
}

fn table(node: &Node) -> (&TableKind, &Vec<Vec<Node>>, &Vec<char>) {
    match node.single() {
        Node::Table { shape, rows, columns, .. } => (shape, rows, columns),
        Node::Delimited { parts, .. } => table(&parts[0]),
        other => panic!("not a table: {other:?}"),
    }
}

#[test]
fn small_matrices_with_delimiters() {
    for (env, open, close) in [("psmallmatrix", '(', ')'), ("bsmallmatrix", '[', ']'), ("Bsmallmatrix", '{', '}'), ("vsmallmatrix", '|', '|'), ("Vsmallmatrix", '‖', '‖')] {
        let r = strict(&format!(r"\begin{{{env}}} a & b \\ c & d \end{{{env}}}"));
        let Node::Delimited { open: Some(o), close: Some(c), parts, .. } = r.tree.single() else { panic!("{env}: {:?}", r.tree) };
        assert_eq!((*o, *c), (open, close), "{env}");
        let (kind, rows, _) = table(&parts[0]);
        assert_eq!((kind, rows.len()), (&TableKind::SmallMatrix, 2), "{env}");
    }
    // the starred forms take the alignment of their columns, as smallmatrix* does
    let r = strict(r"\begin{bsmallmatrix*}[r] -1 & b \\ c & 10 \end{bsmallmatrix*}");
    assert_eq!(table(&r.tree).2, &vec!['r', 'r']);
    assert_eq!(table(&strict(r"\begin{smallmatrix*}[l] a & b \end{smallmatrix*}").tree).2, &vec!['l', 'l']);
    round_trips(r"\begin{psmallmatrix} a & b \\ c & d \end{psmallmatrix}");
}

#[test]
fn split_fractions() {
    let r = strict(r"\frac{\splitfrac{a+b+c}{+d+e}}{f}");
    let Node::Fraction { num, .. } = r.tree.single() else { panic!("{:?}", r.tree) };
    let Node::Fraction { rule, style, num, den, .. } = num.single() else { panic!("{num:?}") };
    assert!(!rule);
    assert_eq!(*style, FractionStyle::Row);
    // each line in text style, the first ending and the second starting with a \quad
    assert!(matches!(num.single(), Node::Style { level: 1, .. }) && matches!(den.single(), Node::Style { level: 1, .. }));
    assert_eq!(r.text, "((a + b + c ¦ + d + e))/f");
    let Node::Fraction { style, .. } = strict(r"\splitdfrac{a}{b}").tree.single().clone() else { panic!() };
    assert_eq!(style, FractionStyle::Display);
}

#[test]
fn adjusted_limits_line_up() {
    let r = strict(r"\adjustlimits\lim_{n\to\infty}\sup_{p\ge n} x_p");
    let ops: Vec<&Node> = r.tree.children().iter().filter(|n| matches!(n, Node::Scripts { .. })).collect();
    assert_eq!(ops.len(), 3, "{:?}", r.tree);
    // both operators as \mathop, with the other operator and the other limit as phantoms
    for op in &ops[..2] {
        let Node::Scripts { base, sub: Some(sub), .. } = op else { panic!() };
        assert!(matches!(base.as_ref(), Node::ForcedClass { class: Class::Op, .. }), "{base:?}");
        assert!(sub.children().iter().any(|n| matches!(n, Node::Phantom { .. })), "{sub:?}");
    }
    assert_eq!(r.speech, en(r"\lim_{n\to\infty}\sup_{p\ge n} x_p").speech);
    // the limits sit on one line: the first is drawn as low as the second, which goes below the p of sup
    let svg = |tex: &str| en(tex).svg;
    assert_ne!(svg(r"\adjustlimits\lim_{n}\sup_{p}"), svg(r"\lim_{n}\sup_{p}"));
    // without operators and limits after it, it is ignored with a warning (and refused in strict mode)
    assert!(en(r"\adjustlimits x").warnings.iter().any(|w| w.rule == "E-REC-9"));
    assert!(convert(r"\adjustlimits x", &Options { strict: true, ..Default::default() }).is_err());
}

#[test]
fn move_eq_left_aboxed_and_arrow_between_lines() {
    // \MoveEqLeft[n]: a first column n em wide and the line n em to its left
    let r = strict(r"\begin{align} \MoveEqLeft a+b+c \\ &= d \end{align}");
    let (_, rows, _) = table(&r.tree);
    assert_eq!(rows[0][0], Node::Space { em: 2.0 });
    assert!(matches!(&rows[0][1], Node::Row { children } if children[0] == Node::Space { em: -2.0 }));
    let r = strict(r"\begin{align} \MoveEqLeft[3] a \\ &= d \end{align}");
    assert_eq!(table(&r.tree).1[0][0], Node::Space { em: 3.0 });
    // \Aboxed: the box in the first column, lapping into the second; the text reads it once
    let r = strict(r"\begin{align} \Aboxed{a &= b+c} \\ d &= e \end{align}");
    let (_, rows, _) = table(&r.tree);
    assert_eq!(rows[0].len(), 2);
    assert_eq!(r.text, "a = b + c ; d = e");
    assert!(r.speech.contains("boxed"), "{}", r.speech);
    // #251: the box is an ordinary atom, as \boxed is: a relation after it gets its thick space, so the
    // line is as wide as with \boxed (the SVG's width, to the thousandth of an em)
    let width = |tex: &str| strict(tex).svg.split("width=\"").nth(1).and_then(|w| w.split('"').next()).unwrap().to_string();
    assert_eq!(width(r"\begin{aligned} \Aboxed{a &= b} \approx c \end{aligned}"), width(r"\begin{aligned} \boxed{a = b} \approx c \end{aligned}"));
    assert_eq!(width(r"\begin{aligned} \Aboxed{a &= b} \approx c \end{aligned}"), width(r"\boxed{a = b} \approx c"));
    // \ArrowBetweenLines: a line of its own with ⇕ (or the symbol asked for), in the first column or,
    // starred, in the fourth
    let r = strict(r"\begin{alignat}{2} && x &= y \\ \ArrowBetweenLines && y &= x \end{alignat}");
    let (_, rows, _) = table(&r.tree);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1], vec![Node::Operator { symbol: '⇕', class: Class::Rel }]);
    let r = strict(r"\begin{alignat}{2} x &= y &&\\ \ArrowBetweenLines*[\Downarrow] y &= x && \end{alignat}");
    assert_eq!(table(&r.tree).1[1][3], Node::Operator { symbol: '⇓', class: Class::Rel });
}

#[test]
fn long_arrows_negated_arrows_and_colons() {
    let r = strict(r"A \xlongrightarrow{f} B \xLongleftarrow[g]{} C \xlongleftarrow{h} D \xLongrightarrow{} E");
    let arrows: Vec<char> = r.tree.children().iter().filter_map(|n| if let Node::Arrow { symbol, .. } = n { Some(*symbol) } else { None }).collect();
    assert_eq!(arrows, ['→', '⇐', '←', '⇒']);
    // \bigtimes is ⨉, a large operator with limits, not \big followed by "times"
    let r = strict(r"\bigtimes_{i=1}^n A_i");
    assert!(matches!(r.tree.children()[0], Node::Scripts { ref base, .. } if **base == Node::LargeOperator { symbol: '⨉', limits: None }), "{:?}", r.tree);
    // \nuparrow and \ndownarrow: the arrows struck out, as relations
    let r = strict(r"a \nuparrow b \ndownarrow c");
    assert_eq!(r.tree.children().iter().filter(|n| matches!(n, Node::ForcedClass { class: Class::Rel, .. })).count(), 2);
    assert_eq!(strict(r"a \vcentercolon b").tree, strict(r"a \vcentcolon b").tree);
    assert_eq!(strict(r"a \ordinarycolon b").tree, strict(r"a : b").tree);
    round_trips(r"A \xlongrightarrow{f} B");
}

#[test]
fn paired_delimiters_with_a_body() {
    // \DeclarePairedDelimiterX: arguments, and \delimsize as the size of the delimiters
    let set = r"\DeclarePairedDelimiterX{\set}[2]{\{}{\}}{#1 \delimsize\vert #2} ";
    assert_eq!(strict(&format!(r"{set}\set{{x}}{{y}}")).text, "{x|y}");
    let starred = strict(&format!(r"{set}\set*{{\frac12}}{{y}}"));
    let Node::Delimited { middles, .. } = starred.tree.single() else { panic!("{:?}", starred.tree) };
    assert_eq!(middles, &vec!['|']);
    let big = strict(&format!(r"{set}\set[\big]{{x}}{{y}}"));
    assert_eq!(big.tree.children().iter().filter(|n| matches!(n, Node::FixedDelimiter { size: 1, .. })).count(), 3, "{:?}", big.tree);
    // XPP: what goes before and after the delimiters
    assert_eq!(strict(r"\DeclarePairedDelimiterXPP{\Prob}[1]{P}{(}{)}{_0}{#1} \Prob{A}").text, "P(A)_0");
    // the plain form reads as before: its argument as written, between \mathopen and \mathclose
    assert_eq!(strict(r"\DeclarePairedDelimiter{\abs}{\lvert}{\rvert} \abs{-x}").text, "|−x|");
}

#[test]
fn gathered_to_one_side_multlined_and_spreadlines() {
    // lgathered: every line in the left column of aligned; rgathered: in the right one
    let r = strict(r"\begin{lgathered} a+b \\ c \\[2pt] d \end{lgathered}");
    let (kind, rows, _) = table(&r.tree);
    assert_eq!(kind, &TableKind::Aligned);
    assert!(rows.iter().all(|row| row.len() == 2 && row[0] == Node::row(vec![])), "{rows:?}");
    let r = strict(r"\begin{rgathered} a+b \\ c \end{rgathered}");
    let (kind, rows, _) = table(&r.tree);
    assert_eq!(kind, &TableKind::Aligned);
    assert!(rows.iter().all(|row| row.len() == 1));
    // a nested environment keeps its own lines
    let r = strict(r"\begin{lgathered} \begin{matrix} 1 \\ 2 \end{matrix} \\ c \end{lgathered}");
    assert_eq!(table(&r.tree).1.len(), 2);
    // multlined: the first line at the left, the last at the right, as multline
    let r = strict(r"\begin{multlined}[t][5cm] a+b \\ c+d \\ e \end{multlined}");
    let (_, rows, _) = table(&r.tree);
    assert!(matches!(rows[0][0], Node::Multicolumn { alignment: 'l', .. }) && matches!(rows[2][0], Node::Multicolumn { alignment: 'r', .. }));
    // spreadlines: the content, its space between lines read and not used
    assert_eq!(strict(r"\begin{spreadlines}{2em} \begin{gathered} a \\ b \end{gathered} \end{spreadlines}").text, strict(r"\begin{gathered} a \\ b \end{gathered}").text);
    round_trips(r"\begin{lgathered} a+b \\ c \end{lgathered}");
}

#[test]
fn cramped_commands_and_math_boxes() {
    assert_eq!(strict(r"\crampedsubstack{a \\ b}").tree, strict(r"\substack{a \\ b}").tree);
    assert_eq!(strict(r"\begin{crampedsubarray}{l} a \\ b \end{crampedsubarray}").tree, strict(r"\begin{subarray}{l} a \\ b \end{subarray}").tree);
    assert_eq!(strict(r"\cramped{x^2}").text, "x^2");
    assert!(matches!(strict(r"\cramped[\scriptstyle]{x^2}").tree.single(), Node::Style { level: 2, .. }));
    assert!(
        matches!(strict(r"\sum_{\crampedclap{i \in I}} a_i").tree.children()[0], Node::Scripts { ref sub, .. } if matches!(sub.as_deref().map(Node::single), Some(Node::Overlapped { side: 'c', .. })))
    );
    assert!(matches!(strict(r"\crampedllap{x}").tree.single(), Node::Overlapped { side: 'l', .. }));
    assert_eq!(strict(r"\mathmbox{x+y}").text, "x + y");
    // \mathmakebox[width][position]: the content in a box that wide, to the left, right or center
    let r = strict(r"\mathmakebox[3em][r]{x}");
    assert!(r.tree.children().contains(&Node::Space { em: 3.0 }), "{:?}", r.tree);
    assert!(matches!(strict(r"\mathmakebox[2em]{x}").tree.children()[1], Node::Overlapped { side: 'c', .. }));
}

#[test]
fn tag_forms_are_read_and_ignored() {
    let r = strict(r"\newtagform{brackets}[\textbf]{[}{]} \usetagform{brackets} x");
    assert_eq!(r.text, "x");
    // \refeq: the label's number without the parentheses of \eqref
    let labels = [("eq:a".to_string(), "3".to_string())].into_iter().collect();
    let r = convert(r"\refeq{eq:a} \eqref{eq:a}", &Options { labels, ..Default::default() }).unwrap();
    assert_eq!(r.text, "3(3)");
}
