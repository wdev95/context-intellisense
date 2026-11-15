//! Braille output (output/braille): the constructions and signs that `braille.rs` does not reach. Each
//! expected string is derived from the rule the code cites (BANA, *The Nemeth Braille Code for Mathematics
//! and Science Notation*, 2022; *Código Matemático Unificado*, MEC/SEESP, 2006; see spec/braille.md) and
//! from the dot numbers of its sign tables, which `dots` turns into cells here independently of the code.

use context_equation_core::output::braille::transcribe;
use context_equation_core::tree::Variant;
use context_equation_core::{convert, BrailleCode, Input, Locale, Node, Options};

/// Cells from dot numbers separated by spaces, "1246 135" → ⠫⠕; "0" is the blank cell.
fn dots(d: &str) -> String {
    d.split_whitespace()
        .map(|c| {
            let bits = c.chars().filter_map(|d| d.to_digit(10)).filter(|d| (1..=8).contains(d)).fold(0, |acc, d| acc | 1 << (d - 1));
            char::from_u32(0x2800 + bits).unwrap()
        })
        .collect()
}

/// The braille of `src` in `code`, in the language of the code; no symbol may lack its sign (S-BRL-1).
fn braille_in(src: &str, input: Input, code: BrailleCode) -> String {
    let locale = if code == BrailleCode::Cmu { Locale::PtBr } else { Locale::En };
    let r = convert(src, &Options { locale, input, braille: Some(code), strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{src}: {e}"));
    assert!(r.applied.iter().all(|a| a.rule != "S-BRL-1"), "{src}: {:?}", r.applied);
    r.braille
}

fn check(code: BrailleCode, input: Input, cases: &[(&str, String, &str)]) {
    for (src, expected, source) in cases {
        assert_eq!(braille_in(src, input, code), *expected, "{src} ({source})");
    }
}

fn identifier(name: &str, variant: Variant) -> Node {
    Node::Identifier { name: name.to_string(), variant }
}

fn number(value: &str) -> Node {
    Node::Number { value: value.to_string(), group: false, thousands: None, variant: Variant::Upright }
}

/// A matrix between two different fences, which MathML keeps as a table inside fences (not a pmatrix):
/// `intent` marks the content as one node.
fn fenced_table(open: &str, close: &str) -> String {
    format!(
        r#"<math><mrow intent="binomial-coefficient($n,$k)"><mo>{open}</mo><mtable><mtr><mtd><mn>1</mn></mtd><mtd><mi>a</mi></mtd></mtr><mtr><mtd><mn>2</mn></mtd><mtd><mi>b</mi></mtd></mtr></mtable><mo>{close}</mo></mrow></math>"#
    )
}

#[test]
fn nemeth_comparison_signs_of_the_tables() {
    // comparison signs with a space on each side (§21.13); negation is ⠌ before the sign (§21.1)
    let spaced = |d: &str| format!("⠁⠀{}⠀⠃", dots(d));
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[
            (r"a \ni b", spaced("4 26"), "Rule 21 table, contains as member"),
            (r"a \nparallel b", spaced("34 1246 123"), "§21.1 applied to parallel"),
            (r"a \simeq b", spaced("4 156 156"), "Rule 21 table, tilde over a bar"),
            (r"a \longleftarrow b", spaced("1246 246 25 25 25"), "§22.5.3: long arrows, three shaft cells"),
            (r"a \longleftrightarrow b", spaced("1246 246 25 25 25 135"), "§22.5.3"),
            (r"a \updownarrow b", spaced("1246 126 246 25 25 135"), "Rule 22, up and down arrow"),
            (r"A \nsubseteq B", format!("⠠⠁⠀{}⠀⠠⠃", dots("34 456 5 13 156")), "§21.1 applied to subset or equal"),
        ],
    );
}

#[test]
fn nemeth_modified_arrows_take_their_uncontracted_form() {
    // ⠐ the arrow ⠣ the label ⠻ (§15), the arrow in its uncontracted form (§22.1)
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[
            (r"\overset{f}{\to}", format!("⠐{}⠣⠋⠻", dots("1246 25 25 135")), "§22.1"),
            (r"\xLeftrightarrow{a}", format!("⠐{}⠣⠁⠻", dots("1246 246 2356 2356 135")), "§22.1, double arrow"),
        ],
    );
}

