//! Digits in the alphabets that have them (#203): `\mathbf{1}` is 𝟏, `\mathsf{1}` 𝟣, `\mathtt{1}` 𝟷, as
//! LaTeX draws bold, sans-serif and typewriter digits. The number keeps its value; speech and braille
//! read it as the number, the text and MathML write the Unicode digits and MathML reads them back.

use context_equation_core::tree::{Node, Variant};
use context_equation_core::{convert, font, Input, Options};

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

fn only(node: &Node) -> &Node {
    match node {
        Node::Row { children } if children.len() == 1 => only(&children[0]),
        other => other,
    }
}

/// (input, the number's alphabet, its text)
const CASES: &[(&str, Variant, &str)] = &[
    (r"\mathbf{1}", Variant::Bold, "𝟏"),
    (r"\mathsf{1}", Variant::SansSerif, "𝟣"),
    (r"\mathtt{1}", Variant::Monospace, "𝟷"),
    (r"\mathbfsf{1}", Variant::BoldSansSerif, "𝟭"),
    (r"\mathsfit{2}", Variant::SansSerif, "𝟤"),
    (r"\mathbfsfit{2}", Variant::BoldSansSerif, "𝟮"),
    (r"\boldsymbol{2}", Variant::Bold, "𝟐"),
    (r"\symbf{3}", Variant::Bold, "𝟑"),
    (r"\mathbf{1.5}", Variant::Bold, "𝟏.𝟓"),
    (r"\mathbf{120}", Variant::Bold, "𝟏𝟐𝟎"),
    // the alphabets without digits leave them upright, as before
    (r"\mathit{1}", Variant::Upright, "1"),
    (r"\mathcal{1}", Variant::Upright, "1"),
    (r"\mathfrak{1}", Variant::Upright, "1"),
    (r"\mathrm{1}", Variant::Upright, "1"),
    (r"1", Variant::Upright, "1"),
];

#[test]
fn digits_take_the_alphabet_in_strict_and_tolerant_mode() {
    for &(tex, variant, text) in CASES {
        for options in [strict(), Options::default()] {
            let out = convert(tex, &options).unwrap_or_else(|e| panic!("{tex}: {e}"));
            assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
            match only(&out.tree) {
                Node::Number { variant: v, .. } => assert_eq!(*v, variant, "{tex}"),
                other => panic!("{tex}: {other:?}"),
            }
            assert_eq!(out.text, text, "{tex}");
        }
    }
}

#[test]
fn the_issue_examples() {
    let out = convert(r"\mathbf{1} + \mathbf{x}", &strict()).unwrap();
    assert_eq!(out.text, "𝟏 + 𝐱");
    assert_eq!(convert(r"\mathsf{1} \quad \mathtt{1}", &strict()).unwrap().text, "𝟣 𝟷");
    // the index is outside \mathbf: regular digits; inside, bold ones
    assert_eq!(convert(r"\mathbf{v}_{12}", &strict()).unwrap().text, "𝐯_12");
    assert_eq!(convert(r"\mathbf{v_{12}}", &strict()).unwrap().text, "𝐯_𝟏𝟐");
}

#[test]
fn the_font_draws_the_digits() {
    for c in ['𝟏', '𝟣', '𝟷', '𝟭'] {
        assert!(font::font().glyph(c).is_some(), "STIX Two Math has no {c}");
    }
    let plain = convert("1", &strict()).unwrap().svg;
    for tex in [r"\mathbf{1}", r"\mathsf{1}", r"\mathtt{1}"] {
        assert_ne!(convert(tex, &strict()).unwrap().svg, plain, "{tex}");
    }
}

#[test]
fn speech_and_braille_read_the_number() {
    for tex in [r"\mathbf{12}", r"\mathsf{12}", r"\mathtt{12}"] {
        let out = convert(tex, &strict()).unwrap();
        let plain = convert("12", &strict()).unwrap();
        assert_eq!(out.speech, plain.speech, "{tex}");
        assert_eq!(out.braille, plain.braille, "{tex}");
    }
}

#[test]
fn mathml_writes_the_unicode_digits_and_reads_them_back() {
    for tex in [r"\mathbf{1} + \mathbf{x}", r"\mathsf{1} \quad \mathtt{1}", r"\mathbf{v_{12}}", r"\mathbf{1.5}", r"\mathbfsf{7}"] {
        let out = convert(tex, &strict()).unwrap();
        let back = convert(&out.mathml, &Options { input: Input::MathMl, ..strict() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert_eq!(back.tree, out.tree, "{tex}: {}", out.mathml);
    }
    assert!(convert(r"\mathbf{12}", &strict()).unwrap().mathml.contains("<mn>𝟏𝟐</mn>"));
    // KaTeX's form: the digit with mathvariant
    let katex = convert(r#"<math><mn mathvariant="bold">1</mn></math>"#, &Options { input: Input::MathMl, ..strict() }).unwrap();
    assert_eq!(katex.text, "𝟏");
}

#[test]
fn the_double_struck_one_stays_a_symbol() {
    // #159: 𝟙 is an identifier, not a number
    match only(&convert(r"\mathbb{1}", &strict()).unwrap().tree) {
        Node::Identifier { name, variant: Variant::DoubleStruck } => assert_eq!(name, "1"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn asciimath_bold_digits() {
    let out = convert("bb(1) + sf(2) + tt(3)", &Options { input: Input::AsciiMath, ..strict() }).unwrap();
    assert_eq!(out.text, "𝟏 + 𝟤 + 𝟹");
}
