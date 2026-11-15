//! Tolerant mode (spec/tolerance.md): invalid LaTeX becomes a drawing with a warning; strict mode refuses.

use context_equation_core::{convert, Locale, Options};

fn tolerant(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { locale: Locale::PtBr, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn warning_rules(tex: &str) -> Vec<&'static str> {
    tolerant(tex).warnings.iter().map(|a| a.rule).collect()
}

fn strict(tex: &str) -> Result<context_equation_core::Output, context_equation_core::Error> {
    convert(tex, &Options { locale: Locale::PtBr, strict: true, ..Default::default() })
}

#[test]
fn t_tol_valid_equation_same_in_both_modes() {
    for tex in [r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}", r"\SI{9,8}{\meter}"] {
        let a = tolerant(tex);
        let b = strict(tex).unwrap();
        assert_eq!(a.svg, b.svg, "{tex}");
        assert!(a.warnings.is_empty(), "{tex}: {:?}", a.warnings);
    }
}

#[test]
fn e_rec_1_unknown_command() {
    // #209: the name is drawn whole, never cut into a known command and letters
    assert_eq!(tolerant(r"\vertA\vert = 1").text, r"\vertA| = 1");
    assert!(tolerant(r"\foobar + 1").svg.contains(r##"fill="#c0392b""##));
    assert!(strict(r"\foobar").is_err());
}

#[test]
fn e_rec_2_json_escapes() {
    assert_eq!(tolerant(r"a²+b").text, "a² + b");
    assert_eq!(warning_rules(r"x\u00b2\nb) y"), vec!["E-REC-2", "E-REC-2"]);
}

#[test]
fn e_rec_3_double_script_in_chemistry() {
    let r = tolerant(r"\text{HSO}_4^-_{(aq)}");
    assert_eq!(warning_rules(r"\text{HSO}_4^-_{(aq)}"), vec!["E-REC-3"]);
    assert!(r.text.contains("HSO_4^−"), "{}", r.text);
}

#[test]
fn e_rec_4_and_5_unpaired_braces_and_delimiters() {
    assert_eq!(tolerant(r"x = \frac{1}{2").text, "x = 1/2");
    assert_eq!(tolerant(r"a } + b").text, "a + b");
    assert_eq!(tolerant(r"\left( x + 1").text, "(x + 1");
    assert_eq!(warning_rules(r"\left( x"), vec!["E-REC-5"]);
}

#[test]
fn e_rec_6_unknown_environment() {
    assert_eq!(tolerant(r"\begin{foo} a & b \end{foo}").text, "a, b");
}

#[test]
fn e_rec_7_and_9_missing_argument_and_dollar_in_text() {
    assert_eq!(tolerant(r"\xrightleftharpoons").text, "⇌");
    assert_eq!(tolerant(r"15 + 15 = \text{R$ } 30,00").text, "15 + 15 = R$ 30,00");
}

#[test]
fn bordermatrix() {
    let r = tolerant(r"\bordermatrix{ & c_1 & c_2 \cr r_1 & a & b \cr r_2 & c & d}");
    assert!(r.warnings.is_empty());
    assert!(r.speech.starts_with("matriz 3 por 3"), "{}", r.speech);
}

#[test]
fn backslash_before_unicode_char_does_not_panic() {
    // regression: the prefix cut of \vertA (removed in #209) used to cut '∠' in the middle of its bytes
    for tex in [r"\∠ABC", r"\é", r"\π r^2", r"\—"] {
        tolerant(tex);
    }
}

#[test]
fn e_rec_2_command_eaten_by_json() {
    // "\frac" in JSON without escaping becomes form feed + "rac"
    assert_eq!(tolerant("\u{0C}rac{1}{2} \times 3").text, "1/2 × 3");
    assert_eq!(tolerant("\u{08}egin{pmatrix} 1 \\\\ 2 \\end{pmatrix}").text, "([1], [2])");
    // a real tab is still a space
    assert_eq!(tolerant("a\tb").text, "ab");
}

#[test]
fn e_rec_2_doubled_backslashes() {
    assert_eq!(tolerant(r"y = 17,9 \\cdot 2,5").text, "y = 17,9 · 2,5");
    // with any single-backslash command, the doubles are real line breaks
    assert!(tolerant(r"\begin{cases} a \\ b \end{cases}").warnings.is_empty());
}

/// These inputs used to stop tolerant mode with an error and no SVG at all, against "draw what it can".
#[test]
fn units_angles_and_physics_qty_never_stop_the_drawing() {
    for tex in [r"\qty(\frac{a}{b})", r"\SI{1}{\angstrom}", r"\ang{12;30;15}", r"\qty[x]", r"\qty|x|"] {
        let r = tolerant(tex);
        assert!(r.warnings.is_empty(), "{tex}: {:?}", r.warnings);
        assert!(strict(tex).is_ok(), "{tex}");
        assert!(r.svg.contains("<use"), "{tex}: nothing drawn");
    }
    // what is still unknown is drawn as written, with a warning, instead of refused
    let r = tolerant(r"\SI{3}{\furlong}");
    assert_eq!(r.text, "3 furlong");
    assert_eq!(warning_rules(r"\SI{3}{\furlong}"), vec!["E-REC-1"]);
    assert!(strict(r"\SI{3}{\furlong}").is_err());
    assert_eq!(tolerant(r"\SI{x}{\meter}").text, "x m");
    assert_eq!(warning_rules(r"\SI{x}{\meter}"), vec!["E-REC-9"]);
    assert_eq!(warning_rules(r"\ang{1;2;3;4}"), vec!["E-REC-9"]);
    assert_eq!(tolerant(r"\num{1.2.3}").text, "1,2,3");
}

#[test]
fn e_rec_1_unknown_negation_is_drawn_whole() {
    // #209: amssymb has no \nlesssim; it is drawn whole in red, not read as \not\lesssim nor as a line
    // break and "lesssim"
    let r = tolerant(r"a \nlesssim b");
    assert_eq!(warning_rules(r"a \nlesssim b"), vec!["E-REC-1"]);
    assert_eq!(r.text, r"a\nlesssimb");
    assert!(strict(r"a \nlesssim b").is_err());
    // the existing negations are not recovered: they are known, with no warning
    assert!(tolerant(r"a \nleq b \ngeq c \nsim d").warnings.is_empty());
}

#[test]
fn e_rec_9_unknown_character_and_bond_in_ce() {
    assert_eq!(warning_rules(r"\ce{H☃}"), vec!["E-REC-9"]);
    assert_eq!(warning_rules(r"\ce{A \bond{?} B}"), vec!["E-REC-9"]);
    assert!(strict(r"\ce{A \bond{?} B}").is_err());
}

#[test]
fn e_rec_10_unknown_mathml() {
    let mathml = |src: &str, strict: bool| convert(src, &Options { input: context_equation_core::Input::MathMl, strict, ..Default::default() });
    let r = mathml("<math><mi>x</mi><mfoo><mn>1</mn></mfoo></math>", false).unwrap();
    assert_eq!(r.text, "x1");
    assert_eq!(r.warnings.iter().map(|a| a.rule).collect::<Vec<_>>(), vec!["E-REC-10"]);
    assert!(mathml("<math><mi>x</mi><mfoo><mn>1</mn></mfoo></math>", true).is_err());
    // the XML itself must be well formed in both modes: there is no tree to recover from
    assert!(mathml("<math><mi>x</math>", false).is_err());
}

#[test]
fn e_rec_9_stray_dollar_in_the_middle() {
    assert_eq!(tolerant(r"a + b $ = c").text, "a + b = c");
    assert_eq!(warning_rules(r"a + b $ = c"), vec!["E-REC-9"]);
    assert!(strict(r"a + b $ = c").is_err());
    // the outer delimiters are L-ENT-2, not a stray dollar
    for tex in [r"$a + b = c$", r"$$a + b = c$$"] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
        assert_eq!(strict(tex).unwrap().text, "a + b = c");
    }
}

#[test]
fn e_rec_3_script_mark_as_the_argument_of_a_script() {
    // TeX: "Missing { inserted"
    for tex in [r"y__k", r"x^^2", r"x_^2", r"x^ _2"] {
        assert_eq!(warning_rules(tex), vec!["E-REC-3"], "{tex}");
        assert!(strict(tex).is_err(), "{tex}");
    }
    // the drawing is the one from before: the mark is the script
    assert_eq!(tolerant(r"y__k").text, "y__k");
    // what was valid stays valid
    for tex in [r"y_{_k}", r"x_1^2", r"x^2_1", r"{}_a^b"] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
        assert!(strict(tex).is_ok(), "{tex}");
    }
}

