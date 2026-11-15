//! MathML input (input/mathml.rs and input/mathml/content.rs), the corners the element tests leave:
//! groupings for an intent, empty and stray elements, operator spacing and sizes, the accents of
//! every mark, fences around what is not a table, lengths without a unit, and the content MathML
//! spelled as LaTeX for each construction and its recovery.

use context_equation_core::tree::{Cancel, TableKind, TextPart, Variant};
use context_equation_core::{convert, Class, Input, Locale, Node, Options};

fn options(strict: bool) -> Options {
    Options { input: Input::MathMl, strict, ..Options::default() }
}

fn wrap(src: &str) -> String {
    if context_equation_core::input::mathml::looks_like_mathml(src) {
        src.to_string()
    } else {
        format!("<math>{src}</math>")
    }
}

/// Valid MathML: the same in both modes, with no warnings.
fn mml(src: &str) -> context_equation_core::Output {
    let m = wrap(src);
    let strict = convert(&m, &options(true)).unwrap_or_else(|e| panic!("{m}: {e}"));
    let tolerant = convert(&m, &options(false)).unwrap();
    assert!(tolerant.warnings.is_empty(), "{m}: {:?}", tolerant.warnings);
    assert_eq!(strict.svg, tolerant.svg, "{m}");
    strict
}

/// MathML read with a recovery: an error in strict mode, the given warnings in tolerant mode.
fn recovered(src: &str, rules: &[&str]) -> context_equation_core::Output {
    let m = wrap(src);
    assert!(convert(&m, &options(true)).is_err(), "{m}: strict mode refuses it");
    let r = convert(&m, &options(false)).unwrap_or_else(|e| panic!("{m}: {e}"));
    assert_eq!(r.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), rules, "{m}: {:?}", r.warnings);
    r
}

fn first(src: &str) -> Node {
    mml(src).tree.children()[0].clone()
}

fn latex(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn text(t: &str) -> Node {
    Node::Text { parts: vec![TextPart::Text(t.into())] }
}

#[test]
fn a_grouping_for_an_intent_gives_its_children_to_the_row() {
    // an <mrow arg> or an interval's <mrow intent> only names its content: no group of its own
    let plain = mml("<mi>x</mi><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow>").tree;
    assert_eq!(plain.children().len(), 2, "{plain:?}");
    for grouping in [r#"<mrow arg="a">"#, r#"<mrow intent="open-interval($a,$b)">"#] {
        let tree = mml(&format!("<mi>x</mi>{grouping}<mi>a</mi><mo>+</mo><mi>b</mi></mrow>")).tree;
        assert_eq!(tree.children().len(), 4, "{grouping}: {tree:?}");
    }
}

#[test]
fn empty_and_stray_elements() {
    // maction and mmultiscripts with nothing in them, semantics with only a TeX annotation: nothing
    for src in [r#"<maction actiontype="toggle"/>"#, "<mmultiscripts/>", r#"<semantics><annotation encoding="application/x-tex">x</annotation></semantics>"#] {
        let r = mml(&format!("<mi>a</mi>{src}<mi>b</mi>"));
        assert_eq!(r.text, "ab", "{src}");
        assert_eq!(r.tree.children().len(), 2, "{src}");
    }
    // a row outside a table: its cells side by side
    assert_eq!(mml("<mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr>").text, "ab");
    // cells without rows in a table: each a row of its own
    let n = first("<mtable><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtable>");
    let Node::Table { rows, .. } = &n else { panic!("{n:?}") };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1], vec![Node::Identifier { name: "b".into(), variant: Variant::Italic }]);
}

