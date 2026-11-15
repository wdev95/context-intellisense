//! AsciiMath input (input/asciimath.rs), the corners the grammar tests leave: raw arguments in any
//! bracket, primes before an exponent, brace labels on the wrong side, characters that are no
//! symbol, the alphabets on quoted text and scripts, matrices with odd brackets, and what is not a
//! matrix after all.

use context_equation_core::tree::{TableKind, TextPart, Variant};
use context_equation_core::{convert, Class, Input, Node, Options};

fn options(strict: bool) -> Options {
    Options { input: Input::AsciiMath, strict, ..Options::default() }
}

/// Valid AsciiMath: the same in both modes, with no warnings.
fn am(src: &str) -> context_equation_core::Output {
    let strict = convert(src, &options(true)).unwrap_or_else(|e| panic!("{src}: {e}"));
    let tolerant = convert(src, &options(false)).unwrap();
    assert!(tolerant.warnings.is_empty(), "{src}: {:?}", tolerant.warnings);
    assert_eq!(strict.svg, tolerant.svg, "{src}");
    strict
}

fn first(src: &str) -> Node {
    am(src).tree.children()[0].clone()
}

fn latex(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn text(t: &str) -> Node {
    Node::Text { parts: vec![TextPart::Text(t.into())] }
}

#[test]
fn raw_arguments_take_any_bracket_and_spaces_before_it() {
    // text(…), text[…] and text{…} are the same text; a space may come before the bracket
    for src in ["text(a b)", "text[a b]", "text{a b}", "text  (a b)", "mbox[a b]"] {
        assert_eq!(first(src), text("a b"), "{src}");
    }
    assert!(matches!(first("color [blue] x"), Node::Colored { ref color, .. } if color == "blue"));
    // without a bracket, "text" is no command: its letters one by one, as any other word
    assert_eq!(am("text x").tree, am("t e x t x").tree);
}

#[test]
fn primes_come_before_the_exponent() {
    // f'^2: the prime and then the exponent, in one superscript, as LaTeX reads f'^2
    let n = first("f'^2");
    let Node::Scripts { sup: Some(sup), sub: None, .. } = &n else { panic!("{n:?}") };
    assert!(matches!(sup.children(), [Node::Operator { symbol: '′', .. }, Node::Number { .. }]), "{sup:?}");
    assert_eq!(am("f'^2").speech, latex("f'^2").speech);
    assert_eq!(am("f''(x)").text, latex("f''(x)").text);
}

#[test]
fn a_brace_takes_its_label_only_on_its_own_side() {
    // a superscript on a brace below is no label: the brace is scripted
    let n = first("ubrace(a+b)^2");
    let Node::Scripts { base, sup: Some(_), sub: None } = &n else { panic!("{n:?}") };
    assert!(matches!(base.as_ref(), Node::Brace { above: false, label: None, .. }), "{base:?}");
    let n = first("obrace(a+b)_2");
    assert!(matches!(&n, Node::Scripts { base, sub: Some(_), .. } if matches!(base.as_ref(), Node::Brace { above: true, label: None, .. })), "{n:?}");
}

#[test]
fn a_character_that_is_no_symbol_is_drawn_as_itself() {
    // ? is a closing punctuation in TeX; # and ¤ have no class and are ordinary
    assert_eq!(first("?"), Node::Operator { symbol: '?', class: Class::Close });
    assert_eq!(first("#"), Node::Operator { symbol: '#', class: Class::Ord });
    assert_eq!(first("¤"), Node::Operator { symbol: '¤', class: Class::Ord });
    // a character only a LaTeX command gives keeps that command's class: ≰ is \nleq
    assert_eq!(first("≰"), Node::Operator { symbol: '≰', class: Class::Rel });
    // non-Latin letters are identifiers, upright when they are not lowercase Greek
    assert_eq!(first("ж"), Node::Identifier { name: "ж".into(), variant: Variant::Upright });
    assert_eq!(first("λ"), Node::Identifier { name: "λ".into(), variant: Variant::Italic });
}

#[test]
fn overset_and_underset_take_the_class_of_their_base() {
    // over a relation: a relation; over a large operator: an operator; over a letter: ordinary
    assert!(matches!(first("overset(a)(=)"), Node::ForcedClass { class: Class::Rel, .. }));
    assert!(matches!(first("underset(i)(sum)"), Node::ForcedClass { class: Class::Op, .. }));
    assert!(matches!(first("overset(a)(sin)"), Node::ForcedClass { class: Class::Op, .. }));
    assert!(matches!(first("overset(a)(x)"), Node::ForcedClass { class: Class::Ord, .. }));
    assert_eq!(am("overset(a)(x)").speech, latex(r"\overset{a}{x}").speech);
}

#[test]
fn accents_widen_over_more_than_one_letter() {
    // vec over one letter is the small arrow; over more it stretches, as \overrightarrow
    assert!(matches!(first("vec v"), Node::Accent { accent: '\u{20D7}', stretchy: false, .. }));
    assert!(matches!(first("vec(AB)"), Node::Accent { accent: '→', stretchy: true, .. }));
    assert!(matches!(first("hat(AB)"), Node::Accent { accent: '\u{0302}', stretchy: true, .. }));
    assert!(matches!(first("tilde x"), Node::Accent { accent: '\u{0303}', stretchy: false, .. }));
    assert!(matches!(first("overparen(AB)"), Node::Accent { accent: '⏜', stretchy: true, below: false, .. }));
    assert!(matches!(first("underparen(AB)"), Node::Accent { accent: '⏝', stretchy: true, below: true, .. }));
}

#[test]
fn alphabets_reach_quoted_text_rows_and_scripts() {
    // bb "AB" is the bold word, as \mathbf{AB}
    assert_eq!(first(r#"bb "AB""#), Node::Identifier { name: "AB".into(), variant: Variant::Bold });
    // the base of a script takes the alphabet, the script keeps its own
    let n = first("bbb(R^n)");
    let Node::Scripts { base, sup: Some(sup), .. } = &n else { panic!("{n:?}") };
    assert_eq!(base.as_ref(), &Node::Identifier { name: "R".into(), variant: Variant::DoubleStruck });
    assert!(matches!(sup.as_ref(), Node::Identifier { variant: Variant::Italic, .. }), "{sup:?}");
    // rm and mathrm: upright; what has no letters (a fraction) is left as it is
    assert_eq!(first("rm x"), Node::Identifier { name: "x".into(), variant: Variant::Upright });
    assert_eq!(first("mathrm x"), first("rm x"));
    assert!(matches!(first("bb(1/2)"), Node::Fraction { .. }));
}

#[test]
fn matrices_with_the_invisible_bracket_on_either_side() {
    // { and :} are cases; {: and } the right-hand cases
    assert!(matches!(first("{:(a,b),(c,d)}"), Node::Table { shape: TableKind::RCases, .. }));
    assert_eq!(am("{:(a,b),(c,d)}").speech, latex(r"\begin{rcases} a & b \\ c & d \end{rcases}").speech);
    // brackets that pair to no matrix (a parenthesis closed by a bracket): the rows stay bracketed
    let n = first("((a,b),(c,d)]");
    assert!(matches!(&n, Node::Delimited { open: Some('('), close: Some(']'), .. }), "{n:?}");
}

#[test]
fn augmented_matrices_draw_a_rule_for_a_column_of_bars() {
    let n = first("[[a,b,|,c],[d,e,|,f]]");
    let Node::Table { shape: TableKind::BMatrix, rows, verticals, .. } = &n else { panic!("{n:?}") };
    assert_eq!(rows[0].len(), 3);
    assert_eq!(verticals, &vec![2]);
}

#[test]
fn what_looks_like_a_matrix_and_is_not() {
    for src in [
        // rows in angle brackets
        "[<<a,b>>,<<c,d>>]",
        // rows with different brackets
        "[(a,b),[c,d]]",
        // rows not separated by commas
        "[(a,b)(c,d)]",
        // a row of absolute values
        "[|a|,|b|]",
    ] {
        let n = first(src);
        assert!(matches!(n, Node::Delimited { open: Some('['), close: Some(']'), .. }), "{src}: {n:?}");
    }
}

#[test]
fn half_invisible_brackets_are_removed_around_a_script() {
    // {:a+b) and (a+b:} are bracketed expressions with one side invisible; as a script their brackets go
    assert_eq!(am("x^{:a+b)").tree, latex("x^{a+b}").tree);
    assert_eq!(am("x^(a+b:}").tree, latex("x^{a+b}").tree);
    assert_eq!(am("x^[a+b]").tree, latex("x^{a+b}").tree);
    assert_eq!(am("x^{a+b}").tree, latex("x^{a+b}").tree);
}
