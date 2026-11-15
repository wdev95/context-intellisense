//! Braille output (output/braille): every expected string comes from an example of the official source,
//! cited next to it. Nemeth: BANA, *The Nemeth Braille Code for Mathematics and Science Notation* (2022).
//! CMU: *Código Matemático Unificado para a Língua Portuguesa* (MEC/SEESP, 2006), as transcribed by the
//! Instituto Benjamin Constant. See spec/braille.md.

use context_equation_core::{convert, BrailleCode, Locale, Options};

/// The examples of each code in its language: the CMU's in pt-BR (decimal comma, thousands point).
fn braille(tex: &str, code: BrailleCode) -> String {
    let locale = if code == BrailleCode::Cmu { Locale::PtBr } else { Locale::En };
    let r = convert(tex, &Options { locale, braille: Some(code), strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    assert!(r.applied.iter().all(|a| a.rule != "S-BRL-1"), "{tex}: {:?}", r.applied);
    r.braille
}

fn check(code: BrailleCode, cases: &[(&str, &str, &str)]) {
    for (tex, expected, source) in cases {
        assert_eq!(braille(tex, code), *expected, "{tex} ({source})");
    }
}

#[test]
fn nemeth_numbers_and_operations() {
    check(
        BrailleCode::Nemeth,
        &[
            ("1+x+y=0", "⠼⠂⠬⠭⠬⠽⠀⠨⠅⠀⠼⠴", "Ex. 3-14"),
            ("x = -1", "⠭⠀⠨⠅⠀⠤⠼⠂", "§3.3"),
            (".35", "⠼⠨⠒⠢", "§3"),
            ("3.14", "⠼⠒⠨⠂⠲", "§3"),
            ("(0 = x)", "⠷⠴⠀⠨⠅⠀⠭⠾", "§3.4"),
            ("y = 2 \\sin x", "⠽⠀⠨⠅⠀⠼⠆⠎⠊⠝⠀⠭", "§3.3"),
            ("x \\neq y", "⠭⠀⠌⠨⠅⠀⠽", "§21.1"),
            ("a=b", "⠁⠀⠨⠅⠀⠃", "Ex. 6-31"),
        ],
    );
}

#[test]
fn nemeth_letters_and_the_english_letter_indicator() {
    check(
        BrailleCode::Nemeth,
        &[
            ("n, s", "⠰⠝⠠⠀⠰⠎", "Ex. 6-8"),
            ("\\cos A", "⠉⠕⠎⠀⠠⠁", "Ex. 6-15"),
            ("(a, 2x, b)", "⠷⠁⠠⠀⠆⠭⠠⠀⠃⠾", "Ex. 6-23, enclosed list"),
            ("\\{a, b, c, d\\}", "⠨⠷⠁⠠⠀⠃⠠⠀⠉⠠⠀⠙⠨⠾", "Ex. 6-25"),
            ("i = 1, 2, \\ldots, n", "⠊⠀⠨⠅⠀⠼⠂⠠⠀⠼⠆⠠⠀⠄⠄⠄⠠⠀⠰⠝", "Ex. 6-29"),
            ("(a, 2x, y = z)", "⠷⠰⠁⠠⠀⠼⠆⠭⠠⠀⠽⠀⠨⠅⠀⠵⠾", "Ex. 6-30"),
            ("\\{x: x > 0\\}", "⠨⠷⠰⠭⠸⠒⠀⠭⠀⠨⠂⠀⠼⠴⠨⠾", "Ex. 8-46"),
            ("[0, 1]", "⠈⠷⠴⠠⠀⠂⠈⠾", "§3.5"),
            ("\\Sigma", "⠨⠠⠎", "§6.1.4"),
            ("\\mathbb{R}", "⠠⠸⠰⠠⠗", "Ex. 7-2"),
        ],
    );
}

#[test]
fn nemeth_fractions_levels_and_radicals() {
    check(
        BrailleCode::Nemeth,
        &[
            ("\\frac{1}{3}", "⠹⠂⠌⠒⠼", "§13"),
            ("4\\frac{3}{8}", "⠼⠲⠸⠹⠒⠌⠦⠸⠼", "§13, mixed number"),
            ("\\frac{\\frac{a}{b}}{c}", "⠠⠹⠹⠁⠌⠃⠼⠠⠌⠉⠠⠼", "§13, complex fraction"),
            ("x^2+1", "⠭⠘⠆⠐⠬⠂", "§14"),
            ("x_1", "⠭⠂", "§14.6"),
            ("x_{i,j}", "⠭⠰⠊⠪⠚", "§14"),
            ("x_a^n", "⠭⠰⠁⠘⠝", "§14.11"),
            ("x'^2", "⠭⠄⠘⠆", "§14.12"),
            ("\\cos^2 x", "⠉⠕⠎⠘⠆⠀⠭", "§18"),
            ("\\log_2 x", "⠇⠕⠛⠆⠀⠭", "§14.6"),
            ("90^\\circ", "⠼⠔⠴⠘⠨⠡", "§23.10"),
            ("\\sqrt{x+y}", "⠜⠭⠬⠽⠻", "§16"),
            ("\\sqrt[3]{x+y}", "⠣⠒⠜⠭⠬⠽⠻", "§16"),
            ("\\sqrt{x^2+1}", "⠜⠭⠘⠆⠐⠬⠂⠻", "§16"),
            ("\\sqrt{x+\\sqrt{x+y}+z}", "⠜⠭⠬⠨⠜⠭⠬⠽⠨⠻⠬⠵⠻", "§16, nested"),
        ],
    );
}

#[test]
fn nemeth_modified_expressions_limits_and_integrals() {
    check(
        BrailleCode::Nemeth,
        &[
            ("\\lim_{x\\to 0} f(x)", "⠐⠇⠊⠍⠩⠭⠀⠫⠕⠀⠼⠴⠻⠀⠋⠷⠭⠾", "Ex. 15-3"),
            ("\\sum_{n=1}^{\\infty}", "⠐⠨⠠⠎⠩⠝⠀⠨⠅⠀⠼⠂⠣⠠⠿⠻", "Ex. 15-37"),
            ("\\int_a^b f(x)dx", "⠮⠰⠁⠘⠃⠐⠋⠷⠭⠾⠙⠭", "Ex. 23-16"),
            ("\\bar{x}", "⠭⠱", "§15.3, contracted bar"),
            ("\\hat{x}", "⠐⠭⠣⠸⠣⠻", "§15"),
            ("\\vec{AB}", "⠐⠠⠁⠠⠃⠣⠫⠕⠻", "§15"),
            ("\\binom{n}{k}", "⠷⠝⠩⠅⠾", "Ex. 15-43"),
        ],
    );
}

#[test]
fn nemeth_matrices_have_a_line_per_row() {
    let b = braille(r"\begin{pmatrix} 1 & a \\ 2 & b \end{pmatrix}", BrailleCode::Nemeth);
    // enlarged parentheses on every line, a numeric indicator on every number, no letter indicator (§25.8)
    assert_eq!(b, "⠠⠷⠼⠂⠀⠁⠠⠾\n⠠⠷⠼⠆⠀⠃⠠⠾");
}

#[test]
fn cmu_numbers_and_operations() {
    check(
        BrailleCode::Cmu,
        &[
            ("1.720", "⠼⠁⠄⠛⠃⠚", "§3.1"),
            ("3,2", "⠼⠉⠂⠃", "§3.2.1"),
            ("\\frac{3}{4}", "⠼⠒⠙", "§3.2.2"),
            ("2\\frac{3}{4}", "⠼⠃⠼⠒⠙", "§3.2.2"),
            ("6 \\pm 2", "⠼⠋⠖⠒⠤⠼⠃", "§4.1"),
            ("6 \\times 2", "⠼⠋⠦⠼⠃", "§4.1"),
            ("6 \\div 2", "⠼⠋⠲⠼⠃", "§4.1"),
            ("\\pi \\approx 3,1416", "⠈⠏⠈⠶⠼⠉⠂⠁⠙⠁⠋", "§4.2"),
            ("5x = 40b", "⠼⠑⠭⠶⠼⠙⠚⠐⠃", "§1.1"),
            ("x \\neq y", "⠭⠘⠶⠽", "§4.3"),
            ("n!", "⠝⠘⠄", "§7.5.1"),
        ],
    );
}

#[test]
fn cmu_fractions_powers_and_roots() {
    check(
        BrailleCode::Cmu,
        &[
            ("\\frac{a}{c}", "⠁⠲⠉", "§5.1"),
            ("a+\\frac{b}{c}", "⠁⠖⠃⠲⠉", "§5.1"),
            ("\\frac{a}{c \\cdot x}", "⠁⠲⠢⠉⠄⠭⠔", "§5.1"),
            ("\\frac{a+b}{c}", "⠢⠁⠖⠃⠔⠲⠉", "§5.1"),
            ("\\frac{a+\\frac{b}{c}}{d+e}", "⠢⠁⠖⠃⠲⠉⠔⠲⠢⠙⠖⠑⠔", "§5.1"),
            ("\\frac{\\frac{a+b}{c+d}}{x+y}", "⠢⠢⠁⠖⠃⠔⠲⠢⠉⠖⠙⠔⠔⠲⠢⠭⠖⠽⠔", "§5.1"),
            ("x^2", "⠭⠡⠼⠃", "§5.2"),
            ("x^{-1}", "⠭⠡⠤⠼⠁", "§5.2"),
            ("x^{a+b}", "⠭⠡⠢⠁⠖⠃⠔", "§5.2"),
            ("7x^3-2x^2+x+1", "⠼⠛⠭⠡⠼⠉⠤⠼⠃⠭⠡⠼⠃⠖⠭⠖⠼⠁", "§5.4"),
            ("\\sqrt{x}", "⠫⠱⠭", "§5.3"),
            ("\\sqrt[3]{x}", "⠫⠼⠉⠱⠭", "§5.3"),
            ("\\sqrt[n]{a+b}", "⠫⠝⠱⠢⠁⠖⠃⠔", "§5.3"),
            ("\\sqrt{x^2+y^2}", "⠫⠱⠢⠭⠡⠼⠃⠖⠽⠡⠼⠃⠔", "§5.4"),
            ("\\frac{x^2+1}{x^2-1}", "⠢⠭⠡⠼⠃⠖⠼⠁⠔⠲⠢⠭⠡⠼⠃⠤⠼⠁⠔", "§5.4"),
            ("x^{\\sqrt{a+5}}", "⠭⠡⠫⠱⠢⠁⠖⠼⠑⠔", "§5.4"),
        ],
    );
}

/// #285: the auxiliary parentheses ⠢…⠔ are not used around an expression already in ( ) [ ] { } (§1.3),
/// written with plain signs as with `\left…\right`. The grouping signs: ( ⠣ ) ⠜, [ ⠷ ] ⠾, { ⠐⠇ } ⠸⠂.
#[test]
fn cmu_no_auxiliary_parentheses_around_a_group() {
    check(
        BrailleCode::Cmu,
        &[
            // radicand
            ("\\sqrt{(a+b)}", "⠫⠱⠣⠁⠖⠃⠜", "§1.3"),
            ("\\sqrt{[a+b]}", "⠫⠱⠷⠁⠖⠃⠾", "§1.3"),
            ("\\sqrt{\\{a+b\\}}", "⠫⠱⠐⠇⠁⠖⠃⠸⠂", "§1.3"),
            ("\\sqrt{((a+b))}", "⠫⠱⠣⠣⠁⠖⠃⠜⠜", "§1.3"),
            // numerator and denominator
            ("\\frac{(a+b)}{c}", "⠣⠁⠖⠃⠜⠲⠉", "§1.3, §5.1"),
            ("\\frac{a}{[c+d]}", "⠁⠲⠷⠉⠖⠙⠾", "§1.3, §5.1"),
            // exponent
            ("x^{(a+b)}", "⠭⠡⠣⠁⠖⠃⠜", "§1.3, §5.2"),
            // index
            ("\\sqrt[\\{n+1\\}]{x}", "⠫⠐⠇⠝⠖⠼⠁⠸⠂⠱⠭", "§1.3, §5.3"),
            // more than one group is more than one term: the auxiliary parentheses stay
            ("\\sqrt{(a)+(b)}", "⠫⠱⠢⠣⠁⠜⠖⠣⠃⠜⠔", "§1.3"),
            ("\\sqrt{(a+b)(c+d)}", "⠫⠱⠢⠣⠁⠖⠃⠜⠣⠉⠖⠙⠜⠔", "§1.3"),
        ],
    );
}

#[test]
fn cmu_sets_functions_and_calculus() {
    check(
        BrailleCode::Cmu,
        &[
            ("x \\in A", "⠭⠣⠂⠨⠁", "§6.1"),
            ("A \\cup B", "⠨⠁⠸⠜⠨⠃", "§6.1"),
            ("\\{x, y, z\\}", "⠐⠇⠭⠂⠀⠽⠂⠀⠵⠸⠂", "§6.1"),
            ("\\mathbb{R}", "⠸⠗", "§3.5"),
            ("\\Delta", "⠘⠙", "§1.2"),
            ("f(x)", "⠋⠣⠭⠜", "§7.1"),
            ("\\lim_{x \\to c}", "⠇⠊⠍⠄⠭⠒⠂⠉⠱", "§7.2"),
            ("\\frac{d}{dx}", "⠙⠲⠢⠙⠭⠔", "§7.3"),
            ("\\sum_{i=1}^{n}", "⠘⠎⠊⠶⠼⠁⠒⠝⠱", "§7.5.1"),
            ("\\int_1^4 x^2 dx", "⠯⠼⠁⠒⠼⠙⠱⠭⠡⠼⠃⠐⠙⠭", "§7.7"),
            ("\\log_b x", "⠇⠕⠛⠄⠃⠱⠭", "§7.5.2"),
            ("\\sin^2 x", "⠎⠑⠝⠄⠡⠼⠃⠭", "§7.7"),
            ("\\vec{AB}", "⠒⠂⠢⠨⠁⠨⠃⠔", "§8.1"),
            ("\\overline{AB}", "⠈⠉⠢⠨⠁⠨⠃⠔", "§6.1"),
        ],
    );
}

#[test]
fn the_locale_chooses_the_code() {
    let code = |locale| convert("x", &Options { locale, ..Options::default() }).unwrap().braille_code;
    assert_eq!(code(Locale::PtBr), BrailleCode::Cmu);
    assert_eq!(code(Locale::Es), BrailleCode::Cmu);
    assert_eq!(code(Locale::En), BrailleCode::Nemeth);
    assert_eq!(code(Locale::Fr), BrailleCode::Nemeth);
    // the option forces either code
    let r = convert(r"\frac{3}{4}", &Options { locale: Locale::PtBr, braille: Some(BrailleCode::Nemeth), ..Options::default() }).unwrap();
    assert_eq!(r.braille, "⠹⠒⠌⠲⠼");
    assert_eq!("CMU".parse::<BrailleCode>(), Ok(BrailleCode::Cmu));
    assert!("ueb".parse::<BrailleCode>().unwrap_err().contains("unknown braille code"));
}

#[test]
fn braille_reaches_the_svg_and_the_json() {
    let r = convert(r"\frac{3}{4}", &Options { locale: Locale::PtBr, ..Options::default() }).unwrap();
    assert!(r.svg.contains(r#"<contextEquation:braille code="cmu">⠼⠒⠙</contextEquation:braille>"#), "{}", r.svg);
    let json: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(json["braille"], "⠼⠒⠙");
    assert_eq!(json["braille_code"], "cmu");
    // every input notation gives the same braille
    for (src, input) in [("3/4", context_equation_core::Input::AsciiMath), ("<math><mfrac><mn>3</mn><mn>4</mn></mfrac></math>", context_equation_core::Input::MathMl)] {
        assert_eq!(convert(src, &Options { locale: Locale::PtBr, input, ..Options::default() }).unwrap().braille, "⠼⠒⠙");
    }
}

#[test]
fn s_brl_1_a_symbol_without_a_sign_is_the_full_cell() {
    for code in BrailleCode::ALL {
        let r = convert(r"a \ltimes b", &Options { braille: Some(code), ..Options::default() }).unwrap();
        assert!(r.braille.contains('⠿'), "{code}: {}", r.braille);
        let note = r.applied.iter().find(|a| a.rule == "S-BRL-1").unwrap_or_else(|| panic!("{code}: {:?}", r.applied));
        assert!(note.description.contains('⋉'), "{}", note.description);
        // it is a note on the output, not a problem with the input
        assert!(r.warnings.is_empty());
    }
}

#[test]
fn every_construction_of_the_corpus_has_braille_in_both_codes() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    for file in ["constructions.json", "packages.json"] {
        let json = std::fs::read_to_string(format!("{dir}/{file}")).unwrap();
        for (name, tex) in serde_json::from_str::<Vec<(String, String)>>(&json).unwrap() {
            for code in BrailleCode::ALL {
                let Ok(r) = convert(&tex, &Options { braille: Some(code), ..Options::default() }) else { continue };
                assert!(r.braille.chars().all(|c| ('\u{2800}'..='\u{28FF}').contains(&c) || c == '\n'), "{code} {name}: {}", r.braille);
            }
        }
    }
}

/// Found by comparing with MathCAT (DAISY) and settled by the BANA 2022 document, which wins where
/// the two disagree (MathCAT writes ⟹ with three shafts and a Fraktur letter with ⠸⠸, for example).
#[test]
fn nemeth_signs_settled_against_the_source() {
    check(
        BrailleCode::Nemeth,
        &[
            ("f(x)|_{x=4}", "⠋⠷⠭⠾⠳⠰⠭⠀⠰⠨⠅⠀⠼⠲", "§14.8.7: the space after a comparison keeps the level"),
            ("a \\ll b", "⠁⠀⠐⠅⠈⠐⠅⠻⠀⠃", "Rule 21 table"),
            ("a \\cong b", "⠁⠀⠈⠱⠨⠅⠀⠃", "Rule 21 table, equals under tilde"),
            ("a \\approx b", "⠁⠀⠈⠱⠈⠱⠀⠃", "Rule 21 table, double tilde"),
            ("A \\supset B", "⠠⠁⠀⠸⠨⠂⠀⠠⠃", "Rule 21 table"),
            ("x \\mid x > 0", "⠭⠀⠳⠀⠭⠀⠨⠂⠀⠼⠴", "§21.7, Ex. 21-18"),
            ("p \\wedge q \\vee r", "⠏⠈⠩⠟⠈⠬⠗", "Rule 20 table"),
            ("a \\oplus b", "⠁⠫⠉⠸⠫⠬⠻⠃", "Rule 20 table"),
            ("f \\circ g", "⠋⠨⠡⠛", "Rule 20 table"),
            ("\\lfloor x \\rfloor + \\lceil y \\rceil", "⠈⠰⠷⠭⠈⠰⠾⠬⠈⠘⠷⠽⠈⠘⠾", "§19.4, Ex. 19-22"),
            ("\\therefore AB", "⠠⠡⠀⠠⠁⠠⠃", "§23.18, Ex. 23-50"),
            ("\\angle ABC", "⠫⠪⠀⠠⠁⠠⠃⠠⠉", "Rule 17, Ex. 6-18"),
            ("p \\implies q", "⠏⠀⠫⠶⠶⠕⠀⠟", "Ex. 22-19: two shaft cells"),
            ("\\mathfrak{g}", "⠸⠛", "Ex. 6-2: German letter"),
            ("\\mathsf{A}", "⠠⠨⠰⠠⠁", "Rule 2 typeforms"),
            ("\\Phi", "⠨⠠⠋", "§6.1.4: capitals have no alternative form"),
            ("\\cancel{x}", "⠪⠭⠻", "Rule 12"),
            ("\\underline{x}", "⠭⠩⠱", "§15.2.3, Ex. 15-24"),
            ("\\overbrace{x+y}", "⠐⠭⠬⠽⠣⠨⠷⠻", "Ex. 19-17"),
            ("\\underbrace{x+y}", "⠐⠭⠬⠽⠩⠨⠾⠻", "Ex. 19-18"),
            ("\\hbar", "⠈⠓", "Rule 23 table"),
            ("\\aleph", "⠠⠠⠁", "§6.1, Hebrew"),
            ("A \\Leftarrow B", "⠠⠁⠀⠫⠪⠶⠶⠀⠠⠃", "Ex. 22-20"),
            ("\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}", "⠈⠠⠷⠁⠀⠃⠈⠠⠾\n⠈⠠⠷⠉⠀⠙⠈⠠⠾", "§19.6 table: ⠈⠠⠷"),
        ],
    );
}

#[test]
fn nemeth_systems_keep_what_surrounds_them_on_the_top_line() {
    // Ex. 19-30: what precedes and follows the system is on its first line; the other rows align below
    let b = braille(r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & x \le 0 \end{cases}", BrailleCode::Nemeth);
    assert_eq!(b, "⠋⠷⠭⠾⠀⠨⠅⠀⠨⠠⠷⠼⠂⠀⠭⠀⠨⠂⠀⠼⠴\n⠀⠀⠀⠀⠀⠀⠀⠀⠨⠠⠷⠼⠴⠀⠭⠀⠐⠅⠱⠀⠼⠴");
    let b = braille(r"\begin{rcases} a \\ b \end{rcases} \Rightarrow c", BrailleCode::Nemeth);
    assert_eq!(b, "⠁⠨⠠⠾⠀⠫⠶⠶⠕⠀⠉\n⠃⠨⠠⠾");
}

#[test]
fn cmu_signs_settled_against_the_source() {
    check(
        BrailleCode::Cmu,
        &[
            ("{}^{14}_{6}\\mathrm{C}", "⠨⠉⠠⠌⠼⠋⠈⠡⠼⠁⠙", "§2.2: left indices after the letter"),
            ("\\overset{x}{=}", "⠶⠡⠡⠭", "§2.2: sobrescrito"),
            ("\\underset{x}{=}", "⠶⠌⠌⠭", "§2.2: subscrito"),
            ("1, 2, \\ldots, n", "⠼⠁⠂⠀⠼⠃⠂⠀⠄⠄⠄⠂⠀⠝", "§3.2.1 and §6.1"),
            ("f \\circ g", "⠋⠠⠆⠛", "§7.1: composição"),
            ("a \\therefore b", "⠁⠀⠠⠡⠀⠃", "§6.3: portanto"),
            ("a \\because b", "⠁⠀⠈⠌⠀⠃", "§6.3: posto que"),
            ("\\angle ABC", "⠘⠒⠢⠨⠁⠨⠃⠨⠉⠔", "§8.1: ângulo"),
            ("\\triangle ABC", "⠠⠾⠨⠁⠨⠃⠨⠉", "§8.1: triângulo"),
            ("p \\wedge q", "⠏⠰⠂⠟", "§6.2: conjunção"),
            ("\\neg p", "⠠⠄⠏", "§6.2: negação"),
            ("\\aleph", "⠠⠳", "§6.1: alef"),
            ("x \\sim y", "⠭⠐⠢⠄⠽", "§6.1: equivale a"),
            ("\\nabla f", "⠈⠻⠋", "§7.3"),
        ],
    );
}

#[test]
fn cmu_variants_limits_and_signs_it_lacks() {
    check(
        BrailleCode::Cmu,
        &[
            ("\\mathcal{L}", "⠰⠇", "§1.1: ⠰ before a capital in another variant, no ⠨"),
            ("\\mathfrak{g}", "⠠⠛", "§1.1: ⠠ before a lowercase letter in another variant"),
            ("\\mathbb{C}", "⠸⠉", "§3.5: number sets"),
            ("\\max_{x} f(x)", "⠍⠁⠭⠄⠌⠌⠭⠋⠣⠭⠜", "§2.1, §2.2: a limit directly under the name"),
        ],
    );
    // the 2006 code has no sign for these: the full cell and S-BRL-1, not a silent omission
    for tex in [r"\cancel{x}", r"\boxed{x}", r"\overbrace{a+b}", r"\hat{x}"] {
        let r = convert(tex, &Options { locale: Locale::PtBr, ..Options::default() }).unwrap();
        assert!(r.braille.contains('⠿'), "{tex}: {}", r.braille);
        assert!(r.applied.iter().any(|a| a.rule == "S-BRL-1"), "{tex}");
    }
    // the Feynman slash is not a cancellation, and neither code has an overlay sign for it: the letter,
    // then the full cell with the note
    for locale in [Locale::PtBr, Locale::En] {
        let r = convert(r"\slashed{D}", &Options { locale, ..Options::default() }).unwrap();
        assert!(r.braille.ends_with('⠿') && !r.braille.contains('⠪'), "{locale}: {}", r.braille);
        assert!(r.applied.iter().any(|a| a.rule == "S-BRL-1" && a.description.contains("U+0338")), "{locale}: {:?}", r.applied);
    }
}

#[test]
fn signs_found_by_the_live_list_on_the_reference_page() {
    check(
        BrailleCode::Nemeth,
        &[
            ("A \\backslash B", "⠠⠁⠸⠡⠠⠃", "Rule 20 table"),
            ("\\overarc{AB}", "⠐⠠⠁⠠⠃⠣⠫⠁⠻", "Ex. 15-54"),
            ("\\ce{A <=>[K] B}", "⠠⠁⠀⠐⠫⠒⠒⠕⠫⠪⠒⠒⠣⠠⠅⠻⠀⠠⠃", "Rule 21 table: right over left arrow, modified"),
        ],
    );
    // a range is drawn with an en dash where the font cannot write the language's "to" (L-UNI-2)
    let r = convert("\\SIrange{1}{10}{m}", &Options { locale: Locale::Ja, braille: Some(BrailleCode::Nemeth), strict: true, ..Options::default() }).unwrap();
    assert_eq!(r.braille, "⠼⠂⠀⠍⠀⠤⠤⠀⠼⠂⠴⠀⠍", "Rule 8 dash, §20.1.1, §6.3.1 c");
    check(BrailleCode::Cmu, &[("A \\backslash B", "⠨⠁⠐⠄⠨⠃", "§6.1: diferença de conjuntos"), ("x \\nsim y", "⠭⠘⠐⠢⠄⠽", "§4.3")]);
}