#[test]
fn e_rec_2_unicode_script_after_a_script_mark() {
    // the glyph is read as its plain character: the same tree as the LaTeX the author meant
    for (pasted, meant) in [
        (r"A = 9\,\text{cm}^²", r"A = 9\,\text{cm}^2"),
        (r"a_₂ + a_₃", r"a_2 + a_3"),
        (r"x^ⁿ", r"x^n"),
        (r"x_ᵢ", r"x_i"),
        (r"A^⁻¹", r"A^{-1}"),
        (r"10^⁻³", r"10^{-3}"),
        (r"x^⁽²⁾", r"x^{(2)}"),
        (r"e^ˣ", r"e^x"),
        (r"x_₁^²", r"x_1^2"),
    ] {
        let r = tolerant(pasted);
        assert_eq!(r.tree.to_json(), tolerant(meant).tree.to_json(), "{pasted}");
        assert_eq!(warning_rules(pasted).iter().filter(|w| **w == "E-REC-2").count(), pasted.matches(['^', '_']).count(), "{pasted}");
        assert!(strict(pasted).is_err(), "{pasted}");
    }
    // a run stops where the kind changes: ^²₁ is ^2 and a subscript glyph after it
    assert_eq!(tolerant(r"x^²₁").tree.to_json(), tolerant(r"x^2₁").tree.to_json());
    // without ^ or _ the glyph stays as it was, in both modes
    for tex in [r"x²", r"a₂ + b"] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
        assert_eq!(strict(tex).unwrap().tree.to_json(), tolerant(tex).tree.to_json(), "{tex}");
    }
    // the ordinal indicators are not script characters
    assert!(!warning_rules(r"30^º").contains(&"E-REC-2"));
}

