//! Speech of the corners the other tests leave out: the number sets written as Unicode characters,
//! vectors in braces, units over a fraction, a `\mathop` around more than an operator, and trees
//! built by hand.

use context_equation_core::output::speech::speak;
use context_equation_core::tree::{TextPart, Variant};
use context_equation_core::{convert, Locale, Node, Options};

fn speech(tex: &str, locale: Locale) -> String {
    convert(tex, &Options { locale, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}")).speech
}

fn identifier(name: &str) -> Node {
    Node::Identifier { name: name.into(), variant: Variant::Upright }
}

#[test]
fn the_number_sets_written_as_unicode_characters_are_read_by_name() {
    // L-LET-2: ℕ ℤ ℚ ℝ ℂ by their names, the other double-struck letters as letters
    assert_eq!(
        speech("ℂ ℍ ℕ ℙ ℚ ℝ ℤ 𝔸", Locale::En),
        "the complex numbers double-struck H the natural numbers double-struck P the rational numbers the real numbers the integers double-struck A"
    );
    // pt-BR joins the preposition with the article of the set's name
    assert_eq!(speech(r"x \in ℕ", Locale::PtBr), "x pertence aos naturais");
}

#[test]
fn a_double_struck_digit_character_is_read_as_its_digit() {
    // 𝟙, the indicator function, as the character in an identifier (a tree built by hand)
    assert_eq!(speak(&identifier("𝟙"), Locale::En), "double-struck 1");
    // and as MathML writes it
    let r = convert("<math><mi>𝟙</mi></math>", &Options { input: context_equation_core::Input::MathMl, ..Options::default() }).unwrap();
    assert_eq!(r.speech, "double-struck 1");
}

#[test]
fn a_vector_in_braces_is_still_a_vector_for_the_dot_product() {
    // L-OPR-1: the dot between two vectors is the dot product, braces or not
    assert_eq!(speech(r"{\mathbf{v}} \cdot \mathbf{w}", Locale::En), "v dot w");
    assert_eq!(speech(r"{{\vec{v}}} \cdot \vec{w}", Locale::En), "vector v dot vector w");
}

#[test]
fn units_over_a_fraction_with_a_bare_number_on_top_read_as_per() {
    // siunitx's per-mode=fraction: 1/s is "per second", the 1 is not read
    assert_eq!(speech(r"\sisetup{per-mode=fraction} \qty{3}{\per\second}", Locale::En), "3 per second");
    assert_eq!(speech(r"\sisetup{per-mode=fraction} \qty{3}{\kilo\metre\per\second}", Locale::En), "3 kilometers per second");
}

#[test]
fn a_quantity_whose_unit_is_not_a_row_keeps_its_symbol() {
    // the reader always writes a row; a tree built by hand may not
    let metre = Node::Unit { prefix: None, name: "meter".into(), base: Box::new(Node::Text { parts: vec![TextPart::Text("m".into())] }) };
    let quantity = Node::Quantity { value: Some("3".into()), unit: Box::new(metre) };
    assert_eq!(speak(&quantity, Locale::En), "m");
}

#[test]
fn a_mathop_around_an_operator_and_its_argument_is_read_as_written() {
    // \mathop holds more than the operator: its limits are a subscript of the whole
    assert_eq!(speech(r"\mathop{\sin x}\limits_{n} y", Locale::En), "sine of x sub n y");
}
