//! MathML input (input/mathml.rs): presentation elements, fences, tables, scripts, decorations,
//! entities, tolerant recovery and the recognition of MathML pasted as LaTeX (L-ENT-4).

use context_equation_core::tree::{Cancel, Frame, TableKind, Variant};
use context_equation_core::{convert, Input, Locale, Node, Options};

fn options(strict: bool) -> Options {
    Options { input: Input::MathMl, strict, ..Options::default() }
}

/// Valid MathML: the same in both modes, with no warnings.
fn mml(src: &str) -> context_equation_core::Output {
    let m = if context_equation_core::input::mathml::looks_like_mathml(src) { src.to_string() } else { format!("<math>{src}</math>") };
    let strict = convert(&m, &options(true)).unwrap_or_else(|e| panic!("{m}: {e}"));
    let tolerant = convert(&m, &options(false)).unwrap();
    assert!(tolerant.warnings.is_empty(), "{m}: {:?}", tolerant.warnings);
    assert_eq!(strict.svg, tolerant.svg, "{m}");
    strict
}

fn first(src: &str) -> Node {
    mml(src).tree.children()[0].clone()
}

fn latex(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// The same equation in MathML and in LaTeX: the same speech and the same linear text.
#[test]
fn same_meaning_as_latex() {
    for (m, tex) in [
        (
            "<mi>x</mi><mo>=</mo><mfrac><mrow><mo>−</mo><mi>b</mi><mo>±</mo><msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>−</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt></mrow><mrow><mn>2</mn><mi>a</mi></mrow></mfrac>",
            r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}",
        ),
        ("<munderover><mo>∑</mo><mrow><mi>n</mi><mo>=</mo><mn>1</mn></mrow><mi>∞</mi></munderover><mfrac><mn>1</mn><msup><mi>n</mi><mn>2</mn></msup></mfrac>", r"\sum_{n=1}^{\infty} \frac{1}{n^2}"),
        ("<msubsup><mo>∫</mo><mn>0</mn><mn>1</mn></msubsup><msup><mi>x</mi><mn>2</mn></msup><mi mathvariant=\"normal\">d</mi><mi>x</mi>", r"\int_0^1 x^2 \mathrm{d}x"),
        ("<msup><mi>e</mi><mrow><mi>i</mi><mi>π</mi></mrow></msup><mo>+</mo><mn>1</mn><mo>=</mo><mn>0</mn>", r"e^{i\pi} + 1 = 0"),
        ("<mi>sin</mi><mo>&ApplyFunction;</mo><mi>θ</mi>", r"\sin\theta"),
        ("<munder><mi>lim</mi><mrow><mi>x</mi><mo>→</mo><mn>0</mn></mrow></munder><mi>x</mi>", r"\lim_{x \to 0} x"),
        ("<mroot><mn>8</mn><mn>3</mn></mroot><mo>=</mo><mn>2</mn>", r"\sqrt[3]{8} = 2"),
        ("<mi>a</mi><mo>≰</mo><mi>b</mi><mo>⇒</mo><mi>c</mi><mo>∈</mo><mi mathvariant=\"double-struck\">R</mi>", r"a \nleq b \Rightarrow c \in \mathbb{R}"),
        ("<mover><mi>v</mi><mo>→</mo></mover><mo>⋅</mo><mover><mi>x</mi><mo>^</mo></mover>", r"\vec{v} \cdot \hat{x}"),
        ("<mrow><mo>|</mo><mi>x</mi><mo>|</mo></mrow>", r"\left| x \right|"),
    ] {
        let a = mml(m);
        let b = latex(tex);
        assert_eq!(a.speech, b.speech, "{m}");
        assert_eq!(a.text, b.text, "{m}");
    }
}

#[test]
fn token_elements() {
    // one letter in italic, uppercase Greek upright, several letters a function or an upright word
    assert_eq!(first("<mi>x</mi>"), Node::Identifier { name: "x".into(), variant: Variant::Italic });
    assert_eq!(first("<mi>Γ</mi>"), Node::Identifier { name: "Γ".into(), variant: Variant::Upright });
    assert_eq!(first("<mi>sin</mi>"), Node::Function { name: "sin".into(), limits: false });
    assert_eq!(first("<mi>speed</mi>"), Node::Function { name: "speed".into(), limits: false });
    assert_eq!(first("<mi mathvariant=\"bold\">AB</mi>"), Node::Identifier { name: "AB".into(), variant: Variant::Bold });
    assert_eq!(first("<mi mathvariant=\"bold\">v</mi>"), Node::Identifier { name: "v".into(), variant: Variant::Bold });
    assert_eq!(first("<mi> x </mi>"), Node::Identifier { name: "x".into(), variant: Variant::Italic });
    assert_eq!(first("<mn>3.14</mn>"), Node::Number { value: "3.14".into(), group: false, thousands: None, variant: Variant::Upright });
    assert!(matches!(first("<mo>∑</mo>"), Node::LargeOperator { symbol: '∑', .. }));
    assert!(matches!(first("<mo>lim</mo>"), Node::Function { limits: true, .. }));
    assert_eq!(mml("<mtext>if </mtext><ms>s</ms>").text, "if\"s\"");
    assert!(matches!(first("<mspace width=\"1em\"/>"), Node::Space { em } if (em - 1.0).abs() < 1e-6));
    assert!(matches!(first("<mspace width=\"thickmathspace\"/>"), Node::Space { em } if (em - 5.0 / 18.0).abs() < 1e-6));
}