#[test]
fn e_rec_9_misplaced_and_extra_alignment_tabs() {
    // TeX: "Misplaced alignment tab character &"
    assert_eq!(tolerant(r"u & v").text, "u v");
    assert_eq!(warning_rules(r"u & v"), vec!["E-REC-9"]);
    assert!(strict(r"u & v").is_err());
    // TeX: "Extra alignment tab has been changed to \cr"; the extra cell keeps the default alignment
    let extra = r"\begin{array}{cc} 1 & 2 & 3 \end{array}";
    assert_eq!(tolerant(extra).text, "1, 2, 3");
    assert_eq!(warning_rules(extra), vec!["E-REC-9"]);
    assert!(strict(extra).is_err());
    assert!(strict(r"\begin{tabular}{l} a & b \end{tabular}").is_err());
    // what fits the declared columns stays valid, \multicolumn counting the columns it spans
    for tex in [
        r"\begin{array}{cc} 1 & 2 \\ 3 & 4 \end{array}",
        r"\begin{array}{cc} 1 \\ 3 & 4 \\ \end{array}",
        r"\begin{array}{cc} \multicolumn{2}{c}{x} \\ 1 & 2 \end{array}",
        r"\begin{array}{*{3}{c}} 1 & 2 & 3 \end{array}",
        r"\begin{pmatrix} 1 & 2 & 3 \end{pmatrix}",
        r"\begin{aligned} a &= b \\ c &= d \end{aligned}",
    ] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}: {:?}", tolerant(tex).warnings);
        assert!(strict(tex).is_ok(), "{tex}");
    }
    assert!(strict(r"\begin{array}{cc} \multicolumn{2}{c}{x} & 3 \end{array}").is_err());
}

