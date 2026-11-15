//! LaTeX reader (input/latex.rs), the corners the other suites leave out: words in `\mathit`,
//! `\textbf` around inline math, `\phase` styles, glue registers as measures, siunitx's repeated
//! per-mode, mhchem's states and arrow labels, the tensor package's indices, physics' matrices,
//! brackets, function powers and `\qty`, math commands written in text, `multline`'s alignments,
//! HTML attributes, TeX's `\char`, `\Newextarrow`, paired delimiters, comments in verbatim arguments
//! and the Unicode superscripts and subscripts written after `^` and `_`.

use context_equation_core::input::latex::read_with;
use context_equation_core::rules::Registry;
use context_equation_core::{Error, Locale, Node};

fn read_in(tex: &str, strict: bool) -> (Result<Node, Error>, Registry) {
    let mut registry = Registry::default();
    let r = read_with(tex, Locale::En, &mut registry, strict);
    (r, registry)
}

/// Tolerant mode's tree (as Debug, to search for pieces) and the warnings.
fn tolerant(tex: &str) -> (String, Vec<String>) {
    let (r, registry) = read_in(tex, false);
    let node = r.unwrap_or_else(|e| panic!("{tex}: {e}"));
    (format!("{node:?}"), registry.warnings.iter().map(|a| format!("{}: {}", a.rule, a.description)).collect())
}

/// Tree of a valid input: read the same in both modes, with no warnings.
fn tree(tex: &str) -> String {
    let (strict, _) = read_in(tex, true);
    let strict = strict.unwrap_or_else(|e| panic!("{tex}: {e}"));
    let (t, warnings) = tolerant(tex);
    assert!(warnings.is_empty(), "{tex}: {warnings:?}");
    assert_eq!(format!("{strict:?}"), t, "{tex}");
    t
}

/// Error message in strict mode.
fn strict_error(tex: &str) -> String {
    match read_in(tex, true).0 {
        Ok(node) => panic!("{tex}: strict mode accepted: {node:?}"),
        Err(e) => e.to_string(),
    }
}

/// Error message, which also happens in tolerant mode.
fn always_error(tex: &str) -> String {
    let strict = strict_error(tex);
    match read_in(tex, false).0 {
        Ok(node) => panic!("{tex}: tolerant mode accepted: {node:?}"),
        Err(e) => assert_eq!(e.to_string(), strict, "{tex}"),
    }
    strict
}

/// Invalid: strict mode refuses, tolerant mode draws with a warning from the expected rule.
fn recovers(tex: &str, rule: &str) -> String {
    strict_error(tex);
    let (t, a) = tolerant(tex);
    assert!(a.iter().any(|a| a.starts_with(rule)), "{tex}: expected {rule}, got {a:?}");
    t
}

fn has(t: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(t.contains(snippet), "missing {snippet:?} in {t}");
    }
}

// ---------- fonts and text ----------