#[test]
fn nemeth_arrows_as_modifiers() {
    // ⠐ the expression ⠣ over / ⠩ under, the arrow, ⠻ (§15). Only the right arrow has a contracted
    // form, ⠫⠕ (§22); the left arrow (⠫⠪ alone is the angle, Rule 17) and the two-headed one keep
    // their full signs from the arrow table (§22.1)
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[
            (r"\overrightarrow{x}", format!("⠐⠭⠣{}⠻", dots("1246 135")), "§15, §22: contracted right arrow"),
            (r"\overleftarrow{x}", format!("⠐⠭⠣{}⠻", dots("1246 246 25 25")), "§15, §22.1: left arrow"),
            (r"\overleftrightarrow{AB}", format!("⠐⠠⠁⠠⠃⠣{}⠻", dots("1246 246 25 25 135")), "§15, §22.1: two-headed arrow"),
            (r"\underrightarrow{x}", format!("⠐⠭⠩{}⠻", dots("1246 135")), "§15, §22: under"),
            (r"\underleftarrow{x}", format!("⠐⠭⠩{}⠻", dots("1246 246 25 25")), "§15, §22.1: under"),
            (r"\underleftrightarrow{x}", format!("⠐⠭⠩{}⠻", dots("1246 246 25 25 135")), "§15, §22.1: under"),
        ],
    );
}

#[test]
fn nemeth_a_table_between_different_fences_has_them_enlarged_on_every_line() {
    // §19.6: enlarged grouping signs (dot 6 after the prefix) at both ends of every line; §25.8.1: a numeric
    // indicator on every number; no English-letter indicator in a matrix
    for (open, close, o, c) in [
        ("(", "]", "6 12356", "4 6 23456"),
        ("[", ")", "4 6 12356", "6 23456"),
        ("{", "|", "46 6 12356", "6 1256"),
        ("|", "‖", "6 1256", "6 1256 6 1256"),
        ("‖", "}", "6 1256 6 1256", "46 6 23456"),
    ] {
        let expected = format!("{o}⠼⠂⠀⠁{c}\n{o}⠼⠆⠀⠃{c}", o = dots(o), c = dots(c));
        assert_eq!(braille_in(&fenced_table(open, close), Input::MathMl, BrailleCode::Nemeth), expected, "{open} {close}");
    }
    // angle brackets have no enlarged form: the table is written between the plain signs
    let angle = braille_in(&fenced_table("⟨", "⟩"), Input::MathMl, BrailleCode::Nemeth);
    assert!(angle.starts_with("⠨⠨⠷") && angle.ends_with("⠨⠨⠾"), "{angle}");
}

#[test]
fn nemeth_a_complex_fraction_sees_through_wrappers() {
    // §13: a fraction inside the numerator makes the outer one complex, also behind color, an HTML class,
    // a forced class or a style; ⠠⠹ ⠠⠌ ⠠⠼
    let complex = "⠠⠹⠹⠁⠌⠃⠼⠠⠌⠉⠠⠼".to_string();
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[
            (r"\frac{\color{red}{\frac{a}{b}}}{c}", complex.clone(), "§13, colored"),
            (r"\frac{\htmlClass{k}{\frac{a}{b}}}{c}", complex.clone(), "§13, attributed"),
            (r"\frac{\mathbin{\frac{a}{b}}}{c}", complex.clone(), "§13, forced class"),
            (r"\frac{{\textstyle\frac{a}{b}}}{c}", complex.clone(), "§13, style"),
            // the rectangle with the fraction as its interior modifier (§15.10)
            (r"\frac{\boxed{\frac{a}{b}}}{c}", format!("⠠⠹{}⠹⠁⠌⠃⠼⠻⠠⠌⠉⠠⠼", dots("1246 1235 456 1246")), "§13, §15.10, framed"),
        ],
    );
}

#[test]
fn nemeth_a_number_before_a_fraction_that_is_not_a_mixed_number() {
    // §13.4 mixed numbers are a whole number and a fraction of numbers; otherwise the number and a simple
    // fraction ⠹ ⠌ ⠼ (§13), the decimal point ⠨ (§3)
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[(r"2\frac{a}{b}", "⠼⠆⠹⠁⠌⠃⠼".into(), "§13, a literal fraction"), (r"2.5\frac{1}{2}", "⠼⠆⠨⠢⠹⠂⠌⠆⠼".into(), "§13, a decimal before it")],
    );
}