fn with_numbers(strict: bool) -> Options {
    let labels = [("eq:a", "3"), ("eq:b", "4a")].map(|(k, v)| (k.to_string(), v.to_string())).into();
    let citations = [("knuth", "1"), ("einstein1905", "2")].map(|(k, v)| (k.to_string(), v.to_string())).into();
    Options { strict, labels, citations, ..Default::default() }
}

#[test]
fn l_ent_5_references_take_the_numbers_of_the_document() {
    for strict in [false, true] {
        let r = convert(r"\text{by } \eqref{eq:a}, \quad c = \ref{eq:b}", &with_numbers(strict)).unwrap();
        assert_eq!(r.text, "by (3), c = 4a");
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        // speech and braille say the number, never the key
        assert!(r.speech.contains("(3)") && !r.speech.contains("eq:a"), "{}", r.speech);
        assert!(!r.braille.contains('⠿'), "{}", r.braille);
        let r = convert(r"E = mc^2 \quad \text{\cite{einstein1905}} \cite{knuth,einstein1905} \cite[p.~5]{knuth}", &with_numbers(strict)).unwrap();
        assert_eq!(r.text, "E = mc^2 [2][1, 2][1, p. 5]");
    }
}

#[test]
fn e_rec_9_unknown_label_or_citation() {
    // as on LaTeX's first run: ?? for a label, ? for a citation
    for (tex, text) in [(r"\eqref{eq:z}", "(??)"), (r"\ref{eq:z}", "??"), (r"\cite{nobody}", "[?]"), (r"\cite{knuth,nobody}", "[1, ?]"), (r"\text{see \ref{eq:z}}", "see ??")] {
        let r = convert(tex, &with_numbers(false)).unwrap();
        assert_eq!(r.text, text, "{tex}");
        assert_eq!(r.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), vec!["E-REC-9"], "{tex}");
        assert!(!r.speech.contains("eq:z") && !r.speech.contains("nobody"), "{tex}: {}", r.speech);
        assert!(convert(tex, &with_numbers(true)).is_err(), "{tex}");
    }
}

#[test]
fn l_ent_5_a_tag_numbers_the_labels_of_its_line() {
    let r = strict(r"a = b \label{eq:a} \tag{3}").unwrap();
    assert_eq!(r.labels.get("eq:a").map(String::as_str), Some("3"));
    assert_eq!(strict(r"x \label{k} \tag*{A} \quad \ref{k}").unwrap().labels.get("k").map(String::as_str), Some("A"));
    // per line in a numbered environment; a \label with no \tag has no number
    let r = strict(r"\begin{align} x &= 1 \label{one} \tag{4} \\ y &= 2 \label{two} \end{align} \eqref{one}").unwrap();
    assert_eq!(r.labels.len(), 1, "{:?}", r.labels);
    assert!(r.text.ends_with("(4)"), "{}", r.text);
    let r = strict(r"\begin{equation} x = 1 \label{e1} \tag{5} \end{equation} \eqref{e1}").unwrap();
    assert!(r.text.ends_with("(5)"), "{}", r.text);
    // the \\ inside cases does not end the numbered line
    let r = strict(r"\label{c} f = \begin{cases} a \\ b \end{cases} \tag{6}").unwrap();
    assert_eq!(r.labels.get("c").map(String::as_str), Some("6"));
    assert!(strict(r"x \label{no-tag}").unwrap().labels.is_empty());
}