#[test]
fn numbers_follow_the_locale() {
    let pt = |m: &str| convert(&format!("<math>{m}</math>"), &Options { input: Input::MathMl, locale: Locale::PtBr, ..Options::default() }).unwrap();
    // a decimal comma in pt-BR (L-NUM-1), thousands with a dot (L-NUM-3)
    assert_eq!(pt("<mn>3,14</mn>").tree.children()[0], Node::Number { value: "3.14".into(), group: false, thousands: None, variant: Variant::Upright });
    assert_eq!(pt("<mn>1.000</mn>").tree.children()[0], Node::Number { value: "1000".into(), group: false, thousands: Some('.'), variant: Variant::Upright });
    assert_eq!(first("<mn>1,000</mn>"), Node::Number { value: "1000".into(), group: false, thousands: Some(','), variant: Variant::Upright });
    assert_eq!(pt("<mn>3.14</mn>").text, "3,14");
}

#[test]
fn operators_take_the_class_of_their_character() {
    use context_equation_core::Class;
    for (mo, class) in [
        ("+", Class::Bin),
        ("=", Class::Rel),
        ("≰", Class::Rel),
        ("⊗", Class::Bin),
        ("(", Class::Open),
        ("{", Class::Open),
        ("]", Class::Close),
        (",", Class::Punct),
        ("∞", Class::Ord),
        ("→", Class::Rel),
    ] {
        let tree = mml(&format!("<mi>a</mi><mo>{mo}</mo><mi>b</mi>")).tree;
        assert!(tree.children().iter().any(|c| matches!(c, Node::Operator { class: k, .. } if *k == class)), "{mo}: {tree:?}");
    }
    // invisible operators draw nothing
    assert_eq!(mml("<mi>a</mi><mo>&InvisibleTimes;</mo><mi>b</mi><mo>&#x2061;</mo>").tree.children().len(), 2);
}

#[test]
fn fences_stretch_and_wrap_tables() {
    assert!(matches!(first("<mrow><mo>(</mo><mfrac><mi>a</mi><mi>b</mi></mfrac><mo>)</mo></mrow>"), Node::Delimited { open: Some('('), close: Some(')'), .. }));
    // not balanced inside: ( a ) + ( b ) keeps its plain parentheses
    let t = mml("<mrow><mo>(</mo><mi>a</mi><mo>)</mo><mo>+</mo><mo>(</mo><mi>b</mi><mo>)</mo></mrow>").tree;
    assert_eq!(t.children().len(), 7, "{t:?}");
    assert!(!format!("{t:?}").contains("Delimited"), "{t:?}");
    let table = "<mtable><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr><mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable>";
    for (open, close, shape) in
        [("(", ")", TableKind::PMatrix), ("[", "]", TableKind::BMatrix), ("|", "|", TableKind::VMatrix), ("‖", "‖", TableKind::VVMatrix), ("{", "}", TableKind::BraceMatrix)]
    {
        let n = first(&format!("<mrow><mo>{open}</mo>{table}<mo>{close}</mo></mrow>"));
        assert!(matches!(n, Node::Table { shape: s, .. } if s == shape), "{open}{close}: {n:?}");
    }
    assert!(matches!(first(&format!("<mrow><mo>{{</mo>{table}</mrow>")), Node::Table { shape: TableKind::Cases, .. }));
    assert!(matches!(first(&format!("<mrow>{table}<mo>}}</mo></mrow>")), Node::Table { shape: TableKind::RCases, .. }));
    assert!(matches!(first(&format!("<mfenced open=\"[\" close=\"]\">{table}</mfenced>")), Node::Table { shape: TableKind::BMatrix, .. }));
    assert_eq!(mml("<mfenced><mi>a</mi><mi>b</mi></mfenced>").text, "(a, b)");
    assert_eq!(mml("<mfenced open=\"[\" close=\")\" separators=\";\"><mi>a</mi><mi>b</mi></mfenced>").text, "[a; b)");
}

