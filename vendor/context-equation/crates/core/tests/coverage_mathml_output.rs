//! MathML output (spec/mathml.md), the corners the other suites leave: the function concepts of
//! every function a language renames, the spacing of each class, letters whose variant Unicode has
//! no character for, the intervals of L-INT-1 with longer bounds, and the elements a tree can ask
//! for that the LaTeX reader seldom writes (scripts and stacks without their parts, an empty name).

use context_equation_core::output::mathml::{class_spacing, function_concept};
use context_equation_core::tree::{Cancel, Node, Variant};
use context_equation_core::{convert, Class, Locale, Options};

fn mml_in(tex: &str, locale: Locale) -> String {
    let out = convert(tex, &Options { strict: true, locale, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    out.mathml
}

fn mml(tex: &str) -> String {
    mml_in(tex, Locale::En)
}

/// The MathML of a tree built by hand, as an inline equation.
fn of(tree: Node) -> String {
    context_equation_core::mathml(&tree, Locale::En, false, "")
}

fn identifier(name: &str, variant: Variant) -> Node {
    Node::Identifier { name: name.into(), variant }
}

const INLINE: &str = r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="inline">"#;

#[test]
fn every_function_a_language_may_rename_has_its_concept() {
    // lcm is renamed in French: the intent says which function `ppcm` is
    let m = mml_in(r"\lcm(a, b)", Locale::Fr);
    assert!(m.contains(r#"<mi intent="least-common-multiple">ppcm</mi>"#), "{m}");
    for (name, concept) in [("lcm", "least-common-multiple"), ("max", "max"), ("min", "min"), ("lim", "limit"), ("arg", "complex-arg")] {
        assert_eq!(function_concept(name), Some(concept));
    }
    // a function MathML has no concept for
    assert_eq!(function_concept("sgn"), None);
}

#[test]
fn an_operator_of_class_op_gets_thin_spaces() {
    assert_eq!(class_spacing(Class::Op), ("0.1667em", "0.1667em", None));
    let m = of(Node::Operator { symbol: '+', class: Class::Op });
    assert_eq!(m, format!(r#"{INLINE}<mo lspace="0.1667em" rspace="0.1667em">+</mo></math>"#));
}

#[test]
fn letters_with_no_character_in_their_variant_keep_the_attribute() {
    // é has no bold (nor bold italic…) letter in Unicode: the name keeps the mathvariant
    for (variant, name) in
        [(Variant::Bold, "bold"), (Variant::BoldItalic, "bold-italic"), (Variant::BoldSansSerif, "bold-sans-serif"), (Variant::SansSerifBoldItalic, "sans-serif-bold-italic")]
    {
        let m = of(identifier("aé", variant));
        assert_eq!(m, format!(r#"{INLINE}<mi mathvariant="{name}">aé</mi></math>"#));
    }
    // an empty name is an empty <mi>
    assert_eq!(of(identifier("", Variant::Italic)), format!("{INLINE}<mi></mi></math>"));
}

#[test]
fn a_slashed_letter_with_no_character_in_its_variant_is_a_fallback() {
    // 𝔸 exists, a double-struck α does not: the slashed token cannot be written, the TeX is kept
    let tree = Node::Canceled { base: Box::new(identifier("α", Variant::DoubleStruck)), shape: Cancel::Slash };
    let m = context_equation_core::mathml(&tree, Locale::En, false, r"\slashed{\mathbb{α}}");
    assert!(m.contains("<semantics>") && m.contains(r#"<annotation encoding="application/x-tex">\slashed{\mathbb{α}}</annotation>"#), "{m}");
    // the same read from LaTeX
    assert!(mml(r"\slashed{\mathbb{α}}").contains("<semantics>"));
    // a letter with one is the token
    assert!(mml(r"\slashed{\mathbb{A}}").contains("<mi>𝔸\u{0338}</mi>"));
}

#[test]
fn what_mathml_cannot_draw_is_found_inside_a_vcenter() {
    let m = mml(r"\vcenter{\xymatrix{A \ar[r] & B}}");
    assert!(m.contains("<semantics>"), "{m}");
    // with nothing of the kind inside, the box is its content
    assert_eq!(mml(r"\vcenter{x}"), r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="block"><mi>x</mi></math>"#);
}

#[test]
fn intervals_with_longer_bounds_and_spans_that_are_not_intervals() {
    // a bound of more than one node is an <mrow> with its arg
    let m = mml_in(r"]a+1, b[", Locale::Fr);
    assert!(m.contains(r#"<mrow intent="open-interval($a,$b)">"#), "{m}");
    assert!(m.contains(r#"<mrow arg="a"><mi>a</mi><mo>+</mo><mn>1</mn></mrow>"#), "{m}");
    assert!(m.contains(r#"<mi arg="b">b</mi>"#), "{m}");
    // two separators, or a bracket opened inside: no interval
    for tex in [r"]a, b, c[", r"]a, (b, c["] {
        assert!(!mml_in(tex, Locale::Fr).contains("intent"), "{tex}");
    }
    // a [ that opens and a [ that closes, as a tree may give them: closed at the left, open at the right
    let op = |symbol: char, class: Class| Node::Operator { symbol, class };
    let tree = Node::row(vec![op('[', Class::Open), identifier("a", Variant::Italic), op(',', Class::Punct), identifier("b", Variant::Italic), op('[', Class::Close)]);
    let m = context_equation_core::mathml(&tree, Locale::Fr, false, "");
    assert!(m.contains(r#"intent="closed-open-interval($a,$b)""#), "{m}");
}

#[test]
fn delimiters_scripts_accents_and_text_in_their_rarer_forms() {
    // \bigl| is an opening fence: prefix form, as | alone is not one
    let m = mml(r"\bigl| x \bigr|");
    assert!(m.contains(r#"<mo form="prefix" lspace="0em" rspace="0em" stretchy="true""#), "{m}");
    assert!(m.contains(r#"<mo form="postfix""#), "{m}");
    // a limit over a sum, with none under it
    assert!(mml(r"\sum^{n} x").contains("<mover><mo>∑</mo><mi>n</mi></mover>"));
    // \check: the caron as the reader knows it
    assert!(mml(r"\check{x}").contains(r#"<mover accent="true"><mi>x</mi><mo stretchy="false">ˇ</mo></mover>"#));
    // empty text is an empty <mtext>
    assert!(mml(r"\text{}").contains("<mtext></mtext>"));
}

#[test]
fn scripts_and_stacks_without_their_parts() {
    let x = || Box::new(identifier("x", Variant::Italic));
    // scripts with neither part keep an empty subscript, so the element has its two children
    assert_eq!(of(Node::Scripts { base: x(), sup: None, sub: None }), format!("{INLINE}<msub><mi>x</mi><mrow></mrow></msub></math>"));
    // a stack with neither part is its base
    assert_eq!(of(Node::Stacked { base: x(), above: None, below: None }), format!("{INLINE}<mrow><mi>x</mi></mrow></math>"));
}