#[test]
fn e_rec_9_percent_starts_a_comment() {
    // TeX: '%' drops the rest of the line, the line break and the next line's leading spaces
    assert_eq!(strict(r"p = 15% \cdot q").unwrap().text, "p = 15");
    assert_eq!(strict("a + b % the sum\n    + c").unwrap().text, "a + b + c");
    assert!(strict("a + b % the sum\n    + c").unwrap().warnings.is_empty());
    // tolerant mode keeps the percent sign the author most likely meant, with a warning
    assert_eq!(tolerant(r"p = 15% \cdot q").text, "p = 15% · q");
    assert_eq!(warning_rules(r"p = 15% \cdot q"), vec!["E-REC-9"]);
    // one warning per comment TeX would see: the second '%' of a line is already inside the first's
    assert_eq!(warning_rules(r"10% + 20%"), vec!["E-REC-9"]);
    assert_eq!(warning_rules("10% +\n20%"), vec!["E-REC-9", "E-REC-9"]);
    // escaped, it is a percent sign in both modes, with no warning
    for tex in [r"15\% \cdot q", r"\text{15\%}", r"a \\ b"] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
        assert!(strict(tex).is_ok(), "{tex}");
    }
    assert_eq!(strict(r"15\% \cdot q").unwrap().text, "15% · q");
    // after an escaped backslash (a line break), '%' is a comment again
    assert_eq!(strict(r"a \\% b").unwrap().text, strict(r"a \\").unwrap().text);
    // verbatim arguments keep their '%', as in TeX
    for tex in [r"\url{https://example.org/a%20b}", r"\verb|5%|"] {
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
        assert_eq!(strict(tex).unwrap().text, tolerant(tex).text, "{tex}");
    }
}

/// #206: plain TeX's math glue registers are measures (3mu, 4mu, 5mu; the stretch ignored), in `\mskip`,
/// `\hskip` and `\mspace`, alone or times a factor.
#[test]
fn muskip_registers_are_measures() {
    let space = |tex: &str| match strict(tex).unwrap_or_else(|e| panic!("{tex}: {e}")).tree.children() {
        [_, context_equation_core::Node::Space { em }, _] => *em,
        other => panic!("{tex}: {other:?}"),
    };
    for (tex, mu) in [
        (r"a \mskip\thinmuskip b", 3.0),
        (r"a \mskip\medmuskip b", 4.0),
        (r"a \mskip\thickmuskip b", 5.0),
        (r"a \hskip\thinmuskip b", 3.0),
        (r"a \mspace{\medmuskip} b", 4.0),
        (r"a \mskip 2\thinmuskip b", 6.0),
        (r"a \mskip-\thickmuskip b", -5.0),
    ] {
        assert!((space(tex) - mu / 18.0).abs() < 1e-6, "{tex}: {}", space(tex));
        assert!(tolerant(tex).warnings.is_empty(), "{tex}");
    }
    // the same space as \, \: and \;
    assert_eq!(strict(r"a \mskip\thinmuskip b").unwrap().tree, strict(r"a \, b").unwrap().tree);
    let out = tolerant(r"a \mskip\medmuskip b \mskip\thickmuskip c");
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
}

/// #206: a measure contextEquation cannot read is no space with a warning (E-REC-9), never a failure in tolerant
/// mode; strict mode refuses it.
#[test]
fn an_unknown_measure_is_no_space_in_tolerant_mode() {
    for tex in [r"a \mskip\foo b", r"a \mskip 3furlong b", r"a \hskip\tabcolsep b", r"a \mspace{2qq} b", r"a \hspace{x} b"] {
        assert_eq!(warning_rules(tex), ["E-REC-9"], "{tex}");
        assert_eq!(tolerant(tex).text, "ab", "{tex}");
        assert!(strict(tex).unwrap_err().to_string().contains("unrecognized measure"), "{tex}");
    }
}

/// #206: the registers alone are known commands (not split as `\th inmuskip`): an assignment contextEquation does
/// not honor, ignored with a warning in tolerant mode; strict mode refuses it.
#[test]
fn a_muskip_register_alone_is_an_ignored_assignment() {
    for (tex, text) in [(r"\thinmuskip", ""), (r"\medmuskip a", "a"), (r"\thickmuskip=2mu plus 1mu a", "a"), (r"\thinmuskip = 4mu a", "a")] {
        let out = tolerant(tex);
        assert_eq!(out.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), ["E-REC-9"], "{tex}");
        assert_eq!(out.text, text, "{tex}");
        assert!(strict(tex).is_err(), "{tex}");
    }
}