#[test]
fn tables_with_alignment_lines_and_labels() {
    let n = first("<mtable columnalign=\"right left\" columnlines=\"solid\" rowlines=\"dashed\"><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr><mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable>");
    let Node::Table { shape: TableKind::Array, columns, verticals, horizontals, .. } = n else { panic!("{n:?}") };
    assert_eq!(columns, vec!['r', 'l']);
    assert_eq!(verticals, vec![1]);
    assert_eq!(horizontals.len(), 1);
    // the label of mlabeledtr goes to the end of the row, like \tag
    let r = mml("<mtable><mlabeledtr><mtd><mtext>(1)</mtext></mtd><mtd><mi>x</mi></mtd></mlabeledtr></mtable>");
    assert!(r.text.contains("x (1)"), "{}", r.text);
    // a row with only its label keeps it, after the same space as in a row with cells (#283)
    let r = mml("<mtable><mlabeledtr><mtd><mtext>(1)</mtext></mtd></mlabeledtr></mtable>");
    let Node::Table { rows, .. } = &r.tree.children()[0] else { panic!("{:?}", r.tree) };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 1, "{rows:?}");
    let cell = rows[0][0].children();
    assert!(matches!(cell, [Node::Space { em }, Node::Text { .. }] if *em == 2.0), "{cell:?}");
    assert_eq!(r.speech, "matrix 1 by 1: row 1: (1)");
    assert!(r.mathml.contains("<mtr><mtd><mspace width=\"2em\"/><mtext>(1)</mtext></mtd></mtr>"), "{}", r.mathml);
}

#[test]
fn scripts_limits_and_multiscripts() {
    assert!(matches!(first("<msub><mi>x</mi><mn>1</mn></msub>"), Node::Scripts { sub: Some(_), sup: None, .. }));
    assert!(matches!(first("<msubsup><mi>x</mi><mn>1</mn><mn>2</mn></msubsup>"), Node::Scripts { sub: Some(_), sup: Some(_), .. }));
    // munderover over a large operator: limits, also for an integral MathML stacks on purpose
    let Node::Scripts { base, .. } = first("<munderover><mo>∫</mo><mn>0</mn><mn>1</mn></munderover>") else { panic!() };
    assert!(matches!(*base, Node::LargeOperator { symbol: '∫', limits: Some(true) }));
    // isotope: prescripts on an empty base
    assert_eq!(mml("<mmultiscripts><mi>C</mi><mprescripts/><mn>6</mn><mn>14</mn></mmultiscripts>").text, "_6^14C");
    assert_eq!(mml("<mmultiscripts><mi>R</mi><mi>i</mi><none/><none/><mi>j</mi></mmultiscripts>").text, "R_i^j");
    // stacking that is neither an accent nor a limit
    assert!(matches!(first("<mover><mo>=</mo><mtext>def</mtext></mover>"), Node::ForcedClass { class: context_equation_core::Class::Rel, .. }));
}

#[test]
fn decorations_over_and_under() {
    assert!(matches!(first("<mover><mi>v</mi><mo>→</mo></mover>"), Node::Accent { accent: '\u{20D7}', .. }));
    assert!(matches!(first("<mover accent=\"true\"><mi>x</mi><mo>^</mo></mover>"), Node::Accent { accent: '\u{0302}', stretchy: false, .. }));
    assert!(matches!(first("<mover><mi>x</mi><mo>˙</mo></mover>"), Node::Accent { accent: '\u{0307}', .. }));
    assert!(matches!(first("<mover><mrow><mi>A</mi><mi>B</mi></mrow><mo>¯</mo></mover>"), Node::Bar { above: true, .. }));
    assert!(matches!(first("<munder><mi>x</mi><mo>_</mo></munder>"), Node::Bar { above: false, .. }));
    assert!(matches!(first("<munder><mrow><mi>A</mi><mi>B</mi></mrow><mo>→</mo></munder>"), Node::Accent { accent: '→', below: true, .. }));
    assert!(matches!(first("<mover><mrow><mi>A</mi><mi>B</mi></mrow><mo>⏜</mo></mover>"), Node::Accent { accent: '⏜', .. }));
    // a brace and its label, as MathJax writes them
    let n = first("<munder><munder><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow><mo>⏟</mo></munder><mi>n</mi></munder>");
    assert!(matches!(n, Node::Brace { above: false, label: Some(_), bracket: false, .. }), "{n:?}");
    assert!(matches!(first("<mover><mi>x</mi><mo>⎴</mo></mover>"), Node::Brace { above: true, bracket: true, .. }));
}