#[test]
fn primes_come_before_the_scripts_in_both_codes() {
    // a triple prime as one character: Nemeth ⠄ per prime (§14.12), CMU ⠳ per prime (§2.4)
    let triple = r#"<math><msup><mi>f</mi><mo>‴</mo></msup></math>"#;
    let squared = r#"<math><msup><mi>f</mi><mrow><mo>‴</mo><mn>2</mn></mrow></msup></math>"#;
    assert_eq!(braille_in(triple, Input::MathMl, BrailleCode::Nemeth), "⠋⠄⠄⠄");
    assert_eq!(braille_in(squared, Input::MathMl, BrailleCode::Nemeth), "⠋⠄⠄⠄⠘⠆");
    assert_eq!(braille_in(triple, Input::MathMl, BrailleCode::Cmu), "⠋⠳⠳⠳");
    assert_eq!(braille_in(squared, Input::MathMl, BrailleCode::Cmu), "⠋⠳⠳⠳⠡⠼⠃");
}

#[test]
fn nemeth_units_built_as_trees() {
    // a quantity whose unit is one node (`value` is the text of the number, which the unit row draws):
    // the unit is an abbreviation, with no English-letter indicator (§6.3.1 c), unlike a letter alone (§6.3)
    let quantity = Node::Quantity { value: Some("3".into()), unit: Box::new(identifier("m", Variant::Upright)) };
    assert_eq!(transcribe(&quantity, BrailleCode::Nemeth).text, "⠍");
    assert_eq!(transcribe(&identifier("m", Variant::Upright), BrailleCode::Nemeth).text, "⠰⠍");
    // a unit is a letter for its subscript: a numeric subscript of first order needs no indicator (§14.6)
    let unit = Node::Unit { prefix: None, name: "metre".into(), base: Box::new(identifier("m", Variant::Upright)) };
    let scripts = Node::Scripts { base: Box::new(unit), sup: None, sub: Some(Box::new(number("2"))) };
    assert_eq!(transcribe(&scripts, BrailleCode::Nemeth).text, "⠍⠆");
    // so is a quantity whose unit is that one letter
    let scripts = Node::Scripts { base: Box::new(quantity), sup: None, sub: Some(Box::new(number("2"))) };
    assert_eq!(transcribe(&scripts, BrailleCode::Nemeth).text, "⠍⠆");
}

#[test]
fn nemeth_a_numeric_subscript_of_a_unit_letter() {
    // the base of the script is the quantity, not the unit: what decides is what it ends with in braille.
    // \si{m}_1 is m with the subscript 1, as m_1: no subscript indicator (§14.6). \qty{3}{m}_1 also ends
    // with the letter m, which the subscript follows directly in braille as in print (3 m₁), so it takes
    // none either; the number before it is spaced from the unit (§10), not part of the letter. A unit of
    // several letters (kg) or a unit with its own superscript is not a letter, and keeps the indicator.
    check(
        BrailleCode::Nemeth,
        Input::Latex,
        &[
            (r"m_1", "⠍⠂".into(), "§14.6"),
            (r"\si{m}_1", "⠍⠂".into(), "§14.6, as m_1"),
            (r"\si{\metre}_1", "⠍⠂".into(), "§14.6, a named unit"),
            (r"\qty{3}{m}_1", "⠼⠒⠀⠍⠂".into(), "§10, §14.6: the subscript of the unit's letter"),
            (r"\qty{3}{\metre}_1", "⠼⠒⠀⠍⠂".into(), "§10, §14.6"),
            (r"\si{kg}_1", "⠅⠛⠰⠂".into(), "§14.6 does not apply: two letters"),
            (r"\si{m^2}_1", "⠍⠘⠆⠰⠂".into(), "§14.6 does not apply: the base ends with a superscript"),
            (r"\si{m}_x", "⠍⠰⠭".into(), "§14.6 does not apply: not numeric"),
        ],
    );
}

#[test]
fn greek_omicron_in_both_codes() {
    // Nemeth: the Greek letter indicator ⠨ (§6.1.4); CMU: ⠈ before a Greek letter (§1.2)
    assert_eq!(braille_in(r"\omicron", Input::Latex, BrailleCode::Nemeth), "⠨⠕");
    assert_eq!(braille_in(r"\omicron", Input::Latex, BrailleCode::Cmu), "⠈⠕");
}