/// #209: an unknown command is drawn whole, in red, with E-REC-1: never cut into a known command (KaTeX's
/// short aliases `\ch`, `\sub`, `\sect` made nonsense of ordinary names) and letters; strict refuses it.
#[test]
fn unknown_commands_are_never_cut_into_a_known_one_and_letters() {
    for (tex, text) in [
        // (#214 made the issue's \charge, \chemleft, \chemup and \chemskipalign real chemfig commands;
        // names that start with \ch and are still unknown stand in for them)
        (r"\charged{X}", r"\chargedX"),
        (r"\chembond( x \chemrel)", r"\chembond(x\chemrel)"),
        (r"\chemtext", r"\chemtext"),
        (r"\subsection", r"\subsection"),
        (r"\section", r"\section"),
        (r"\nelement", r"\nelement"),
        (r"\expandafter", r"\expandafter"),
        (r"\bigbreak", r"\bigbreak"),
        (r"\vertA", r"\vertA"),
        (r"\alphax", r"\alphax"),
    ] {
        let out = tolerant(tex);
        assert_eq!(out.text, text, "{tex}");
        assert!(out.warnings.iter().all(|w| w.rule == "E-REC-1" && w.description.contains("drawn in red")), "{tex}: {:?}", out.warnings);
        assert!(out.svg.contains(r##"fill="#c0392b""##), "{tex}");
        assert!(strict(tex).unwrap_err().to_string().contains("unsupported command"), "{tex}");
    }
    // the E-REC-1 recoveries that are not a cut stay: a backslash before a symbol
    assert_eq!(tolerant(r"\5 + 1").text, "5 + 1");
}

/// #208: a real line break or tab is whitespace, as in TeX. The JSON readings (a control character in
/// place of the backslash, `\n` + letters) apply only when the input looks JSON-decoded, and a lost
/// backslash never rebuilds one of KaTeX's texvc aliases (`\th`).
#[test]
fn real_line_breaks_and_tabs_are_whitespace() {
    for (tex, meant) in [
        // a command kept its \t: JSON would have eaten it, so the line break is real (not \nu)
        ("a \\to\nu b", r"a \to u b"),
        // indentation (not \th)
        ("a,\n\t\th(x)=1", "a, h(x)=1"),
        // a line break that ends a line, before a word (not \ne xhaust)
        ("a, b, and\nexhaust", "a, b, and exhaust"),
        ("a\n  \\neq b", r"a \neq b"),
    ] {
        let out = tolerant(tex);
        assert!(out.warnings.is_empty(), "{tex:?}: {:?}", out.warnings);
        assert_eq!(out.tree, tolerant(meant).tree, "{tex:?}");
        assert_eq!(strict(tex).unwrap().tree, out.tree, "{tex:?}");
    }
    // the literal escape is read only without real whitespace (a JSON string has none) and with another
    // sign of JSON (#253)
    assert_eq!(warning_rules(r"x\u00b2\nb) y"), ["E-REC-2", "E-REC-2"]);
    assert_eq!(warning_rules("x\u{07}lpha\\nb)\ny"), ["E-REC-2", "E-REC-1"]);
    assert_eq!(warning_rules(r"x\nb) y"), ["E-REC-1"]);
    // a tab before "h" never rebuilds KaTeX's \th, even when the rest looks JSON-decoded
    assert!(!tolerant("x\u{0C}rac{1}{2}\th").text.contains("tanh"));
}

/// #208: JSON-decoded text still has its commands rebuilt: every line break or tab stands right before
/// the letters of a known command.
#[test]
fn commands_eaten_by_json_are_still_rebuilt() {
    // (#253: a tab or a line break is rebuilt only when the input shows another sign of JSON, here the form
    // feed of \frac)
    for (tex, meant) in [
        ("\u{0C}rac{1}{2} \neq b", r"\frac{1}{2} \neq b"),
        ("\u{0C}rac{1}{2} \times 3", r"\frac{1}{2} \times 3"),
        ("\u{0C}rac{1}{2} \to y\n", r"\frac{1}{2} \to y"),
        ("\u{0C}rac{1}{2} \\left( x \right)", r"\frac{1}{2} \left( x \right)"),
    ] {
        let out = tolerant(tex);
        assert_eq!(out.warnings.iter().map(|w| w.rule).collect::<Vec<_>>(), ["E-REC-2"], "{tex:?}");
        assert_eq!(out.tree, tolerant(meant).tree, "{tex:?}");
    }
}

/// #211: a backslash before a line break or a tab is a control space, `\ `, with no warning, in math and
/// in text; `\\` before a line break is still a line break.
#[test]
fn backslash_before_a_line_break_is_a_control_space() {
    for (tex, meant) in [("a\\\nb", r"a\ b"), ("a\\\tb", r"a\ b"), ("a\\\r\nb", r"a\ b"), ("\\text{a\\\nb}", r"\text{a\ b}"), ("x =\\\n y", r"x =\ y")] {
        let out = tolerant(tex);
        assert!(out.warnings.is_empty(), "{tex:?}: {:?}", out.warnings);
        assert_eq!(out.tree, tolerant(meant).tree, "{tex:?}");
        assert_eq!(strict(tex).unwrap().tree, out.tree, "{tex:?}");
    }
    assert_eq!(tolerant("\\begin{matrix}a\\\\\nb\\end{matrix}").tree, tolerant(r"\begin{matrix}a\\b\end{matrix}").tree);
}

/// #212: a line break or a tab inside text is a space, as in TeX (not a box for a character the font
/// lacks); with the spaces around it, a blank line or the next line's indentation, one space.
#[test]
fn line_breaks_and_tabs_in_text_are_spaces() {
    for (tex, meant) in [
        ("\\text{foo\nbar}", r"\text{foo bar}"),
        ("\\text{foo\tbar}", r"\text{foo bar}"),
        ("\\text{foo\r\nbar}", r"\text{foo bar}"),
        ("\\text{can be  \n    used}", r"\text{can be used}"),
        ("\\text{one\n\ntwo}", r"\text{one two}"),
        ("\\fbox{A regular map\nof rings}", r"\fbox{A regular map of rings}"),
    ] {
        let out = tolerant(tex);
        assert!(out.warnings.is_empty(), "{tex:?}: {:?}", out.warnings);
        assert_eq!(out.tree, tolerant(meant).tree, "{tex:?}");
        assert_eq!(strict(tex).unwrap().tree, out.tree, "{tex:?}");
    }
}

/// #213: a text accent on a letter in math is an error in TeX; tolerant mode draws the accented letter, as
/// KaTeX does (`\'e` as `\acute e`), with E-REC-1; strict mode refuses it.
#[test]
fn text_accents_in_math_draw_the_accented_letter() {
    let out = tolerant(r"X_{\'etale}");
    // e and the combining acute accent, not a prime
    assert_eq!(out.text, "X_(e\u{301}tale)");
    assert_eq!(out.tree, tolerant(r"X_{\acute{e}tale}").tree);
    assert_eq!(warning_rules(r"X_{\'etale}"), ["E-REC-1"]);
    assert!(strict(r"X_{\'etale}").is_err());
    for (tex, meant) in
        [(r"\'{e}", r"\acute{e}"), (r"\`e", r"\grave e"), (r"\^o", r"\hat o"), (r#"\"a"#, r"\ddot a"), (r"\~n", r"\tilde n"), (r"\=x", r"\bar x"), (r"\.y", r"\dot y")]
    {
        assert_eq!(tolerant(tex).tree, tolerant(meant).tree, "{tex}");
        assert!(strict(tex).is_err(), "{tex}");
    }
    // not before a letter: the backslash is dropped, as before
    assert_eq!(tolerant(r"f\'(x)").text, "f′(x)");
}

/// #213: the width, height or depth of a box register (`\wd0`, from the classic `\dashint` macro) cannot
/// be measured: no space with a warning in tolerant mode (the register is part of the measure, not a digit
/// drawn after it), refused in strict mode.
#[test]
fn a_box_dimension_is_no_space_in_tolerant_mode() {
    for tex in [r"a\kern-.6\wd0 b", r"a\kern\ht2 b", r"a\mskip\dp12 b", r"a\hskip\wd\mybox b", r"a\hspace{\wd0} b"] {
        let out = tolerant(tex);
        assert_eq!(out.text, "ab", "{tex}");
        assert_eq!(warning_rules(tex), ["E-REC-9"], "{tex}");
        assert!(strict(tex).unwrap_err().to_string().contains("unrecognized measure"), "{tex}");
    }
}