/// The drawing of an SVG (glyphs and what uses them), without the ids, which follow the source.
fn drawing(svg: &str) -> String {
    let id = svg.split("aria-labelledby=\"").nth(1).and_then(|s| s.split("-speech").next()).unwrap();
    svg.split("<defs>").nth(1).unwrap().replace(id, "")
}

#[test]
fn latex_enclose_meets_menclose_in_the_same_tree() {
    for (notation, latex_notation) in [
        ("box", "box"),
        ("roundedbox", "roundedbox"),
        ("circle", "circle"),
        ("updiagonalstrike downdiagonalstrike", "updiagonalstrike,downdiagonalstrike"),
        ("horizontalstrike", "horizontalstrike"),
        ("downdiagonalstrike", "downdiagonalstrike"),
        ("top bottom", "top bottom"),
        ("actuarial", "actuarial"),
        ("radical", "radical"),
    ] {
        let a = mml(&format!("<menclose notation=\"{notation}\"><mi>x</mi><mo>+</mo><mn>1</mn></menclose>"));
        let b = latex(&format!(r"\enclose{{{latex_notation}}}{{x+1}}"));
        assert_eq!(a.tree, b.tree, "{notation}");
        assert_eq!(drawing(&a.svg), drawing(&b.svg), "{notation}: the same drawing");
    }
    // mathcolor colors the notation and the content, as MathML's attribute on menclose
    let a = mml("<menclose notation=\"roundedbox\" mathcolor=\"red\"><mi>a</mi></menclose>");
    let b = latex(r#"\enclose{roundedbox}[mathcolor="red"]{a}"#);
    assert_eq!(a.tree, b.tree);
    assert!(matches!(b.tree.single(), Node::Colored { color, base } if color == "red" && matches!(base.as_ref(), Node::Framed { .. })), "{:?}", b.tree);
}

#[test]
fn enclose_style_and_wrappers() {
    assert!(matches!(first("<menclose notation=\"box\"><mi>x</mi></menclose>"), Node::Framed { shape: Frame::Rectangle, .. }));
    assert!(matches!(first("<menclose notation=\"circle\"><mi>x</mi></menclose>"), Node::Framed { shape: Frame::Circle, .. }));
    assert!(matches!(first("<menclose notation=\"updiagonalstrike downdiagonalstrike\"><mi>x</mi></menclose>"), Node::Canceled { shape: Cancel::X, .. }));
    assert!(matches!(first("<menclose notation=\"radical\"><mi>x</mi></menclose>"), Node::Root { .. }));
    assert!(matches!(first("<mstyle mathcolor=\"red\"><mi>x</mi></mstyle>"), Node::Colored { .. }));
    assert!(matches!(first("<mi mathcolor=\"#00f\">x</mi>"), Node::Colored { .. }));
    assert!(matches!(first("<mstyle mathbackground=\"yellow\"><mi>x</mi></mstyle>"), Node::Framed { shape: Frame::Fill, .. }));
    assert!(matches!(first("<mstyle displaystyle=\"true\"><mi>x</mi></mstyle>"), Node::Style { level: 0, .. }));
    assert!(matches!(first("<mphantom><mi>x</mi></mphantom>"), Node::Phantom { .. }));
    // mathvariant on mstyle reaches the letters inside
    assert!(matches!(first("<mstyle mathvariant=\"bold\"><mi>x</mi></mstyle>"), Node::Identifier { variant: Variant::Bold, .. }));
    // semantics: the presentation, not the annotation; maction: the selected child
    assert_eq!(mml("<semantics><mi>x</mi><annotation encoding=\"application/x-tex\">y</annotation></semantics>").text, "x");
    assert_eq!(mml("<maction actiontype=\"toggle\" selection=\"2\"><mi>a</mi><mi>b</mi></maction>").text, "b");
}

#[test]
fn namespaces_prolog_and_entities() {
    let r = mml("<?xml version=\"1.0\"?><!-- from Word --><m:math xmlns:m=\"http://www.w3.org/1998/Math/MathML\"><m:mi>x</m:mi><m:mo>&le;</m:mo><m:mn>1</m:mn></m:math>");
    assert_eq!(r.text, "x ≤ 1");
    assert_eq!(mml("<mi>&alpha;</mi><mo>&times;</mo><mi>&beta;</mi><mo>&lt;</mo><mi>&infin;</mi>").text, "α × β < ∞");
    assert_eq!(mml("<mi>&#x3B1;</mi><mo>&#43;</mo><mi>b</mi>").text, "α + b");
    // the source keeps its notation in the SVG metadata
    assert!(r.svg.contains(r#"<contextEquation:source type="mathml">"#));
}

#[test]
fn l_ent_4_mathml_pasted_as_latex() {
    let r = convert("<math><mfrac><mn>1</mn><mn>2</mn></mfrac></math>", &Options::default()).unwrap();
    assert_eq!(r.speech, "1 over 2");
    assert!(r.applied.iter().any(|a| a.rule == "L-ENT-4"));
    assert!(r.svg.contains(r#"type="mathml""#));
    // not MathML: LaTeX that merely contains "<"
    assert!(convert(r"a < b", &Options::default()).unwrap().applied.iter().all(|a| a.rule != "L-ENT-4"));
}

#[test]
fn e_rec_10_unknown_elements_and_entities() {
    let src = "<math><mi>x</mi><mo>&foo;</mo><mblah><mi>y</mi></mblah></math>";
    let r = convert(src, &options(false)).unwrap();
    assert_eq!(r.text, "xy");
    let rules: Vec<_> = r.warnings.iter().map(|a| a.rule).collect();
    assert_eq!(rules, vec!["E-REC-10", "E-REC-10"]);
    assert!(convert(src, &options(true)).unwrap_err().to_string().contains("unknown entity"));
    // content MathML outside the subset read is named as such
    let e = convert("<math><apply><curl/><ci>x</ci></apply></math>", &options(true)).unwrap_err();
    assert!(e.to_string().contains("content MathML <curl>"), "{e}");
    // a missing child of mfrac is left empty
    let r = convert("<math><mfrac><mn>1</mn></mfrac></math>", &options(false)).unwrap();
    assert_eq!(r.warnings[0].rule, "E-REC-10");
}

#[test]
fn invalid_xml_and_deep_nesting_are_errors() {
    for src in ["<math><mi>x</mi><mo>+", "<math><mi>x</mo></math>", "<math><mi>x &</mi></math>"] {
        assert!(convert(src, &options(false)).is_err(), "{src}");
    }
    let deep = format!("<math>{}<mi>x</mi>{}</math>", "<mrow>".repeat(200), "</mrow>".repeat(200));
    assert!(convert(&deep, &options(false)).unwrap_err().to_string().contains("nesting"));
}

/// Found by feeding MathJax's MathML to contextEquation next to MathCAT.
#[test]
fn mathjax_mathml_conventions() {
    // consecutive relations in one <mo>
    let r = mml("<mo>≤≥≠</mo>");
    assert_eq!(r.tree.children().len(), 3, "{:?}", r.tree);
    assert_eq!(r.text, latex(r"\le \ge \ne").text);
    // the no-break space of <mtext> is content
    assert_eq!(mml("<mi>p</mi><mtext>&#xA0;and&#xA0;</mtext><mi>q</mi>").text.split_whitespace().collect::<Vec<_>>(), ["p", "and", "q"]);
    // a name followed by ApplyFunction is a function, even one contextEquation does not know
    assert!(matches!(first("<mi>rank</mi><mo>&#x2061;</mo><mi>M</mi>"), Node::Function { .. }));
    // cases with column alignment: still cases
    let n = first(r#"<mrow><mo>{</mo><mtable columnalign="left left"><mtr><mtd><mn>1</mn></mtd><mtd><mi>x</mi></mtd></mtr></mtable></mrow>"#);
    assert!(matches!(n, Node::Table { shape: TableKind::Cases, .. }), "{n:?}");
}

#[test]
fn mathjax_norm_bars_are_a_norm() {
    // MathJax writes \| as the parallel sign ∥ (U+2225); between two of them is a norm
    let n = first(r#"<mo data-mjx-texclass="ORD">&#x2225;</mo><mi>x</mi><mo data-mjx-texclass="ORD">&#x2225;</mo>"#);
    assert!(matches!(n, Node::Delimited { open: Some('‖'), close: Some('‖'), .. }), "{n:?}");
    assert_eq!(mml("<mi>a</mi><mo>∥</mo><mi>b</mi>").text, latex(r"a \parallel b").text);
}

/// Content MathML (input/mathml/content.rs): the same tree as the LaTeX an author would write.
#[test]
fn content_mathml_is_the_same_tree_as_latex() {
    let ci = |x: &str| format!("<ci>{x}</ci>");
    let cn = |x: &str| format!("<cn>{x}</cn>");
    let apply = |op: &str, args: &[String]| format!("<apply><{op}/>{}</apply>", args.concat());
    let x2 = apply("power", &[ci("x"), cn("2")]);
    let a_plus_b = apply("plus", &[ci("a"), ci("b")]);
    let sin_x = apply("sin", &[ci("x")]);
    for (m, tex) in [
        (apply("plus", &[x2.clone(), cn("1")]), r"x^2 + 1"),
        (apply("times", &[a_plus_b.clone(), ci("c")]), r"(a + b) \cdot c"),
        (apply("power", &[a_plus_b.clone(), cn("2")]), r"(a + b)^2"),
        (apply("eq", &[apply("plus", &[apply("times", &[cn("2"), ci("x")]), cn("3")]), cn("7")]), r"2x + 3 = 7"),
        (apply("minus", &[ci("c"), a_plus_b.clone()]), r"c - (a + b)"),
        (apply("minus", &[a_plus_b.clone(), ci("c")]), r"a + b - c"),
        (apply("plus", &[apply("minus", &[ci("x")]), ci("y")]), r"-x + y"),
        (apply("plus", &[ci("a"), apply("minus", &[ci("b")])]), r"a + (-b)"),
        (apply("times", &[apply("divide", &[cn("1"), cn("2")]), ci("x")]), r"\frac{1}{2} x"),
        (apply("times", &[cn("2"), cn("3")]), r"2 \cdot 3"),
        (apply("times", &[ci("π"), apply("power", &[ci("r"), cn("2")])]), r"\pi r^2"),
        (apply("divide", &[a_plus_b.clone(), cn("2")]), r"\frac{a + b}{2}"),
        (apply("root", &[ci("x")]), r"\sqrt{x}"),
        ("<apply><root/><degree><cn>3</cn></degree><ci>x</ci></apply>".into(), r"\sqrt[3]{x}"),
        (apply("abs", &[ci("x")]), r"|x|"),
        (apply("factorial", &[ci("n")]), r"n!"),
        (apply("factorial", &[apply("plus", &[ci("n"), cn("1")])]), r"(n + 1)!"),
        (sin_x.clone(), r"\sin x"),
        (apply("cos", &[apply("plus", &[ci("a"), ci("b")])]), r"\cos(a + b)"),
        (apply("power", &[sin_x.clone(), cn("2")]), r"\sin^2 x"),
        ("<apply><log/><logbase><cn>2</cn></logbase><ci>x</ci></apply>".into(), r"\log_2 x"),
        (apply("ln", &[ci("x")]), r"\ln x"),
        (apply("exp", &[ci("x")]), r"e^{x}"),
        ("<apply><ci>f</ci><ci>x</ci></apply>".into(), r"f(x)"),
        ("<apply><int/><bvar><ci>x</ci></bvar><lowlimit><cn>0</cn></lowlimit><uplimit><cn>1</cn></uplimit>".to_string() + &x2 + "</apply>", r"\int_0^1 x^2 \, dx"),
        ("<apply><int/><bvar><ci>x</ci></bvar><ci>x</ci></apply>".into(), r"\int x \, dx"),
        (
            "<apply><sum/><bvar><ci>i</ci></bvar><lowlimit><cn>1</cn></lowlimit><uplimit><ci>n</ci></uplimit><apply><power/><ci>i</ci><cn>2</cn></apply></apply>".into(),
            r"\sum_{i=1}^{n} i^2",
        ),
        ("<apply><diff/><bvar><ci>x</ci></bvar><ci>y</ci></apply>".into(), r"\frac{dy}{dx}"),
        ("<apply><diff/><bvar><ci>x</ci><degree><cn>2</cn></degree></bvar><ci>y</ci></apply>".into(), r"\frac{d^2 y}{dx^2}"),
        ("<apply><diff/><bvar><ci>x</ci></bvar>".to_string() + &apply("plus", &[x2.clone(), cn("1")]) + "</apply>", r"\frac{d}{dx}(x^2 + 1)"),
        ("<apply><partialdiff/><bvar><ci>x</ci></bvar><ci>f</ci></apply>".into(), r"\frac{\partial f}{\partial x}"),
        (
            "<apply><limit/><bvar><ci>x</ci></bvar><lowlimit><cn>0</cn></lowlimit>".to_string() + &apply("divide", &[sin_x.clone(), ci("x")]) + "</apply>",
            r"\lim_{x \to 0} \frac{\sin x}{x}",
        ),
        (apply("in", &[ci("x"), apply("union", &[ci("A"), ci("B")])]), r"x \in A \cup B"),
        (apply("intersect", &[ci("A"), ci("B")]), r"A \cap B"),
        (apply("leq", &[ci("x"), cn("3")]), r"x \leq 3"),
        (apply("neq", &[ci("x"), cn("0")]), r"x \neq 0"),
        (apply("approx", &[ci("π"), cn("3.14")]), r"\pi \approx 3.14"),
        (apply("lt", &[cn("0"), ci("x"), cn("1")]), r"0 < x < 1"),
        ("<interval closure=\"closed-open\"><cn>0</cn><cn>1</cn></interval>".into(), r"[0, 1)"),
        ("<interval><ci>a</ci><ci>b</ci></interval>".into(), r"[a, b]"),
        ("<interval closure=\"open\"><cn>0</cn><infinity/></interval>".into(), r"(0, \infty)"),
        ("<set><cn>1</cn><cn>2</cn><cn>3</cn></set>".into(), r"\{1, 2, 3\}"),
        ("<set><bvar><ci>x</ci></bvar><condition><apply><gt/><ci>x</ci><cn>0</cn></apply></condition></set>".into(), r"\{x \mid x > 0\}"),
        ("<apply><in/><ci>x</ci><reals/></apply>".into(), r"x \in \mathbb{R}"),
        ("<cn type=\"rational\">1<sep/>2</cn>".into(), r"\frac{1}{2}"),
        ("<cn type=\"e-notation\">1.5<sep/>3</cn>".into(), r"1.5 \times 10^{3}"),
        (apply("power", &[cn("-3"), cn("2")]), r"(-3)^2"),
        (apply("plus", &[ci("<msub><mi>x</mi><mn>1</mn></msub>"), ci("<msub><mi>x</mi><mn>2</mn></msub>")]), r"x_1 + x_2"),
        ("<apply><csymbol cd=\"arith1\">plus</csymbol><ci>a</ci><cn>1</cn></apply>".into(), r"a + 1"),
    ] {
        let a = mml(&m);
        let b = latex(tex);
        assert_eq!(a.tree, b.tree, "{m}\n{tex}");
        assert_eq!(a.speech, b.speech, "{m}");
    }
}

#[test]
fn content_mathml_follows_the_locale() {
    let pt_mml = |m: &str| convert(&format!("<math>{m}</math>"), &Options { input: Input::MathMl, locale: Locale::PtBr, strict: true, ..Options::default() }).unwrap();
    let pt_tex = |t: &str| convert(t, &Options { locale: Locale::PtBr, strict: true, ..Options::default() }).unwrap();
    // the sine is "sen" in pt-BR, and 3.14 is written 3,14, as the LaTeX the author would type there
    let m = pt_mml("<apply><plus/><apply><sin/><ci>x</ci></apply><cn>3.14</cn></apply>");
    let t = pt_tex(r"\sen x + 3,14");
    assert_eq!(m.tree, t.tree);
    assert_eq!(m.speech, t.speech);
    assert_eq!(m.text, "sen x + 3,14");
}

#[test]
fn semantics_prefers_the_presentation() {
    // presentation first, content annotation: the presentation
    assert_eq!(mml(r#"<semantics><mi>x</mi><annotation-xml encoding="MathML-Content"><ci>y</ci></annotation-xml></semantics>"#).text, "x");
    // content first, presentation annotation: the presentation
    assert_eq!(mml(r#"<semantics><ci>y</ci><annotation-xml encoding="MathML-Presentation"><mi>x</mi></annotation-xml></semantics>"#).text, "x");
    // content only: the content, with a TeX annotation ignored
    assert_eq!(mml(r#"<semantics><apply><plus/><ci>a</ci><cn>1</cn></apply><annotation encoding="application/x-tex">b</annotation></semantics>"#).text, "a + 1");
    // nothing but a content annotation
    assert_eq!(mml(r#"<semantics><annotation-xml encoding="MathML-Content"><apply><minus/><ci>a</ci></apply></annotation-xml></semantics>"#).text, "−a");
}

#[test]
fn e_rec_10_content_mathml_outside_the_subset() {
    let r = convert("<math><apply><plus/><ci>a</ci><apply><curl/><ci>F</ci></apply></apply></math>", &options(false)).unwrap();
    assert_eq!(r.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), ["E-REC-10"]);
    assert!(r.warnings[0].description.contains("content MathML <curl>"), "{:?}", r.warnings);
    assert_eq!(r.text, "a + F");
    // a divide with one operand: the missing one left empty
    let r = convert("<math><apply><divide/><cn>1</cn></apply></math>", &options(false)).unwrap();
    assert_eq!(r.warnings[0].rule, "E-REC-10");
    assert!(convert("<math><apply><divide/><cn>1</cn></apply></math>", &options(true)).is_err());
    assert!(convert("<math><bind><csymbol>forall</csymbol></bind></math>", &options(true)).unwrap_err().to_string().contains("content MathML <bind>"));
}

/// What MathML Core writes, and contextEquation's own MathML output (spec/mathml.md), read to the tree it came from.
#[test]
fn mathml_core_conventions() {
    // Unicode's mathematical letters are a letter in a variant
    assert_eq!(first("<mi>𝐱</mi>"), Node::Identifier { name: "x".into(), variant: Variant::Bold });
    assert_eq!(first("<mi>ℝ</mi>"), Node::Identifier { name: "R".into(), variant: Variant::DoubleStruck });
    assert_eq!(first("<mi>𝛤</mi>"), Node::Identifier { name: "Γ".into(), variant: Variant::Italic });
    // stretchy="false" keeps a parenthesis plain; a stretchy one inside is \middle; an empty fence is \left.
    assert!(matches!(first(r#"<mrow><mo stretchy="false">(</mo><mi>x</mi><mo stretchy="false">)</mo></mrow>"#), Node::Operator { symbol: '(', .. }));
    let n = first(r#"<mrow><mo>⟨</mo><mi>a</mi><mo stretchy="true">|</mo><mi>b</mi><mo>⟩</mo></mrow>"#);
    assert!(matches!(&n, Node::Delimited { parts, middles, .. } if parts.len() == 2 && middles == &['|']), "{n:?}");
    let n = first(r#"<mrow><mo fence="true" form="prefix"></mo><mi>x</mi><mo>|</mo></mrow>"#);
    assert!(matches!(n, Node::Delimited { open: None, close: Some('|'), .. }), "{n:?}");
    // an <mrow> around one element is a group, as TeX's braces
    assert!(matches!(first("<msup><mrow><mi>x</mi></mrow><mn>2</mn></msup>"), Node::Scripts { base, .. } if matches!(*base, Node::Row { .. })));
    // the same minsize and maxsize: \big; lspace and rspace: a class
    assert!(matches!(first(r#"<mo minsize="1.8em" maxsize="1.8em">(</mo>"#), Node::FixedDelimiter { size: 2, .. }));
    assert!(matches!(first(r#"<mo lspace="0em" rspace="0em">,</mo>"#), Node::Operator { class: context_equation_core::Class::Ord, .. }));
    // ∑ under munderover is left to the style (movablelimits); at the side it is \nolimits
    assert!(matches!(first("<munderover><mo>∑</mo><mi>i</mi><mi>n</mi></munderover>"), Node::Scripts { base, .. } if matches!(*base, Node::LargeOperator { limits: None, .. })));
    assert!(
        matches!(first(r#"<munder><mo movablelimits="false">∑</mo><mi>i</mi></munder>"#), Node::Scripts { base, .. } if matches!(*base, Node::LargeOperator { limits: Some(true), .. }))
    );
    assert!(matches!(first("<msub><mo>∑</mo><mi>i</mi></msub>"), Node::Scripts { base, .. } if matches!(*base, Node::LargeOperator { limits: Some(false), .. })));
    // mpadded: \hphantom, \smash, \mathrlap; mtd columnspan: \multicolumn; mfrac's displaystyle: \dfrac
    assert!(matches!(first(r#"<mpadded height="0em" depth="0em"><mphantom><mi>x</mi></mphantom></mpadded>"#), Node::Phantom { width: true, height: false, .. }));
    assert!(matches!(first(r#"<mpadded height="0em" depth="0em"><mi>x</mi></mpadded>"#), Node::Smashed { height: true, depth: true, .. }));
    assert!(matches!(first(r#"<mpadded width="0em"><mi>x</mi></mpadded>"#), Node::Overlapped { side: 'r', .. }));
    let n = first(r#"<mtable><mtr><mtd columnspan="2"><mi>x</mi></mtd></mtr></mtable>"#);
    assert!(matches!(&n, Node::Table { rows, .. } if matches!(rows[0][0], Node::Multicolumn { columns: 2, .. })), "{n:?}");
    assert!(matches!(first(r#"<mfrac displaystyle="true"><mn>1</mn><mn>2</mn></mfrac>"#), Node::Fraction { style: context_equation_core::tree::FractionStyle::Display, .. }));
    // no-break spaces keep the spaces at the ends of the text
    assert_eq!(first("<mtext>\u{A0}if\u{A0}</mtext>"), Node::Text { parts: vec![context_equation_core::tree::TextPart::Text(" if ".into())] });
    // a stretchy arrow with text over it: \xrightarrow
    assert!(matches!(first(r#"<mover><mo stretchy="true">→</mo><mi>f</mi></mover>"#), Node::Arrow { symbol: '→', .. }));
}