#[test]
fn cmu_negated_and_set_relations() {
    // negation: ⠘ before the relation, no spaces (§4.3); set relations (§6)
    let between = |d: &str| format!("⠁{}⠃", dots(d));
    check(
        BrailleCode::Cmu,
        Input::Latex,
        &[
            (r"a \nless b", between("45 246"), "§4.3, not less than"),
            (r"a \not\equiv b", between("45 2356 2356"), "§4.3, not identical to"),
            (r"a \not\approx b", between("45 4 2356"), "§4.3, not approximately equal"),
            (r"a \not\subset b", between("45 126 3"), "§4.3, §6.1"),
            (r"a \nsubseteq b", between("45 126 23"), "§4.3, §6.1"),
            (r"a \ni b", between("5 345"), "§6.1, contains as member"),
            (r"a \nLeftarrow b", between("45 246 25"), "§4.3 applied to the implication"),
            (r"a \nLeftrightarrow b", between("45 246 25 135"), "§4.3 applied to the equivalence"),
        ],
    );
}

#[test]
fn cmu_constructions_of_the_rows() {
    check(
        BrailleCode::Cmu,
        Input::Latex,
        &[
            // left indices, then the right ones: the letter, ⠠⠌ lower, ⠈⠡ upper, ⠡ the charge (§2.2, §2.4.2)
            (r"{}^{14}_{6}\mathrm{C}^{+}", "⠨⠉⠠⠌⠼⠋⠈⠡⠼⠁⠙⠡⠖".into(), "§2.2, §2.4.2"),
            // the square directly followed by its points (§8.1)
            (r"\square ABCD", format!("{}⠨⠁⠨⠃⠨⠉⠨⠙", dots("456 13456")), "§8.1: quadrado"),
            // ">" followed by the letter o: the letter takes ⠐ (§4.2)
            (r"x > o", "⠭⠕⠐⠕".into(), "§4.2"),
            // a number and a literal fraction is not a mixed number; a after a number takes ⠐ (§1.1 a, §5.1)
            (r"2\frac{a}{b}", "⠼⠃⠐⠁⠲⠃".into(), "§1.1 a, §5.1"),
            // the scale and rotation of a box do not change what it says
            (r"\resizebox{1cm}{!}{x}", "⠭".into(), "transparent"),
            (r"\rotatebox{90}{x}", "⠭".into(), "transparent"),
        ],
    );
}

#[test]
fn cmu_numbers_words_and_letters() {
    check(
        BrailleCode::Cmu,
        Input::Latex,
        &[
            // a numeric fraction: the numerator in the lower part, the denominator in the upper (§3.2.2)
            (r"\frac{67890}{1}", format!("⠼{}⠁", dots("235 2356 236 35 356")), "§3.2.2"),
            (r"\frac{45}{1}", format!("⠼{}⠁", dots("256 26")), "§3.2.2"),
            // a word, a blank, then a number with its sign (§3.1)
            (r"\text{abc 12}", "⠁⠃⠉⠀⠼⠁⠃".into(), "§3.1"),
            // the accented letters of Portuguese braille; a capital with ⠨
            (r"\text{áàâãéêíóôõúüçÁ}", dots("12356 1246 16 345 123456 126 34 346 1456 246 23456 1256 12346 46 12356"), "Portuguese braille"),
        ],
    );
    // a word as one identifier: its letters; in a variant, each letter with its prefix ⠠ (§1.1)
    assert_eq!(transcribe(&identifier("ab", Variant::Italic), BrailleCode::Cmu).text, "⠁⠃");
    assert_eq!(transcribe(&identifier("ab", Variant::Upright), BrailleCode::Cmu).text, "⠁⠃");
    assert_eq!(transcribe(&identifier("ab", Variant::Bold), BrailleCode::Cmu).text, "⠠⠁⠠⠃");
}

#[test]
fn cmu_marks_limits_and_auxiliary_parentheses() {
    check(
        BrailleCode::Cmu,
        Input::Latex,
        &[
            // the left arrow over a letter, written before it (§2.3.2)
            (r"\overleftarrow{x}", format!("{}⠭", dots("5 25")), "§2.3.2"),
            // the limit under max is the subscrito ⠌⠌, a bound over it the sobrescrito ⠡⠡ (§2.2)
            (r"\max_a^b", "⠍⠁⠭⠄⠌⠌⠁⠡⠡⠃".into(), "§2.1, §2.2"),
            // one term needs no auxiliary parentheses (§1.3): a grouped expression, a marked letter
            (r"\sqrt{\left(a+b\right)}", "⠫⠱⠣⠁⠖⠃⠜".into(), "§1.3, §5.3"),
            (r"\sqrt{\bar{x}}", "⠫⠱⠈⠉⠭".into(), "§1.3, §2.3.2"),
        ],
    );
}
