//! mhchem's `\ce` and `\pu` (L-CHE-1): what each construction becomes in the tree, in the speech and in
//! the linear text. Every case is valid mhchem, so it must read the same in both modes, with no warnings.

use context_equation_core::tree::{Class, FractionStyle, TextPart, Variant};
use context_equation_core::{convert, Locale, Node, Options};

fn read(tex: &str, locale: Locale) -> context_equation_core::Output {
    let strict = convert(tex, &Options { locale, strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options { locale, ..Options::default() }).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    assert_eq!(strict.svg, tolerant.svg, "{tex}");
    strict
}

fn en(tex: &str) -> context_equation_core::Output {
    read(tex, Locale::En)
}

fn tree(tex: &str) -> String {
    format!("{:?}", en(tex).tree)
}

fn has(t: &str, snippets: &[&str]) {
    for snippet in snippets {
        assert!(t.contains(snippet), "missing {snippet:?} in {t}");
    }
}

/// Every node of the tree, depth first.
fn nodes(node: &Node) -> Vec<&Node> {
    let mut out = vec![node];
    let children: Vec<&Node> = match node {
        Node::Row { children } => children.iter().collect(),
        Node::Scripts { base, sup, sub } => std::iter::once(base.as_ref()).chain(sup.as_deref()).chain(sub.as_deref()).collect(),
        Node::Fraction { num, den, .. } => vec![num, den],
        Node::Arrow { above, below, .. } => above.as_deref().into_iter().chain(below.as_deref()).collect(),
        Node::Text { parts } => parts.iter().filter_map(|p| if let TextPart::Math(m) = p { Some(m) } else { None }).collect(),
        Node::Delimited { parts, .. } => parts.iter().collect(),
        Node::Quantity { unit, .. } => vec![unit],
        _ => vec![],
    };
    for c in children {
        out.extend(nodes(c));
    }
    out
}

fn arrows(tex: &str) -> Vec<Node> {
    let r = en(tex);
    nodes(&r.tree).into_iter().filter(|n| matches!(n, Node::Arrow { .. })).cloned().collect()
}

#[test]
fn l_che_1_arrow_with_a_label_above() {
    let a = arrows(r"\ce{A ->[\Delta] B}");
    let [Node::Arrow { symbol: '→', above: Some(above), below: None }] = a.as_slice() else { panic!("{a:?}") };
    assert_eq!(**above, Node::Identifier { name: "Δ".into(), variant: context_equation_core::tree::Variant::Upright });
    assert!(en(r"\ce{A ->[\Delta] B}").applied.iter().any(|a| a.rule == "L-CHE-1"));
}

#[test]
fn l_che_1_arrow_with_labels_above_and_below() {
    let a = arrows(r"\ce{A ->[catalyst][heat] B}");
    let [Node::Arrow { above: Some(above), below: Some(below), .. }] = a.as_slice() else { panic!("{a:?}") };
    // words in labels are upright text, not a product of italic letters
    has(&format!("{above:?}"), &["Text(\"catalyst\")"]);
    has(&format!("{below:?}"), &["Text(\"heat\")"]);
    assert_eq!(en(r"\ce{A ->[catalyst][heat] B}").speech, "A right arrow (catalyst) (heat) B");
}

#[test]
fn l_che_1_equilibrium_with_a_formula_as_label() {
    let a = arrows(r"\ce{H2O <=>[K_w] H+ + OH-}");
    let [Node::Arrow { symbol: '⇌', above: Some(above), .. }] = a.as_slice() else { panic!("{a:?}") };
    has(&format!("{above:?}"), &["\"K\"", "Identifier { name: \"w\""]);
    let speech = en(r"\ce{H2O <=>[K_w] H+ + OH-}").speech;
    assert!(speech.contains("in equilibrium with (K sub w)"), "{speech}");
}

#[test]
fn l_che_1_label_with_math_between_dollars() {
    let a = arrows(r"\ce{A + B ->[$\Delta H < 0$] C}");
    let [Node::Arrow { above: Some(above), .. }] = a.as_slice() else { panic!("{a:?}") };
    has(&format!("{above:?}"), &["\"Δ\"", "'<'", "\"0\""]);
    assert!(!format!("{above:?}").contains("'$'"));
}

#[test]
fn l_che_1_every_arrow() {
    for (tex, symbol) in [("->", '→'), ("<-", '←'), ("<->", '↔'), ("<-->", '⇄'), ("<=>", '⇌'), ("<=>>", '⥂'), ("<<=>", '⥄')] {
        let r = en(&format!(r"\ce{{A {tex} B}}"));
        assert!(nodes(&r.tree).iter().any(|n| matches!(n, Node::Operator { symbol: s, class: Class::Rel } if *s == symbol)), "{tex}: {:?}", r.tree);
        // the arrow alone in \ce does not take the next token as a label
        assert_eq!(r.text.matches('[').count(), 0, "{tex}: {}", r.text);
    }
}

#[test]
fn l_che_1_hydrate_coefficient_is_not_a_subscript() {
    let t = tree(r"\ce{CuSO4.5H2O}");
    // the 5 after the dot is a coefficient, beside the dot; the 4 before it stays a subscript of O
    has(&t, &["Operator { symbol: '·', class: Ord }, Number { value: \"5\"", "sub: Some(Number { value: \"4\""]);
    assert!(!t.contains("base: Operator { symbol: '·'"), "{t}");
    assert_eq!(en(r"\ce{CuSO4.5H2O}").text, "CuSO_4·5 H_2O");
    assert_eq!(en(r"\ce{CuSO4*5H2O}").text, "CuSO_4·5 H_2O");
}

#[test]
fn l_che_1_decimal_and_fraction_coefficients() {
    let r = en(r"\ce{0.5 H2}");
    assert_eq!(r.tree.children()[0].clone().simplify().children()[0], Node::Number { value: "0.5".into(), group: false, thousands: None, variant: Variant::Upright });
    assert_eq!(r.speech, "0.5 H sub 2");
    let r = en(r"\ce{1/2 O2 -> O}");
    let first = r.tree.children()[0].children()[0].clone();
    let Node::Fraction { style: FractionStyle::Row, rule: true, .. } = first else { panic!("{first:?}") };
    assert!(r.speech.starts_with("1 over 2 O sub 2"), "{}", r.speech);
    // in pt-BR the decimal coefficient is written with a comma
    assert_eq!(read(r"\ce{0.5 H2}", Locale::PtBr).text, "0,5 H_2");
}

#[test]
fn l_che_1_math_coefficient_between_dollars() {
    let r = en(r"\ce{$x$ H2O}");
    assert!(!r.text.contains('$'), "{}", r.text);
    assert!(nodes(&r.tree).iter().any(|n| matches!(n, Node::Identifier { name, variant: context_equation_core::tree::Variant::Italic } if name == "x")));
    assert_eq!(r.text, "x H_2O");
    // a lone lowercase letter is a variable coefficient too
    assert_eq!(en(r"\ce{n H2O}").text, "n H_2O");
}

#[test]
fn l_che_1_bonds() {
    for (tex, bond) in [
        (r"\ce{CH3-CH3}", "−"),
        (r"\ce{CH2=CH2}", "="),
        (r"\ce{HC#CH}", "≡"),
        (r"\ce{A\bond{-}B}", "−"),
        (r"\ce{A\bond{=}B}", "="),
        (r"\ce{A\bond{#}B}", "≡"),
        (r"\ce{A\bond{3}B}", "≡"),
    ] {
        has(&tree(tex), &[&format!("Text(\"{bond}\")")]);
    }
    // a bond is not a minus, nor a congruence
    for tex in [r"\ce{HC#CH}", r"\ce{CH3-CH3}"] {
        let speech = en(tex).speech;
        assert!(!speech.contains("congruent") && !speech.contains("negative") && !speech.contains("minus"), "{tex}: {speech}");
    }
}

#[test]
fn l_che_1_isotopes_share_one_empty_base() {
    let r = en(r"\ce{^{14}_{6}C}");
    let scripts = nodes(&r.tree).into_iter().find(|n| matches!(n, Node::Scripts { .. })).cloned();
    let Some(Node::Scripts { base, sup: Some(sup), sub: Some(sub) }) = scripts else { panic!("{:?}", r.tree) };
    assert_eq!(*base, Node::row(vec![]));
    assert_eq!(*sup, Node::Number { value: "14".into(), group: false, thousands: None, variant: Variant::Upright });
    assert_eq!(*sub, Node::Number { value: "6".into(), group: false, thousands: None, variant: Variant::Upright });
    assert_eq!(r.text, "_6^14C");
}

#[test]
fn l_che_1_nuclear_equation() {
    let r = en(r"\ce{^{235}_{92}U + ^{1}_{0}n -> ^{141}_{56}Ba + ^{92}_{36}Kr + 3^{1}_{0}n}");
    assert_eq!(r.text, "_92^235U + _0^1n → _56^141Ba + _36^92Kr + 3 _0^1n");
    // after the coefficient 3, the isotope starts a new base instead of attaching to the space
    let t = format!("{:?}", r.tree);
    assert!(!t.contains("base: Space"), "{t}");
}

#[test]
fn l_che_1_charges_radicals_and_oxidation_states() {
    has(&tree(r"\ce{O2^{.-}}"), &["Text(\"•−\")"]);
    has(&tree(r"\ce{CH3^{.}}"), &["Text(\"•\")"]);
    has(&tree(r"\ce{Mn^{VII}}"), &["Text(\"VII\")"]);
    has(&tree(r"\ce{e-}"), &["Text(\"e\")", "Text(\"−\")"]);
    // the subscript of a charged ion still belongs to the same base
    let t = tree(r"\ce{SO4^{2-}_{(aq)}}");
    has(&t, &["sup: Some(Text { parts: [Text(\"2−\")] })", "sub: Some(Number { value: \"4\""]);
    has(&t, &["Text(\"(aq)\")"]);
}

#[test]
fn l_che_1_fraction_with_formulas_and_charges_inside() {
    let r = en(r"\ce{K_{a} = \frac{[H+][A-]}{[HA]}}");
    let Some(Node::Fraction { num, .. }) = nodes(&r.tree).into_iter().find(|n| matches!(n, Node::Fraction { .. })).cloned() else { panic!("{:?}", r.tree) };
    // H+ and A- inside the brackets are charges, not an addition and a subtraction
    let num = format!("{num:?}");
    has(&num, &["sup: Some(Text { parts: [Text(\"+\")] })", "sup: Some(Text { parts: [Text(\"−\")] })"]);
    assert!(!num.contains("class: Bin"), "{num}");
    assert_eq!(r.text, "K_a = ([H^+][A^−])/([HA])");
}

#[test]
fn l_che_1_subscript_variable_and_states() {
    has(&tree(r"\ce{NO_x}"), &["sub: Some(Identifier { name: \"x\""]);
    has(&tree(r"\ce{CO2 (g) + H2O (l)}"), &["Text(\"(g)\")", "Text(\"(l)\")"]);
}

#[test]
fn l_che_1_physical_units() {
    let r = en(r"\pu{123 kJ mol^-1}");
    let Node::Quantity { value: Some(v), .. } = &r.tree.children()[0] else { panic!("{:?}", r.tree) };
    assert_eq!(v, "123");
    assert_eq!(r.text, "123 kJ mol^(−1)");
    assert_eq!(en(r"\pu{1.5e3 m.s^-2}").text, "1.5 × 10^3 m · s^(−2)");
    assert_eq!(en(r"\pu{kg}").text, "kg");
}

/// `//` in \pu is a fraction bar: mhchem.js' `pu` state makes a `pu-frac`, a \frac in text style (#162).
#[test]
fn l_che_1_physical_units_fraction() {
    let r = en(r"\pu{75.3 J // mol K}");
    let Node::Quantity { value: Some(v), .. } = &r.tree.children()[0] else { panic!("{:?}", r.tree) };
    assert_eq!(v, "75.3");
    assert_eq!(r.text, "75.3 J/(mol K)");
    // the same drawing as the fraction written by hand, in text style
    let t = format!("{:?}", r.tree);
    assert!(t.contains("Fraction") && t.contains("style: Row"), "{t}");
    assert_eq!(en(r"\pu{1.2 kJ // mol}").text, "1.2 (kJ)/(mol)");
    // a single slash stays a slash
    assert_eq!(en(r"\pu{75.3 J/mol K}").text, "75.3 J/mol K");
}

#[test]
fn l_che_1_gas_and_precipitate_marks() {
    let r = en(r"\ce{CO2 ^ + AgCl v}");
    assert_eq!(r.text, "CO_2↑ + AgCl↓");
}

#[test]
fn l_che_1_ce_and_pu_inside_text() {
    // as in math, between the words of the text
    let t = tree(r"\text{water, \ce{H2O}, at \pu{20 mol}}");
    assert_eq!(t, tree(r"\text{water, $\ce{H2O}$, at $\pu{20 mol}$}"));
    has(&t, &["Text(\"water, \")", "Math(Row { children: [Scripts", "Quantity"]);
    assert_eq!(en(r"\text{water, \ce{H2O}}").text, en(r"\text{water, }\ce{H2O}").text);
}

fn word(w: &str) -> Node {
    Node::Text { parts: vec![TextPart::Text(w.into())] }
}

fn texts(node: &Node) -> Vec<Node> {
    nodes(node).into_iter().filter(|n| matches!(n, Node::Text { .. })).cloned().collect()
}

#[test]
fn l_che_1_words_are_text() {
    // a capital and lowercase letters that are not an element symbol: one word, not a formula
    let r = en(r"\ce{Hydrogen + Oxygen -> Water}");
    assert_eq!(texts(&r.tree), [word("Hydrogen"), word("Oxygen"), word("Water")]);
    assert_eq!(r.text, "Hydrogen + Oxygen → Water");
    // the word is read as a word, not spelled
    assert_eq!(r.speech, "Hydrogen plus Oxygen tends to Water");
    // an element symbol stays an element, and a formula a formula
    for (tex, pieces) in [(r"\ce{Co}", &["Co"][..]), (r"\ce{CO}", &["C", "O"]), (r"\ce{NaOH}", &["Na", "O", "H"]), (r"\ce{He}", &["He"])] {
        assert_eq!(texts(&en(tex).tree), pieces.iter().map(|p| word(p)).collect::<Vec<_>>(), "{tex}");
    }
}

#[test]
fn l_che_1_words_written_apart_get_a_word_space() {
    // mhchem's entitySkip: two formulas or words written apart, with no operator or arrow between
    let r = en(r"\ce{sodium chloride -> sodium + chlorine}");
    let row = r.tree.children().to_vec();
    assert_eq!(row[..3], [word("sodium"), Node::Space { em: 0.333 }, word("chloride")]);
    // around operators and arrows the spacing is theirs
    assert_eq!(row.iter().filter(|n| matches!(n, Node::Space { em } if *em > 0.0)).count(), 1, "{row:?}");
    assert_eq!(r.text, "sodium chloride → sodium + chlorine");
    assert_eq!(r.speech, "sodium chloride tends to sodium plus chlorine");
    assert_eq!(en(r"\ce{NaCl solution}").text, "NaCl solution");
    // a coefficient keeps its small space, a state written apart and a bond get none
    assert_eq!(en(r"\ce{CO2 + C -> 2 CO}").text, "CO_2 + C → 2 CO");
    for tex in [r"\ce{CO2 (g) + 2 CO}", r"\ce{A \bond{-} B}", r"\ce{n H2O}", r"\ce{$x$ H2O}", r"\ce{AgCl v}"] {
        let t = tree(tex);
        assert!(!t.contains("0.333"), "{tex}: {t}");
    }
}

#[test]
fn l_che_1_carbon_monoxide_is_a_formula() {
    let r = en(r"\ce{CO2 + C -> 2 CO}");
    assert_eq!(r.speech, "C O sub 2 plus C tends to 2 C O");
    assert!(!texts(&r.tree).contains(&word("CO")), "{:?}", r.tree);
}

#[test]
fn l_che_1_state_of_aggregation_with_math_inside() {
    let r = en(r"\ce{NaOH(aq,$\infty$)}");
    let state = r.tree.children().last().cloned();
    let Some(Node::Text { parts }) = state else { panic!("{:?}", r.tree) };
    assert_eq!(parts.len(), 3, "{parts:?}");
    assert_eq!(parts[0], TextPart::Text("(aq,".into()));
    assert_eq!(parts[2], TextPart::Text(")".into()));
    assert!(matches!(&parts[1], TextPart::Math(Node::Operator { symbol: '∞', .. })), "{parts:?}");
    assert_eq!(r.text, "NaOH(aq,∞)");
    assert!(r.speech.starts_with("Na O H (aq, infinity"), "{}", r.speech);
    // a parenthesis that is not a state stays a group of the formula
    has(&tree(r"\ce{Cu(en)2}"), &["Operator { symbol: '(', class: Open }"]);
}

#[test]
fn l_che_1_amphoteric_hydroxide_with_a_word_label() {
    let tex = r"\ce{Zn^2+  <=>[+ 2OH-][+ 2H+]  $\underset{\text{amphoteric hydroxide}}{\ce{Zn(OH)2 v}}$}";
    let a = arrows(tex);
    let [Node::Arrow { symbol: '⇌', above: Some(_), below: Some(_) }] = a.as_slice() else { panic!("{a:?}") };
    let r = en(tex);
    assert!(r.speech.ends_with("Zn open paren O H close paren sub 2 ↓ with amphoteric hydroxide below"), "{}", r.speech);
    has(&format!("{:?}", r.tree), &["Text(\"amphoteric hydroxide\")"]);
}

#[test]
fn l_che_1_words_in_other_languages() {
    // a capital outside ASCII starts a word too: Água, not an operator Á
    let r = read(r"\ce{Água + sódio}", Locale::PtBr);
    assert_eq!(texts(&r.tree), [word("Água"), word("sódio")]);
}

#[test]
fn l_che_1_command_in_an_isotope_superscript() {
    // #252: a command in the superscript is read, as mhchem does: the metastable state of 234mPa is the
    // same superscript as the one written without the command
    let plain = en(r"\ce{^{234m}_{91}Pa}");
    for tex in [r"\ce{^{234\mathrm{m}}_{91}Pa}", r"\ce{^{234\text{m}}_{91}Pa}"] {
        let out = en(tex);
        assert_eq!(out.tree, plain.tree, "{tex}");
        assert_eq!((out.text.as_str(), out.speech.as_str()), (plain.text.as_str(), plain.speech.as_str()), "{tex}");
    }
    // a symbol stays a symbol (no raw "\alpha" text)
    has(&tree(r"\ce{X^{\alpha}}"), &["\"α\""]);
    // a command it does not know warns and is drawn in red; strict mode refuses it
    let tex = r"\ce{^{234\foo{m}}_{91}Pa}";
    assert!(convert(tex, &Options { strict: true, ..Options::default() }).is_err());
    let out = convert(tex, &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1"), "{:?}", out.warnings);
    assert!(!format!("{:?}", out.tree).contains("234\\\\foo"), "{:?}", out.tree);
}

#[test]
fn l_che_1_bonds_of_more_than_one_kind_of_line() {
    // #267: mhchem's \bond{~-}, \bond{~=} (and its spelling ~--), \bond{-~-} and \bond{....}: each its own
    // symbol, read as the lines it is, named by its \bond in the tree's JSON
    let cases = [
        ("~", "bond of a dashed line", "A┄B"),
        ("~-", "bond of a dashed line over a line", "A≃B"),
        ("~=", "bond of a dashed line over two lines", "A≅B"),
        ("~--", "bond of a dashed line over two lines", "A≅B"),
        ("-~-", "bond of a dashed line between two lines", "A≋B"),
        ("....", "bond of four dots", "A····B"),
    ];
    for (bond, speech, text) in cases {
        let tex = format!(r"\ce{{A\bond{{{bond}}}B}}", bond = bond);
        let out = en(&tex);
        assert_eq!(out.speech, format!("A {speech} B"), "{tex}");
        assert_eq!(out.text, text, "{tex}");
        let json = out.tree.to_json();
        let named = if bond == "~--" { "~=" } else { bond };
        assert!(json.contains(&format!(r#""symbol":"\\bond{{{named}}}""#)), "{tex}: {json}");
        assert!(out.svg.contains("<rect") || out.svg.contains("<use"), "{tex}: nothing drawn");
    }
    // ~= and ~-- are one drawing; the others differ from each other and from the single bond
    let tree = |b: &str| en(&format!(r"\ce{{A\bond{{{b}}}B}}")).tree;
    assert_eq!(tree("~="), tree("~--"));
    let drawings: Vec<Node> = ["-", "~", "~-", "~=", "-~-", "...."].iter().map(|b| tree(b)).collect();
    for (i, a) in drawings.iter().enumerate() {
        assert!(drawings[i + 1..].iter().all(|b| b != a), "two bonds read alike");
    }
    // in another language, the language's phrase
    assert_eq!(read(r"\ce{A\bond{~-}B}", Locale::PtBr).speech, "A ligação de uma linha tracejada sobre uma linha B");
}

#[test]
fn l_che_1_arrow_label_between_braces_is_text() {
    // #268: a label between braces is text, spaces and all
    let out = en(r"\ce{A ->[{text above}][{text below}] B}");
    let t = format!("{:?}", out.tree);
    has(&t, &["Text(\"text above\")", "Text(\"text below\")"]);
    assert!(!t.contains("'{'"), "{t}");
    assert_eq!(out.speech, "A right arrow (text above) (text below) B");
    // with math inside, between dollars
    has(&tree(r"\ce{A ->[{$\Delta$ heat}] B}"), &["Text(\" heat\")", "\"Δ\""]);
    // without braces, still read as \ce
    assert_eq!(en(r"\ce{A ->[above][below] B}").speech, "A right arrow (above) (below) B");
}

#[test]
fn l_che_1_math_between_dollars_in_a_script() {
    // #269: $…$ in a subscript or a superscript is mhchem's escape to math, with no warning
    let out = en(r"\ce{Fe(CN)_{$\frac{6}{2}$}}");
    assert_eq!(out.text, "Fe(CN)_(6/2)");
    assert!(nodes(&out.tree).iter().any(|n| matches!(n, Node::Fraction { .. })), "{:?}", out.tree);
    let out = en(r"\ce{X^{$n+$}}");
    assert_eq!(out.text, "X^(n+)");
    assert!(!out.text.contains('$') && !out.speech.contains('$'), "{}", out.speech);
}

#[test]
fn l_che_1_ampersand_and_new_row_go_to_the_alignment() {
    // #270: & and \\ in a \ce inside an alignment are the alignment's, as mhchem passes them on
    let out = en("\\begin{align*}\n  \\ce{A &<=>[+e] B \\\\\n      B &<=>[+e] C}\n\\end{align*}");
    let rows = nodes(&out.tree).into_iter().find_map(|n| if let Node::Table { rows, .. } = n { Some(rows.clone()) } else { None }).expect("a table");
    assert_eq!(rows.len(), 2, "{:?}", out.tree);
    assert!(rows.iter().all(|r| r.len() == 2), "{rows:?}");
    assert_eq!(out.speech, "A in equilibrium with (plus e) B; B in equilibrium with (plus e) C");
    // an escaped & is a character; without an alignment around, & warns as anywhere else
    assert_eq!(en(r"\ce{A\&B}").text, "A&B");
    let out = convert(r"\ce{A & B}", &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.description.contains("'&' outside an alignment")), "{:?}", out.warnings);
}