#[test]
fn slashed_tokens_numbers_that_are_words_and_unknown_variants() {
    // ∂̸: the Feynman slash on a symbol that is no letter
    let n = first("<mi>∂\u{0338}</mi>");
    assert!(matches!(&n, Node::Canceled { shape: Cancel::Slash, base } if matches!(base.as_ref(), Node::Operator { symbol: '∂', .. })), "{n:?}");
    // an <mn> that is no number (Roman numerals, nothing at all) is text
    assert_eq!(first("<mn>XIV</mn>"), text("XIV"));
    assert_eq!(mml("<mi>a</mi><mn></mn>").tree.children()[1], text(""));
    // a thousands separator and a decimal part: 1,000.5
    assert_eq!(first("<mn>1,000.5</mn>"), Node::Number { value: "1000.5".into(), group: false, thousands: Some(','), variant: Variant::Upright });
    // a mathvariant contextEquation does not draw (MathML's Arabic "initial") leaves the letter as it was
    assert_eq!(first(r#"<mi mathvariant="initial">x</mi>"#), Node::Identifier { name: "x".into(), variant: Variant::Italic });
    // a mathematical character that is no letter of any alphabet contextEquation knows (bold digamma): as is
    assert_eq!(first("<mi>\u{1D7CA}</mi>"), Node::Identifier { name: "\u{1D7CA}".into(), variant: Variant::Upright });
    // letters of two alphabets in one <mi> (bold x, italic y) share no variant: the characters stay
    assert!(!matches!(first("<mi>\u{1D431}\u{1D466}</mi>"), Node::Identifier { ref name, .. } if name == "xy"));
}

#[test]
fn ascii_operators_are_their_math_characters() {
    // what a keyboard gives: - is the minus sign, * the asterisk operator, ' the prime
    for (mo, symbol, class) in [("-", '−', Class::Bin), ("*", '∗', Class::Bin), ("'", '′', Class::Ord)] {
        let tree = mml(&format!("<mi>a</mi><mo>{mo}</mo><mi>b</mi>")).tree;
        assert_eq!(tree.children()[1], Node::Operator { symbol, class }, "{mo}");
    }
    assert_eq!(mml("<mi>a</mi><mo>-</mo><mi>b</mi>").tree, latex("a-b").tree);
}

#[test]
fn spacing_and_size_written_on_an_mo() {
    // lspace and rspace of a relation (5/18 em each side) make one; others leave the character's class
    assert_eq!(first(r#"<mo lspace="0.2778em" rspace="0.2778em">+</mo>"#), Node::Operator { symbol: '+', class: Class::Rel });
    assert_eq!(first(r#"<mo lspace="1em" rspace="1em">+</mo>"#), Node::Operator { symbol: '+', class: Class::Bin });
    // a delimiter of fixed size keeps its size under the class its spacing gives
    let n = first(r#"<mo lspace="0em" rspace="0em" minsize="1.2em" maxsize="1.2em">(</mo>"#);
    assert!(matches!(n, Node::FixedDelimiter { symbol: '(', class: Class::Ord, size: 1 }), "{n:?}");
    // a range of sizes is no fixed size
    assert_eq!(first(r#"<mo minsize="1em" maxsize="2em">(</mo>"#), Node::Operator { symbol: '(', class: Class::Open });
}

#[test]
fn mpadded_that_changes_nothing_context_equation_draws() {
    let x = Node::Identifier { name: "x".into(), variant: Variant::Italic };
    assert_eq!(first(r#"<mpadded lspace="1em"><mi>x</mi></mpadded>"#), x);
    assert_eq!(first(r#"<mpadded voffset="0em"><mi>x</mi></mpadded>"#), x);
    assert!(matches!(first(r#"<mpadded voffset="0.5em"><mi>x</mi></mpadded>"#), Node::Raised { em, .. } if (em - 0.5).abs() < 1e-6));
    // a length without a unit: width="0" is no width, a lap
    assert!(matches!(first(r#"<mpadded width="0"><mi>x</mi></mpadded>"#), Node::Overlapped { side: 'r', .. }));
    assert!(matches!(first(r#"<mspace width="2"/>"#), Node::Space { em } if (em - 2.0).abs() < 1e-6));
}

#[test]
fn every_accent_mark_over_and_under() {
    for (mark, accent, below) in [
        ("ˇ", '\u{030C}', false),
        ("´", '\u{0301}', false),
        ("`", '\u{0300}', false),
        ("˘", '\u{0306}', false),
        ("˚", '\u{030A}', false),
        ("⃛", '\u{20DB}', false),
        ("¨", '\u{0308}', false),
    ] {
        let n = first(&format!("<mover><mi>x</mi><mo>{mark}</mo></mover>"));
        assert!(matches!(n, Node::Accent { accent: a, stretchy: false, below: b, .. } if a == accent && b == below), "{mark}: {n:?}");
    }
    // a left arrow over or under, a harpoon-free \overleftarrow and \underleftarrow
    assert!(matches!(first("<mover><mi>x</mi><mo>←</mo></mover>"), Node::Accent { accent: '←', stretchy: true, below: false, .. }));
    assert!(matches!(first("<munder><mi>x</mi><mo>⟵</mo></munder>"), Node::Accent { accent: '←', stretchy: true, below: true, .. }));
    assert_eq!(mml("<mover><mrow><mi>A</mi><mi>B</mi></mrow><mo>←</mo></mover>").tree, latex(r"\overleftarrow{AB}").tree);
}

#[test]
fn a_brace_with_marks_both_over_and_under_is_stacked() {
    // a brace below, then something over and under it: no label, a stack around the brace
    let n = first("<munderover><munder><mrow><mi>a</mi><mo>+</mo><mi>b</mi></mrow><mo>⏟</mo></munder><mi>n</mi><mi>m</mi></munderover>");
    let Node::Stacked { base, above: Some(_), below: Some(_) } = &n else { panic!("{n:?}") };
    assert!(matches!(base.as_ref(), Node::Brace { above: false, label: None, .. }), "{base:?}");
}

#[test]
fn a_lone_brace_with_no_table_is_a_plain_brace() {
    // { or } on one side of something that is no table: no cases, the characters as they are
    for src in ["<mrow><mo>{</mo><mi>x</mi></mrow>", "<mrow><mi>x</mi><mo>}</mo></mrow>"] {
        let n = mml(src).tree;
        assert!(!format!("{n:?}").contains("Table"), "{src}: {n:?}");
        assert_eq!(n.children().len(), 2, "{src}: {n:?}");
    }
    // a table between [ and ) pairs to no matrix: an array between stretched delimiters
    let table = "<mtable><mtr><mtd><mi>a</mi></mtd></mtr><mtr><mtd><mi>b</mi></mtd></mtr></mtable>";
    let n = first(&format!("<mrow><mo>[</mo>{table}<mo>)</mo></mrow>"));
    assert!(
        matches!(&n, Node::Delimited { open: Some('['), close: Some(')'), parts, .. } if matches!(parts[0].children(), [Node::Table { shape: TableKind::Matrix, .. }])),
        "{n:?}"
    );
}

#[test]
fn mfenced_separators() {
    // no separators: the children side by side; fewer separators than gaps: the last one repeats
    assert_eq!(mml(r#"<mfenced separators=""><mi>a</mi><mi>b</mi></mfenced>"#).text, "(ab)");
    assert_eq!(mml(r#"<mfenced separators=";"><mi>a</mi><mi>b</mi><mi>c</mi></mfenced>"#).text, "(a; b; c)");
}

/// Content MathML and the LaTeX it is spelled as: the same tree and the same speech.
fn same_as_latex(pairs: &[(&str, &str)]) {
    for (m, tex) in pairs {
        let a = mml(m);
        let b = latex(tex);
        assert_eq!(a.tree, b.tree, "{m}\n{tex}");
        assert_eq!(a.speech, b.speech, "{m}");
    }
}

#[test]
fn content_presentation_inside_ci() {
    same_as_latex(&[
        ("<ci><mi>speed</mi></ci>", r"\mathrm{speed}"),
        ("<ci><msup><mi>x</mi><mn>2</mn></msup></ci>", r"x^{2}"),
        ("<ci><msubsup><mi>x</mi><mn>1</mn><mn>2</mn></msubsup></ci>", r"x_{1}^{2}"),
        ("<ci><msub><mi>max</mi><mn>1</mn></msub></ci>", r"{\mathrm{max}}_{1}"),
        ("<ci><msub><mi>α</mi><mn>1</mn></msub></ci>", r"α_{1}"),
        ("<ci><mover><mi>x</mi><mo>^</mo></mover></ci>", r"\hat{x}"),
        ("<ci><mover><mi>x</mi><mo>¯</mo></mover></ci>", r"\bar{x}"),
        ("<ci><mover><mi>v</mi><mo>→</mo></mover></ci>", r"\vec{v}"),
        ("<ci><mover><mi>x</mi><mo>˙</mo></mover></ci>", r"\dot{x}"),
        ("<ci><mover><mi>x</mi><mo>~</mo></mover></ci>", r"\tilde{x}"),
        ("<ci><mtext>total</mtext></ci>", r"\text{total}"),
        ("<ci><mi>x</mi><mo>′</mo></ci>", r"x′"),
    ]);
}

#[test]
fn content_presentation_inside_ci_outside_the_subset() {
    // what a <ci> may not hold is written as its text, upright (E-REC-10)
    for (src, tex) in [
        ("<ci><mfrac><mi>a</mi><mi>b</mi></mfrac></ci>", r"\mathrm{ab}"),
        ("<ci><msub><mi>x</mi></msub></ci>", r"\mathrm{x}"),
        ("<ci><mover><mi>x</mi><mo>⌢</mo></mover></ci>", r"\mathrm{x⌢}"),
        ("<ci><mover><mi>x</mi></mover></ci>", r"\mathrm{x}"),
    ] {
        let r = recovered(src, &["E-REC-10"]);
        assert_eq!(r.tree, latex(tex).tree, "{src}");
    }
}

#[test]
fn content_numbers_of_every_type() {
    same_as_latex(&[
        (r#"<cn type="complex-cartesian">3<sep/>4</cn>"#, r"3 + 4 i"),
        (r#"<cn type="complex-cartesian">3<sep/>-4</cn>"#, r"3 - 4 i"),
        (r#"<cn type="rational">-1<sep/>2</cn>"#, r"-\frac{1}{2}"),
        (r#"<cn type="constant">π</cn>"#, r"π"),
        ("<cn>+5</cn>", "5"),
        // a number in an element of its own inside the <cn>
        ("<cn><mn>42</mn></cn>", "42"),
    ]);
    // parts with no type that has parts: read together (E-REC-10)
    assert_eq!(recovered("<cn>1<sep/>2</cn>", &["E-REC-10"]).tree, latex("12").tree);
    // no number: written as text
    assert_eq!(recovered("<cn>abc</cn>", &["E-REC-10"]).tree, latex(r"\mathrm{abc}").tree);
    assert_eq!(recovered("<cn>.</cn>", &["E-REC-10"]).tree, latex(r"\mathrm{.}").tree);
}

#[test]
fn content_identifiers_escape_what_latex_would_read() {
    same_as_latex(&[
        ("<ci></ci>", "{}"),
        ("<csymbol>speed</csymbol>", r"\mathit{speed}"),
        ("<ci>a_b</ci>", r"\mathrm{a\_b}"),
        ("<ci>a#b</ci>", r"\mathrm{a\#b}"),
        ("<ci>a$b</ci>", r"\mathrm{a\$b}"),
        ("<ci>a%b</ci>", r"\mathrm{a\%b}"),
        ("<ci>a&amp;b</ci>", r"\mathrm{a\&b}"),
        ("<ci>a{b}</ci>", r"\mathrm{a\{b\}}"),
        (r"<ci>a\b</ci>", r"\mathrm{a\backslash b}"),
        ("<ci>a^b</ci>", r"\mathrm{a\wedge b}"),
        ("<ci>a~b</ci>", r"\mathrm{a\sim b}"),
    ]);
}

#[test]
fn content_operators_and_their_parentheses() {
    let ci = |x: &str| format!("<ci>{x}</ci>");
    let cn = |x: &str| format!("<cn>{x}</cn>");
    let apply = |op: &str, args: &[String]| format!("<apply><{op}/>{}</apply>", args.concat());
    let half = apply("divide", &[cn("1"), cn("2")]);
    let cases: Vec<(String, &str)> = vec![
        // a fraction inside stretches the parentheses around it
        (apply("times", &[apply("plus", &[half.clone(), ci("x")]), ci("y")]), r"\left( \frac{1}{2} + x \right) \cdot y"),
        (apply("plus", &[ci("x")]), "+x"),
        (apply("plus", &[apply("minus", &[ci("x")])]), "+(-x)"),
        (apply("floor", &[ci("x")]), r"\lfloor x \rfloor"),
        (apply("ceiling", std::slice::from_ref(&half)), r"\lceil \frac{1}{2} \rceil"),
        (apply("setdiff", &[ci("A"), apply("union", &[ci("B"), ci("C")])]), r"A \setminus (B \cup C)"),
        (apply("abs", std::slice::from_ref(&half)), r"\left| \frac{1}{2} \right|"),
        (apply("max", &[ci("a"), ci("b")]), r"\max(a, b)"),
        (apply("max", &[half.clone(), ci("b")]), r"\max\left( \frac{1}{2}, b \right)"),
        (apply("sin", std::slice::from_ref(&half)), r"\sin\left( \frac{1}{2} \right)"),
        // a function is last in a product read side by side; anywhere else a dot
        (apply("times", &[cn("3"), apply("sin", &[ci("x")])]), r"3 \sin x"),
        (apply("times", &[apply("sin", &[ci("x")]), ci("y")]), r"\sin x \cdot y"),
        (apply("times", &[ci("a"), apply("minus", &[ci("b")])]), r"a \cdot (-b)"),
        (apply("power", &[half.clone(), cn("2")]), r"\left( \frac{1}{2} \right)^{2}"),
        (apply("factorial", &[apply("minus", &[ci("n")])]), r"(-n)!"),
        (apply("notin", &[ci("x"), ci("A")]), r"x \notin A"),
        (apply("subset", &[ci("A"), ci("B")]), r"A \subseteq B"),
        (apply("prsubset", &[ci("A"), ci("B")]), r"A \subset B"),
        (apply("equivalent", &[ci("p"), ci("q")]), r"p \equiv q"),
        (apply("eq", &[ci("x"), apply("lt", &[ci("a"), ci("b")])]), r"x = (a < b)"),
        // unary_minus and OpenMath's names
        ("<apply><csymbol>unary_minus</csymbol><ci>x</ci></apply>".into(), "-x"),
        ("<apply><csymbol>unary_plus</csymbol><ci>x</ci></apply>".into(), "+x"),
        ("<apply><ci>sin</ci><ci>x</ci></apply>".into(), r"\sin x"),
        // a function that is not elementary: by its name, or by the presentation of its name
        ("<apply><ci>rank</ci><ci>M</ci></apply>".into(), r"\operatorname{rank}(M)"),
        ("<apply><ci>f</ci>".to_string() + &half + "</apply>", r"f\left( \frac{1}{2} \right)"),
        ("<apply><ci><msub><mi>f</mi><mn>1</mn></msub></ci><ci>x</ci><ci>y</ci></apply>".into(), r"f_{1}(x, y)"),
        ("<apply><apply><ci>f</ci><ci>x</ci></apply><ci>y</ci></apply>".into(), r"f(x)(y)"),
        // semantics inside content: its content
        ("<apply><plus/><semantics><ci>a</ci><annotation>A</annotation></semantics><cn>1</cn></apply>".into(), "a + 1"),
        ("<apply><plus/><semantics><annotation>A</annotation></semantics><cn>1</cn></apply>".into(), "{} + 1"),
        // constants
        ("<apply><in/><ci>p</ci><primes/></apply>".into(), r"p \in \mathbb{P}"),
        ("<apply><neq/><true/><false/></apply>".into(), r"\mathrm{true} \neq \mathrm{false}"),
        ("<apply><plus/><exponentiale/><imaginaryi/><eulergamma/><notanumber/><emptyset/></apply>".into(), r"e + i + \gamma + \mathrm{NaN} + \emptyset"),
        ("<apply><in/><ci>x</ci><naturalnumbers/></apply>".into(), r"x \in \mathbb{N}"),
        ("<apply><in/><ci>x</ci><integers/></apply>".into(), r"x \in \mathbb{Z}"),
        ("<apply><in/><ci>x</ci><rationals/></apply>".into(), r"x \in \mathbb{Q}"),
        ("<apply><in/><ci>x</ci><complexes/></apply>".into(), r"x \in \mathbb{C}"),
        ("<apply><tendsto/><ci>x</ci><pi/></apply>".into(), r"x \to \pi"),
        ("<apply><gt/><ci>x</ci><cn>0</cn></apply>".into(), r"x > 0"),
        ("<apply><geq/><ci>x</ci><cn>0</cn></apply>".into(), r"x \geq 0"),
    ];
    for (m, tex) in &cases {
        same_as_latex(&[(m.as_str(), tex)]);
    }
}

#[test]
fn content_elementary_functions() {
    for name in ["sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "ln", "log", "max", "min", "gcd"] {
        same_as_latex(&[(&format!("<apply><{name}/><ci>x</ci></apply>"), &format!(r"\{name} x"))]);
    }
}

#[test]
fn content_calculus_with_every_qualifier() {
    same_as_latex(&[
        // a derivative without a bound variable: the prime
        ("<apply><diff/><ci>f</ci></apply>", "f'"),
        // the order given on the apply, and the orders of several variables summed
        ("<apply><partialdiff/><bvar><ci>x</ci></bvar><bvar><ci>y</ci></bvar><degree><cn>2</cn></degree><ci>f</ci></apply>", r"\frac{\partial^{2} f}{\partial x \partial y}"),
        ("<apply><partialdiff/><bvar><ci>x</ci></bvar><bvar><ci>y</ci></bvar><ci>f</ci></apply>", r"\frac{\partial^{2} f}{\partial x \partial y}"),
        ("<apply><partialdiff/><bvar><ci>x</ci><degree><ci>m</ci></degree></bvar><bvar><ci>y</ci></bvar><ci>f</ci></apply>", r"\frac{\partial f}{\partial x^{m} \partial y}"),
        // the limits of an integral as an interval, or its domain
        ("<apply><int/><bvar><ci>x</ci></bvar><interval><cn>0</cn><cn>1</cn></interval><ci>f</ci></apply>", r"\int_{0}^{1} f \, dx"),
        ("<apply><int/><bvar><ci>x</ci></bvar><domainofapplication><ci>D</ci></domainofapplication><ci>f</ci></apply>", r"\int_{D} f \, dx"),
        ("<apply><int/><bvar><ci>θ</ci></bvar><ci>f</ci></apply>", r"\int f \, dθ"),
        // a sum over a condition, a product without a bound variable
        ("<apply><sum/><bvar><ci>p</ci></bvar><condition><apply><in/><ci>p</ci><primes/></apply></condition><ci>p</ci></apply>", r"\sum_{p \in \mathbb{P}} p"),
        ("<apply><product/><lowlimit><cn>1</cn></lowlimit><uplimit><ci>n</ci></uplimit><ci>k</ci></apply>", r"\prod_{1}^{n} k"),
        ("<apply><sum/><bvar><ci>i</ci></bvar><interval><cn>1</cn><ci>n</ci></interval><ci>i</ci></apply>", r"\sum_{i = 1}^{n} i"),
        // limits by a condition, or with nothing under them
        ("<apply><limit/><bvar><ci>x</ci></bvar><condition><apply><tendsto/><ci>x</ci><cn>0</cn></apply></condition><ci>f</ci></apply>", r"\lim_{x \to 0} f"),
        ("<apply><limit/><condition><apply><tendsto/><ci>x</ci><cn>0</cn></apply></condition><ci>f</ci></apply>", r"\lim_{x \to 0} f"),
        ("<apply><limit/><ci>f</ci></apply>", r"\lim f"),
        // a set of its elements with a condition
        ("<set><bvar><ci>x</ci></bvar><condition><apply><gt/><ci>x</ci><cn>0</cn></apply></condition><apply><power/><ci>x</ci><cn>2</cn></apply></set>", r"\{x^{2} \mid x > 0\}"),
        (r#"<interval closure="open-closed"><cn>0</cn><cn>1</cn></interval>"#, "(0, 1]"),
    ]);
}

#[test]
fn content_recovery() {
    // a bound variable with no variable: an empty one
    let r = mml("<apply><diff/><bvar><degree><cn>2</cn></degree></bvar><ci>y</ci></apply>");
    assert_eq!(r.tree, latex(r"\frac{d^{2} y}{d {}^{2}}").tree);
    // an apply with nothing in it; a relation with one operand
    assert_eq!(recovered("<apply/>", &["E-REC-10"]).tree, latex("{}").tree);
    assert_eq!(recovered("<apply><eq/><ci>x</ci></apply>", &["E-REC-10"]).tree, latex("x").tree);
    // an interval with one end gives an integral no limits
    let r = mml("<apply><int/><bvar><ci>x</ci></bvar><interval><cn>0</cn></interval><ci>f</ci></apply>");
    assert_eq!(r.tree, latex(r"\int f \, dx").tree);
    // content elements outside the subset, by name: their operands as a row
    for src in ["<lambda><ci>x</ci></lambda>", "<vector><cn>1</cn><cn>2</cn></vector>", "<piecewise/>", "<list/>", "<matrix/>"] {
        let r = convert(&wrap(src), &options(false)).unwrap();
        assert_eq!(r.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), ["E-REC-10"], "{src}");
    }
    assert_eq!(convert(&wrap("<vector><cn>1</cn><cn>2</cn></vector>"), &options(false)).unwrap().tree, latex("{1 2}").tree);
    // content nested deeper than the reader goes is refused, not a stack overflow
    let deep = format!("<math>{}<ci>x</ci>{}</math>", "<apply><minus/>".repeat(200), "</apply>".repeat(200));
    assert!(convert(&deep, &options(false)).unwrap_err().to_string().contains("nesting"));
}

#[test]
fn content_follows_the_locale_for_complex_numbers() {
    let pt = convert("<math><cn type=\"complex-cartesian\">1.5<sep/>2</cn></math>", &Options { input: Input::MathMl, locale: Locale::PtBr, strict: true, ..Options::default() })
        .unwrap();
    assert_eq!(pt.text, "1,5 + 2i");
}