#[test]
fn mathit_joins_only_neighbor_letters_of_the_same_variant_into_a_word() {
    has(&tree(r"\mathit{ab}"), &[r#"Identifier { name: "ab", variant: Italic }"#]);
    // an upright letter inside is not part of the italic word
    let t = tree(r"\mathit{a\mathrm{b}}");
    has(&t, &[r#"Identifier { name: "a", variant: Italic }"#, r#"Identifier { name: "b", variant: Upright }"#]);
}

#[test]
fn a_double_struck_number_that_starts_with_a_point_is_a_plain_number() {
    // only a digit is the double-struck symbol (𝟙); ".5" stays a number
    has(&tree(r"\mathbb{.5}"), &[r#"Number { value: ".5""#]);
    has(&tree(r"\mathbb{1.5}"), &[r#"Identifier { name: "1", variant: DoubleStruck }"#, r#"Number { value: ".5""#]);
}

#[test]
fn bold_text_marks_its_words_and_leaves_its_math_alone() {
    let t = tree(r"\textbf{a $x$ b}");
    has(&t, &[r#"Text("\u{e000}a \u{e001}")"#, r#"Math(Identifier { name: "x", variant: Italic })"#, r#"Text("\u{e000} b\u{e001}")"#]);
}

#[test]
fn cross_references_inside_text_become_its_text() {
    // \label shows nothing, \ref its number (?? when unknown)
    has(&tree(r"\text{a\label{x}b}"), &[r#"Text("ab")"#]);
    has(&recovers(r"\text{a\ref{x}b}", "E-REC-9"), &[r#"Text("a??b")"#]);
}

#[test]
fn math_commands_in_text_take_their_arguments_as_tex_does() {
    // spaces before the argument, an unbraced command as an argument, braces inside the optional
    // argument and inside a group
    for (tex, expected) in [
        (r"\text{\frac {a}{b}}", r#"Fraction { num: Identifier { name: "a", variant: Italic }, den: Identifier { name: "b", variant: Italic }"#),
        (r"\text{\frac\alpha b}", r#"Fraction { num: Identifier { name: "α", variant: Italic }, den: Identifier { name: "b", variant: Italic }"#),
        (r"\text{\sqrt[{3}]{x}}", r#"Root { radicand: Identifier { name: "x", variant: Italic }, index: Some(Number { value: "3""#),
        (r"\text{\frac{a{b}}{c}}", r#"den: Identifier { name: "c", variant: Italic }"#),
    ] {
        has(&recovers(tex, "E-REC-1"), &[expected]);
    }
    // an unclosed group is closed at the end of the text, whether the text's or the argument's
    for tex in [r"\text{\frac{a}{b}", r"\text{\frac{a}{b"] {
        let (t, a) = tolerant(tex);
        has(&t, &[r#"Fraction { num: Identifier { name: "a", variant: Italic }, den: Identifier { name: "b", variant: Italic }"#]);
        assert!(a.iter().any(|a| a.starts_with("E-REC-4")), "{tex}: {a:?}");
    }
    // a missing argument at the end of the text is left empty
    let (t, a) = tolerant(r"\text{\frac a}");
    has(&t, &[r#"num: Identifier { name: "a", variant: Italic }, den: Row { children: [] }"#]);
    assert!(a.iter().any(|a| a.starts_with("E-REC-7")), "{a:?}");
}

// ---------- spacing ----------

#[test]
fn phase_with_an_option_out_of_range_keeps_the_current_style() {
    has(&tree(r"\phase[0]{x}"), &["Framed { base: Style { level: 0"]);
    for tex in [r"\phase[7]{x}", r"\phase[a]{x}"] {
        has(&tree(tex), &[r#"Framed { base: Identifier { name: "x", variant: Italic }, shape: Phase"#]);
    }
}

#[test]
fn a_backslash_with_no_register_name_is_not_a_measure() {
    // \kern\, : there is no glue register, the measure is empty; the \, after it is its own space
    for tex in [r"\kern\,", r"\mskip2\,"] {
        assert!(strict_error(tex).contains("unrecognized measure"), "{tex}");
        let t = recovers(tex, "E-REC-9");
        assert_eq!(t, "Row { children: [Space { em: 0.16666667 }] }", "{tex}");
    }
}

// ---------- siunitx ----------

#[test]
fn repeated_per_mode_with_only_a_denominator_starts_with_one() {
    let t = tree(r"\sisetup{per-mode=repeated-symbol}\si{\per\second}");
    has(&t, &[r#"Number { value: "1""#, "Operator { symbol: '/', class: Ord }", r#"name: "second""#]);
}

// ---------- mhchem ----------

#[test]
fn a_state_of_aggregation_with_math_inside_is_one_entity() {
    let t = tree(r"\ce{A (aq,$\infty$)}");
    has(&t, &[r#"Text { parts: [Text("(aq,"), Math(Operator { symbol: '∞', class: Ord }), Text(")")] }"#]);
    // followed by a letter, it is not a state: its parentheses are drawn as written
    let t = tree(r"\ce{(aq,x)b}");
    has(&t, &["Operator { symbol: '(', class: Open }", r#"Text { parts: [Text("aq")] }"#, r#"Text { parts: [Text("b")] }"#]);
}

#[test]
fn an_arrow_label_honors_braces_and_needs_its_closing_bracket() {
    let t = tree(r"\ce{A ->[{x]}]B}");
    has(&t, &[r#"Arrow { symbol: '→', above: Some(Text { parts: [Text("x]")] }), below: None }"#]);
    // without ']', the bracket is not a label: it is drawn after a plain arrow
    let t = tree(r"\ce{A ->[x}");
    has(&t, &["Operator { symbol: '→', class: Rel }", "Operator { symbol: '[', class: Open }"]);
    assert!(!t.contains("Arrow"), "{t}");
}

// ---------- tensor ----------

#[test]
fn tensor_indices_skip_stray_characters_and_spaces() {
    // the stray x is dropped; the space after ^ and _ is skipped; a braced index stays one group
    let t = tree(r"\tensor{T}{x^ a _{bc}}");
    has(
        &t,
        &[
            r#"sup: Some(Identifier { name: "a", variant: Italic }), sub: None"#,
            r#"sup: None, sub: Some(Row { children: [Identifier { name: "b", variant: Italic }, Identifier { name: "c", variant: Italic }] })"#,
        ],
    );
    assert!(!t.contains(r#""x""#), "{t}");
}

#[test]
fn tensor_indices_take_commands_and_end_empty() {
    // a control symbol (\,) and a control word (\nu) are whole indices; a ^ at the end has an empty index
    let t = tree(r"\tensor{T}{^\,_\nu^}");
    has(
        &t,
        &["sup: Some(Space { em: 0.16666667 }), sub: None", r#"sup: None, sub: Some(Identifier { name: "ν", variant: Italic })"#, "sup: Some(Row { children: [] }), sub: None"],
    );
}

// ---------- physics ----------

#[test]
fn small_matrix_quantities_take_their_delimiters() {
    // \smqty*(…) between \lgroup…\rgroup, \smqty{…} with none
    has(&tree(r"\smqty*(a&b\\c&d)"), &["Delimited { open: Some('⟮'), close: Some('⟯')", "shape: SmallMatrix"]);
    let t = tree(r"\smqty{a&b\\c&d}");
    has(&t, &["Table { shape: SmallMatrix"]);
    assert!(!t.contains("Delimited"), "{t}");
    // \mqty alone is "()", and what follows it is read as written
    assert_eq!(tree(r"\mqty"), "Row { children: [Operator { symbol: '(', class: Open }, Operator { symbol: ')', class: Close }] }");
    has(&tree(r"\mqty x"), &["Operator { symbol: ')', class: Close }, Identifier { name: \"x\""]);
}

#[test]
fn diagonal_matrix_entries_keep_commas_inside_braces() {
    let t = tree(r"\pmqty{\dmat{{a,b},c}}");
    has(
        &t,
        &[
            "Table { shape: PMatrix",
            r#"[Row { children: [Identifier { name: "a", variant: Italic }, Operator { symbol: ',', class: Punct }, Identifier { name: "b", variant: Italic }] }, Row { children: [] }]"#,
        ],
    );
}

#[test]
fn pauli_matrix_by_an_unknown_name_is_left_empty() {
    assert!(strict_error(r"\pmat{5}").contains("no Pauli matrix by that name"));
    assert_eq!(recovers(r"\pmat{5}", "E-REC-9"), "Row { children: [] }");
}

#[test]
fn physics_brackets_nest_close_at_the_end_and_take_angles() {
    // the inner parentheses are part of the content
    let t = tree(r"\order((x))");
    has(&t, &["Delimited { open: Some('('), close: Some(')'), parts: [Row { children: [Operator { symbol: '(', class: Open }, Identifier { name: \"x\""]);
    // an unclosed bracket ends with the input
    has(&tree(r"\order(x"), &["Delimited { open: Some('('), close: Some(')'), parts: [Row { children: [Identifier { name: \"x\""]);
    has(&tree(r"\order<x>"), &["Delimited { open: Some('⟨'), close: Some('⟩')"]);
    // with no bracket, \order is the O alone
    assert_eq!(tree(r"\order"), r#"Row { children: [Identifier { name: "O", variant: Calligraphic }] }"#);
    assert_eq!(tree(r"\order x"), r#"Row { children: [Identifier { name: "O", variant: Calligraphic }, Identifier { name: "x", variant: Italic }] }"#);
}

#[test]
fn a_function_power_needs_parentheses_after_it() {
    has(&tree(r"\sin[2](x)"), &["Scripts { base: Function { name: \"sin\", limits: false }, sup: Some(Number { value: \"2\""]);
    // without them, the brackets are read as written
    let t = tree(r"\sin[2] x");
    has(&t, &["Operator { symbol: '[', class: Open }", "Operator { symbol: ']', class: Close }"]);
    assert!(!t.contains("Scripts"), "{t}");
    // and so is an unclosed bracket
    assert_eq!(
        tree(r"\sin[2 x"),
        r#"Row { children: [Function { name: "sin", limits: false }, Operator { symbol: '[', class: Open }, Number { value: "2", group: false, thousands: None, variant: Upright }, Identifier { name: "x", variant: Italic }] }"#
    );
}

#[test]
fn qty_followed_by_neither_bracket_nor_brace_is_siunitx() {
    // not physics' brackets: siunitx's \qty{value}{unit}, missing its arguments
    assert!(strict_error(r"\qty x").contains("missing argument"));
    has(&recovers(r"\qty x", "E-REC-7"), &["Quantity { value: None"]);
}

#[test]
fn functional_derivative_with_a_power() {
    let t = tree(r"\fdv[2]{F}{x}");
    has(
        &t,
        &[
            r#"num: Row { children: [Scripts { base: Identifier { name: "δ", variant: Italic }, sup: Some(Number { value: "2""#,
            r#"Scripts { base: Identifier { name: "x", variant: Italic }, sup: Some(Number { value: "2""#,
        ],
    );
}

// ---------- environments ----------

#[test]
fn multline_with_one_line_keeps_it_as_written() {
    let t = tree(r"\begin{multline}a\end{multline}");
    has(&t, &[r#"rows: [[Identifier { name: "a", variant: Italic }]]"#]);
    assert!(!t.contains("Multicolumn"), "{t}");
}

#[test]
fn multline_keeps_a_shoved_first_line() {
    let t = tree(r"\begin{multline}\shoveleft{a}\\b\end{multline}");
    has(
        &t,
        &[
            r#"[Multicolumn { columns: 1, alignment: 'l', base: Identifier { name: "a", variant: Italic } }]"#,
            r#"[Multicolumn { columns: 1, alignment: 'r', base: Identifier { name: "b", variant: Italic } }]"#,
        ],
    );
}

#[test]
fn multline_aligns_a_line_split_by_an_ampersand_as_one_cell() {
    // '&' has no place in multline (amsmath: "Misplaced alignment tab character &"): tolerant mode
    // joins the cells, and the joined line is pushed to the left like any first line
    let t = recovers(r"\begin{multline}a&b\\c\end{multline}", "E-REC-9");
    has(
        &t,
        &[
            r#"rows: [[Multicolumn { columns: 1, alignment: 'l', base: Row { children: [Identifier { name: "a", variant: Italic }, Identifier { name: "b", variant: Italic }] } }], [Multicolumn { columns: 1, alignment: 'r'"#,
        ],
    );
}

#[test]
fn an_ampersand_in_a_single_column_environment_is_refused_in_strict_mode() {
    for env in ["multline", "multline*", "gather", "gather*", "gathered"] {
        let tex = format!(r"\begin{{{env}}}a&b\\c&&d\end{{{env}}}");
        assert!(strict_error(&tex).contains(&format!("'&' in line 1 of {env}, which has one column")), "{tex}");
        let (t, a) = tolerant(&tex);
        assert_eq!(a.len(), 2, "{tex}: {a:?}");
        assert!(a[1].starts_with("E-REC-9") && a[1].contains(&format!("line 2 of {env}")), "{tex}: {a:?}");
        // one cell per line: the cells joined, an empty one left out
        let (ab, cd) = (
            r#"Row { children: [Identifier { name: "a", variant: Italic }, Identifier { name: "b", variant: Italic }] }"#,
            r#"Row { children: [Identifier { name: "c", variant: Italic }, Identifier { name: "d", variant: Italic }] }"#,
        );
        if env.starts_with("multline") {
            has(&t, &[&format!("rows: [[Multicolumn {{ columns: 1, alignment: 'l', base: {ab} }}], [Multicolumn {{ columns: 1, alignment: 'r', base: {cd} }}]]")]);
        } else {
            has(&t, &[&format!("rows: [[{ab}], [{cd}]]")]);
        }
    }
    // split and the aligned environments keep their '&'
    for env in ["split", "aligned", "align"] {
        tree(&format!(r"\begin{{{env}}}a&b\end{{{env}}}"));
    }
}

// ---------- HTML attributes ----------

#[test]
fn html_style_without_a_value_and_data_without_a_key_are_dropped() {
    let (t, a) = tolerant(r"\htmlStyle{color}{x}");
    assert_eq!(t, r#"Row { children: [Identifier { name: "x", variant: Italic }] }"#);
    assert!(a.iter().any(|a| a.contains("unsafe or unsupported style \"color\"")), "{a:?}");
    let (t, a) = tolerant(r"\htmlData{=a}{x}");
    assert_eq!(t, r#"Row { children: [Identifier { name: "x", variant: Italic }] }"#);
    assert!(a.iter().any(|a| a.contains("data \"=a\"")), "{a:?}");
}

// ---------- TeX primitives and definitions ----------

#[test]
fn char_takes_a_character_after_a_backquote_with_or_without_backslash() {
    for tex in [r"\char`\a", r"\char`a"] {
        assert_eq!(tree(tex), "Row { children: [Operator { symbol: 'a', class: Ord }] }", "{tex}");
    }
    for tex in [r#"\char"41"#, r"\char'101"] {
        assert_eq!(tree(tex), "Row { children: [Operator { symbol: 'A', class: Ord }] }", "{tex}");
    }
}

#[test]
fn char_without_a_valid_code_is_an_error_in_both_modes() {
    for tex in [r#"\char"ZZ"#, r"\char"] {
        assert!(always_error(tex).contains("\\char needs a character code"), "{tex}");
    }
}

#[test]
fn paired_delimiter_with_an_invalid_number_of_arguments_is_an_error() {
    let e = always_error(r"\DeclarePairedDelimiterX{\norm}[x]{\lVert}{\rVert}{#1}\norm{a}");
    assert!(e.contains("invalid number of arguments: x"), "{e}");
}

#[test]
fn newextarrow_maps_each_arrow_to_its_extensible_form() {
    for symbol in ['→', '←', '⇒', '⇐', '↔', '⇔', '⇌', '⇋', '↦', '↪', '↩', '↠', '↞', '⇀', '⇁', '↼', '↽', '⇄', '='] {
        let tex = format!(r"\Newextarrow{{\xfoo}}{{5,5,5,5}}{{{}}}\xfoo{{a}}", symbol as u32);
        let t = tree(&tex);
        has(&t, &[&format!("Arrow {{ symbol: '{symbol}', above: Some(Identifier {{ name: \"a\"")]);
    }
    // in hexadecimal too
    has(&tree(r"\Newextarrow{\xfoo}{5,5,5,5}{0x2190}\xfoo{a}"), &["Arrow { symbol: '←'"]);
}

#[test]
fn newextarrow_of_a_character_with_no_extensible_form_is_stacked() {
    assert!(strict_error(r"\Newextarrow{\xfoo}{5,5,5,5}{0x2605}\xfoo{a}").contains("U+2605 has no extensible form"));
    let t = recovers(r"\Newextarrow{\xfoo}{5,5,5,5}{0x2605}\xfoo{a}", "E-REC-9");
    has(&t, &["ForcedClass { class: Rel", "Operator { symbol: '★', class: Ord }", r#"above: Some(Identifier { name: "a", variant: Italic })"#]);
}

// ---------- comments ----------

#[test]
fn verbatim_arguments_after_spaces_keep_their_percent_sign() {
    // strict mode drops comments, but not inside \url and \href, even with a space before the brace
    has(&tree(r"\url {a%b}"), &[r#"Text("\u{e004}a%b\u{e005}")"#]);
    assert_eq!(tree(r"\href {a%b}{c}"), r#"Row { children: [Identifier { name: "c", variant: Italic }] }"#);
}

// ---------- Unicode scripts after ^ and _ ----------

#[test]
fn every_unicode_script_after_a_script_mark_is_read_as_its_plain_character() {
    let pairs: &[(&str, char)] = &[
        ("⁰₀", '0'),
        ("¹₁", '1'),
        ("²₂", '2'),
        ("³₃", '3'),
        ("⁴₄", '4'),
        ("⁵₅", '5'),
        ("⁶₆", '6'),
        ("⁷₇", '7'),
        ("⁸₈", '8'),
        ("⁹₉", '9'),
        ("⁺₊", '+'),
        ("⁻₋", '-'),
        ("⁼₌", '='),
        ("⁽₍", '('),
        ("⁾₎", ')'),
        ("ᵃₐ", 'a'),
        ("ᵇ", 'b'),
        ("ᶜ", 'c'),
        ("ᵈ", 'd'),
        ("ᵉₑ", 'e'),
        ("ᶠ", 'f'),
        ("ᵍ", 'g'),
        ("ʰₕ", 'h'),
        ("ⁱᵢ", 'i'),
        ("ʲⱼ", 'j'),
        ("ᵏₖ", 'k'),
        ("ˡₗ", 'l'),
        ("ᵐₘ", 'm'),
        ("ⁿₙ", 'n'),
        ("ᵒₒ", 'o'),
        ("ᵖₚ", 'p'),
        ("𐞥", 'q'),
        ("ʳᵣ", 'r'),
        ("ˢₛ", 's'),
        ("ᵗₜ", 't'),
        ("ᵘᵤ", 'u'),
        ("ᵛᵥ", 'v'),
        ("ʷ", 'w'),
        ("ˣₓ", 'x'),
        ("ʸ", 'y'),
        ("ᶻ", 'z'),
        ("ᴬ", 'A'),
        ("ᴮ", 'B'),
        ("ꟲ", 'C'),
        ("ᴰ", 'D'),
        ("ᴱ", 'E'),
        ("ꟳ", 'F'),
        ("ᴳ", 'G'),
        ("ᴴ", 'H'),
        ("ᴵ", 'I'),
        ("ᴶ", 'J'),
        ("ᴷ", 'K'),
        ("ᴸ", 'L'),
        ("ᴹ", 'M'),
        ("ᴺ", 'N'),
        ("ᴼ", 'O'),
        ("ᴾ", 'P'),
        ("ꟴ", 'Q'),
        ("ᴿ", 'R'),
        ("ᵀ", 'T'),
        ("ᵁ", 'U'),
        ("ⱽ", 'V'),
        ("ᵂ", 'W'),
        ("ᵝᵦ", 'β'),
        ("ᵞᵧ", 'γ'),
        ("ᵟ", 'δ'),
        ("ᶿ", 'θ'),
        ("ᵨ", 'ρ'),
        ("ᵠᵩ", 'φ'),
        ("ᵡᵪ", 'χ'),
    ];
    for (glyphs, plain) in pairs {
        for glyph in glyphs.chars() {
            for mark in ['^', '_'] {
                let tex = format!("y{mark}{glyph}");
                let kind = if matches!(glyph, '\u{2080}'..='\u{209C}' | '\u{1D62}'..='\u{1D6A}' | '\u{2C7C}') { "subscript" } else { "superscript" };
                assert!(strict_error(&tex).contains(&format!("is already a {kind}")), "{tex}");
                let (t, a) = tolerant(&tex);
                assert!(a.iter().any(|a| a.starts_with("E-REC-2")), "{tex}: {a:?}");
                assert_eq!(t, tolerant(&format!("y{mark}{plain}")).0, "{tex}");
            }
        }
    }
}
