//! Layout: from the tree to boxes. Each case checks a geometric property (what sits above,
//! below, wider, taller), not exact coordinates, so it survives fine adjustments.

use context_equation_core::layout::{self, compose, Context, Item, MathBox, Style};
use context_equation_core::rules::Registry;
use context_equation_core::tree::{Class, Node, Variant};
use context_equation_core::{Error, Locale, Options};

/// No fallback fonts, with or without the `emoji` feature: a character the math font lacks is a box
/// here (tests/fallback.rs has the fallbacks).
fn options() -> Options {
    Options { locale: Locale::PtBr, fallback_fonts: Vec::new(), ..Options::default() }
}

/// Box composed through the public API, with the tolerant mode's warnings.
fn math_box(tex: &str) -> MathBox {
    context_equation_core::typeset(tex, &options()).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn inline_box(tex: &str) -> MathBox {
    context_equation_core::typeset(tex, &Options { display: false, ..options() }).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn warnings(tex: &str) -> Vec<String> {
    context_equation_core::typeset(tex, &options()).unwrap().1.into_iter().map(|a| format!("{}: {}", a.rule, a.description)).collect()
}

/// Full SVG, checked against invalid numbers.
fn svg(tex: &str) -> String {
    let r = context_equation_core::convert(tex, &options()).unwrap_or_else(|e| panic!("{tex}: {e}"));
    assert_valid_svg(&r.svg, tex);
    r.svg
}

fn assert_valid_svg(svg: &str, tex: &str) {
    for bad in ["NaN", "inf", "-0-"] {
        assert!(!svg.contains(bad), "{bad} in the SVG of {tex}");
    }
    let vb = svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap();
    let n: Vec<f32> = vb.split_whitespace().map(|v| v.parse().unwrap()).collect();
    // empty content may give zero area, never negative
    assert!(n[2] >= 0.0 && n[3] >= 0.0, "negative viewBox in {tex}: {vb}");
}

/// Composes a hand-built tree (for nodes the reader only produces in rare combinations).
fn compose_node(node: &Node, e: Style, strict: bool) -> Result<MathBox, Error> {
    let mut registry = Registry::default();
    let mut ctx = Context { locale: Locale::PtBr, registry: &mut registry, strict, kerning: Default::default(), fallbacks: &[] };
    compose(node, e, &mut ctx)
}

fn ident(c: &str) -> Node {
    Node::Identifier { name: c.into(), variant: Variant::Italic }
}

fn total(c: &MathBox) -> f32 {
    c.height + c.depth
}

fn consistent(c: &MathBox, tex: &str) {
    assert!(c.width.is_finite() && c.height.is_finite() && c.depth.is_finite(), "{tex}: {c:?}");
    assert!(c.width >= 0.0, "{tex}: width {}", c.width);
    assert!(total(c) >= 0.0, "{tex}: total height {}", total(c));
}

fn bars(c: &MathBox) -> usize {
    c.items.iter().filter(|i| matches!(i, Item::Rule { .. })).count()
}

fn lines(c: &MathBox) -> usize {
    c.items.iter().filter(|i| matches!(i, Item::Line { .. })).count()
}

// ---------- alphabets ----------

#[test]
fn letters_already_in_the_letterlike_block() {
    use layout::alphabet::character;
    use Variant::*;
    let cases = [
        (Italic, 'h', 'ℎ'),
        (Italic, 'ı', '𝚤'),
        (Italic, 'ȷ', '𝚥'),
        (Calligraphic, 'B', 'ℬ'),
        (Calligraphic, 'E', 'ℰ'),
        (Calligraphic, 'F', 'ℱ'),
        (Calligraphic, 'H', 'ℋ'),
        (Calligraphic, 'I', 'ℐ'),
        (Calligraphic, 'L', 'ℒ'),
        (Calligraphic, 'M', 'ℳ'),
        (Calligraphic, 'R', 'ℛ'),
        (Calligraphic, 'e', 'ℯ'),
        (Calligraphic, 'g', 'ℊ'),
        (Calligraphic, 'o', 'ℴ'),
        (Fraktur, 'C', 'ℭ'),
        (Fraktur, 'H', 'ℌ'),
        (Fraktur, 'I', 'ℑ'),
        (Fraktur, 'R', 'ℜ'),
        (Fraktur, 'Z', 'ℨ'),
        (DoubleStruck, 'C', 'ℂ'),
        (DoubleStruck, 'H', 'ℍ'),
        (DoubleStruck, 'N', 'ℕ'),
        (DoubleStruck, 'P', 'ℙ'),
        (DoubleStruck, 'Q', 'ℚ'),
        (DoubleStruck, 'R', 'ℝ'),
        (DoubleStruck, 'Z', 'ℤ'),
    ];
    for (v, from, to_char) in cases {
        assert_eq!(character(from, v), to_char, "{v:?} {from}");
    }
}

#[test]
fn alphabets_by_block() {
    use layout::alphabet::character;
    use Variant::*;
    assert_eq!(character('x', Upright), 'x');
    assert_eq!(character('A', Bold), '𝐀');
    assert_eq!(character('7', Bold), '𝟕');
    assert_eq!(character('Δ', Bold), '𝚫');
    assert_eq!(character('α', Bold), '𝛂');
    assert_eq!(character('A', BoldItalic), '𝑨');
    assert_eq!(character('Γ', BoldItalic), '𝜞');
    assert_eq!(character('β', BoldItalic), '𝜷');
    assert_eq!(character('A', Calligraphic), '𝒜');
    assert_eq!(character('a', Fraktur), '𝔞');
    assert_eq!(character('1', DoubleStruck), '𝟙');
    assert_eq!(character('A', SansSerif), '𝖠');
    assert_eq!(character('3', SansSerif), '𝟥');
    assert_eq!(character('A', Monospace), '𝙰');
    assert_eq!(character('0', Monospace), '𝟶');
    assert_eq!(character('α', Italic), '𝛼');
    // no form in the block: stays as is
    assert_eq!(character('7', Italic), '7');
    assert_eq!(character('Δ', Calligraphic), 'Δ');
    assert_eq!(character('é', Bold), 'é');
    assert_eq!(layout::math_italic('x'), '𝑥');
}

#[test]
fn variants_through_the_reader() {
    for tex in [r"\mathbb{R}", r"\mathcal{L}", r"\mathfrak{g}", r"\mathsf{A}", r"\mathtt{x}", r"\boldsymbol{\alpha}", r"\mathbf{v}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(c.width > 0.0, "{tex}");
    }
}

// ---------- box ----------

#[test]
fn scalebox_scales_glyphs_rules_lines_and_circles() {
    // the content of \scalebox is text mode: compare with the inline fraction
    let normal = inline_box(r"\frac{\cancel{a}}{\textcircled{b}}");
    let double = math_box(r"\scalebox{2}{$\frac{\cancel{a}}{\textcircled{b}}$}");
    assert!((double.width - 2.0 * normal.width).abs() < 1.0, "{} vs {}", double.width, normal.width);
    assert!(bars(&double) >= 1 && lines(&double) >= 1);
    assert!(double.items.iter().any(|i| matches!(i, Item::Ellipse { rx, .. } if *rx > 0.0)));
    let thicknesses = |c: &MathBox| c.items.iter().find_map(|i| if let Item::Line { thickness, .. } = i { Some(*thickness) } else { None }).unwrap();
    assert!((thicknesses(&double) - 2.0 * thicknesses(&normal)).abs() < 0.01);
}

// ---------- styles ----------

#[test]
fn explicit_styles_change_size() {
    let d = math_box(r"\displaystyle\sum_{i=1}^n i");
    let t = math_box(r"\textstyle\sum_{i=1}^n i");
    let s = math_box(r"\scriptstyle x");
    let ss = math_box(r"\scriptscriptstyle x");
    assert!(d.height + d.depth > t.height + t.depth, "display limits go above and below");
    assert!(s.width > ss.width);
    // hand-built level: 0 to 3 and one out-of-range level, which counts as scriptscript
    let widths: Vec<f32> = [0u8, 1, 2, 3, 9].iter().map(|n| compose_node(&Node::Style { level: *n, base: Box::new(ident("x")) }, Style::display(), true).unwrap().width).collect();
    assert_eq!(widths[0], widths[1]);
    assert!(widths[1] > widths[2] && widths[2] > widths[3]);
    assert_eq!(widths[3], widths[4]);
}

#[test]
fn text_style_has_no_limits_above() {
    let e = Style::text();
    assert!(!e.is_script());
    assert_eq!(e.scale(), 1.0);
    let inline = inline_box(r"\sum_{i=1}^n i");
    let display = math_box(r"\sum_{i=1}^n i");
    assert!(inline.width > display.width * 0.8, "inline, the scripts go to the side");
    assert!(total(&inline) < total(&display));
}

#[test]
fn fraction_inside_script_becomes_scriptscript() {
    // num(Script) drops to scriptscript
    let c = math_box(r"x^{\frac{a}{b}}");
    consistent(&c, "x^frac");
    let c = math_box(r"x^{\tfrac{1}{2}} + \dfrac{1}{2} + \frac{a}{b}");
    consistent(&c, "tfrac/dfrac");
}

// ---------- leaves and text ----------

#[test]
fn characters_without_glyph_become_equivalent_or_box() {
    for tex in [r"\text{✔ ✅ ✗ ✘ ❌ – — ‘a’ “b”}", "\\text{a\u{00A0}b\u{2009}c\u{202F}d}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(warnings(tex).is_empty(), "{tex}: {:?}", warnings(tex));
    }
    // emoji has no equivalent: a box with a warning
    let a = warnings(r"\text{olá 🙂}");
    assert!(a.iter().any(|a| a.contains("U+1F642")), "{a:?}");
    let a = warnings("🙂 + 1");
    assert!(a.iter().any(|a| a.contains("U+1F642")), "{a:?}");
    // in strict mode, a font error
    let strict = Options { strict: true, ..options() };
    assert!(matches!(context_equation_core::typeset(r"\text{🙂}", &strict), Err(Error::MissingGlyph { character: '🙂' })));
    assert!(matches!(context_equation_core::typeset("🙂", &strict), Err(Error::MissingGlyph { .. })));
}

#[test]
fn text_with_special_spaces() {
    let wide = math_box("\\text{a\u{2003}b}");
    let narrow = math_box("\\text{a b}");
    let attached = math_box("\\text{a\u{200B}b}");
    assert!(wide.width > narrow.width && narrow.width > attached.width);
}

#[test]
fn bold_text_uses_block_letters() {
    let normal = math_box(r"\text{abc}");
    let bold = math_box(r"\text{\textbf{abc}}");
    assert!(bold.width > normal.width);
    // an accented letter stays regular, but without a box
    assert!(warnings(r"\text{\textbf{ação}}").is_empty());
}

#[test]
fn italic_text_has_no_correction_inside_it() {
    // L-TXT-5: text italic sets its letters side by side, as a text font does: no italic correction
    // between them, and none after \itshape's group
    let f = math_box(r"\text{\itshape f}").width;
    assert!((math_box(r"\text{\itshape ff}").width - 2.0 * f).abs() < 1e-4);
    assert!(math_box(r"\text{\itshape f}").width > math_box(r"\text{f}").width);
    // \textit{…} ends with the last letter's correction (LaTeX's \/), but not before a period or a comma
    assert!(math_box(r"\text{\textit{f}}").width > f + 1e-4);
    let period = math_box(r"\text{{\itshape f}.}").width;
    assert!((math_box(r"\text{\textit{f}.}").width - period).abs() < 1e-4);
    assert!((math_box(r"\text{\textit{f},}").width - math_box(r"\text{{\itshape f},}").width).abs() < 1e-4);
    // nor after a space
    assert!((math_box(r"\text{\textit{f }}").width - math_box(r"\text{{\itshape f }}").width).abs() < 1e-4);
    // bold italic in \textbf
    assert!(math_box(r"\text{\textbf{\textit{f}}}").width != math_box(r"\text{\textit{f}}").width);
}

#[test]
fn letters_of_mathit_make_a_word() {
    // \mathit's font is a text font: no italic correction between its letters, as math italic has
    let word = math_box(r"\mathit{ff}").width;
    assert!(word < math_box(r"ff").width, "{word}");
    // one letter is the same as math italic's, its correction at the end included
    assert!((math_box(r"\mathit{f}").width - math_box(r"f").width).abs() < 1e-4);
    assert!((math_box(r"\mathit{ff}+1").width - math_box(r"\mathit{f}\mathit{f}+1").width).abs() > 1e-4);
}

#[test]
fn bold_small_capitals_and_monospace_go_on_after_math_in_their_group() {
    // the glyphs of a box, left to right
    let glyphs = |tex: &str| -> Vec<u16> { math_box(tex).items.iter().filter_map(|i| if let Item::Glyph { id, .. } = i { Some(id.0) } else { None }).collect() };
    // `\textbf{Z $x$ Z}`: both Z bold, as TeX sets them
    let g = glyphs(r"\text{\textbf{Z$x$Z}}");
    assert_eq!(g[0], g[2], "{g:?}");
    assert_eq!(g[0], glyphs(r"\text{\textbf{Z}}")[0]);
    // after other math in text too (`\ensuremath`, `\(…\)`), and for small capitals and monospace:
    // the letters on both sides are those of the group without math
    for (tex, alone) in [
        (r"\text{\textbf{ab\ensuremath{x}ab}}", r"\text{\textbf{ab}}"),
        (r"\text{\textbf{ab\(x\)ab}}", r"\text{\textbf{ab}}"),
        (r"\text{\textsc{ab $x$ ab}}", r"\text{\textsc{ab}}"),
        (r"\text{\texttt{ab $x$ ab}}", r"\text{\texttt{ab}}"),
    ] {
        let (g, a) = (glyphs(tex), glyphs(alone));
        assert!(g[..2] == a[..] && g[3..] == a[..], "{tex}: {g:?} against {a:?}");
    }
    // \bf until its group ends, not after
    let g = glyphs(r"\text{{\bf Z $x$ Z} Z}");
    assert!(g[0] == g[2] && g[2] != g[3], "{g:?}");
    // math at the start of the group: what follows is bold
    assert_eq!(glyphs(r"\text{\textbf{$x$Z}}")[1], glyphs(r"\text{\textbf{Z}}")[0]);
}

#[test]
fn multi_letter_identifier_without_variant() {
    let node = Node::Identifier { name: "abc".into(), variant: Variant::Italic };
    let c = compose_node(&node, Style::display(), true).unwrap();
    assert!(!c.single_glyph && c.width > 0.0);
    // a character without a form in the block falls back to the character itself
    let node = Node::Identifier { name: "ж".into(), variant: Variant::DoubleStruck };
    assert!(compose_node(&node, Style::display(), true).unwrap().width > 0.0);
}

#[test]
fn translated_functions() {
    let r = context_equation_core::convert(r"\sin x + \tan y", &options()).unwrap();
    assert!(r.applied.iter().any(|a| a.rule == "L-FUN-1"), "{:?}", r.applied);
    let r = context_equation_core::convert(r"\sin x", &Options { locale: Locale::En, ..options() }).unwrap();
    assert!(r.applied.iter().all(|a| a.rule != "L-FUN-1"));
}

#[test]
fn grouped_numbers_and_thousands() {
    let plain = math_box("12345");
    let grouped = math_box(r"\num{12345}");
    assert!(grouped.width > plain.width, "thin space between groups");
    let decimal = math_box(r"\num{1234,56}");
    consistent(&decimal, "num");
    let thousands = math_box("1.234.567");
    assert!(thousands.width > math_box("1234567").width);
}

// ---------- classes and spaces ----------

#[test]
fn classes_of_compound_nodes() {
    use layout::class_of;
    let x = || Box::new(ident("x"));
    let rel = || Box::new(Node::Operator { symbol: '=', class: Class::Rel });
    assert_eq!(class_of(&Node::Colored { color: "red".into(), base: Box::new(Node::row(vec![Node::Operator { symbol: '=', class: Class::Rel }])) }), Some(Class::Rel));
    assert_eq!(class_of(&Node::Colored { color: "red".into(), base: Box::new(Node::row(vec![ident("a"), ident("b")])) }), Some(Class::Ord));
    assert_eq!(class_of(&Node::Style { level: 1, base: rel() }), Some(Class::Rel));
    assert_eq!(class_of(&Node::Scaled { factor: 2.0, vertical: None, base: rel() }), Some(Class::Rel));
    assert_eq!(class_of(&Node::Scripts { base: rel(), sup: Some(x()), sub: None }), Some(Class::Rel));
    assert_eq!(class_of(&Node::Space { em: 1.0 }), None);
}

#[test]
fn spaces_vanish_in_scripts() {
    // in scripts only the "strong" spaces remain: x^{a+b} is proportionally narrower
    let display = math_box("a+b");
    let index = math_box("x^{a+b}");
    consistent(&index, "x^{a+b}");
    assert!(index.width < display.width + 1000.0);
    // every class combination goes through the table
    for tex in [r"\sin\sin", r"a,b", r"(a)\,", r"\left(a\right)\sum", r"a\mathpunct{.}\mathrel{=}", r"\mathinner{x}\mathopen{(}", r"x \mathbin{+} \mathclose{)}"] {
        consistent(&math_box(tex), tex);
    }
}

#[test]
fn open_interval_with_trailing_bracket() {
    let r = context_equation_core::convert("x \\in ]0, 1[ + 2", &options()).unwrap();
    assert!(r.applied.iter().filter(|a| a.rule == "L-INT-1").count() >= 2, "{:?}", r.applied);
    // [ without a ] before it is not a closing
    let r = context_equation_core::convert("a [ b", &options()).unwrap();
    assert!(r.applied.iter().all(|a| a.rule != "L-INT-1"));
}

#[test]
fn italic_correction_before_non_letter() {
    let with_fn = math_box("f(x)");
    let without = math_box(r"\mathrm{f}(x)");
    assert!(with_fn.width > without.width);
}

// ---------- fractions and roots ----------

#[test]
fn fraction_without_rule_separates_numerator_and_denominator() {
    for tex in [r"\binom{n}{k}", r"\tbinom{\frac{a}{b}}{\frac{c}{d}}", r"x^{\binom{n}{k}}", r"{a \atop b}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert_eq!(bars(&c), tex.matches("frac").count(), "{tex}: only the inner fractions have a rule");
    }
}

#[test]
fn root_with_index_and_tall_radicand() {
    let plain = math_box(r"\sqrt{x}");
    let index = math_box(r"\sqrt[3]{x}");
    assert!(index.width > plain.width);
    let tall = math_box(r"\sqrt{\frac{\frac{a}{b}}{\frac{c}{d}}}");
    assert!(total(&tall) > 2.0 * total(&plain));
    let inline = inline_box(r"\sqrt{x}");
    consistent(&inline, "inline sqrt");
    // a wide index does not leave a negative width
    consistent(&math_box(r"\sqrt[123456]{x}"), "wide index");
}

// ---------- scripts and limits ----------

#[test]
fn function_and_forced_operator_limits() {
    let lim = math_box(r"\lim_{x \to 0} f(x)");
    let inline = inline_box(r"\lim_{x \to 0} f(x)");
    assert!(total(&lim) > total(&inline));
    for tex in [r"\mathop{T}_{a}^{b}", r"\operatorname*{arg\,max}_{x} f", r"\int\limits_0^1 x", r"\sum\nolimits_{i} x", r"\max_{i} x_i"] {
        consistent(&math_box(tex), tex);
    }
    // \mathop over something that already uses limits
    let node = Node::Scripts {
        base: Box::new(Node::ForcedClass { class: Class::Op, base: Box::new(Node::LargeOperator { symbol: '∑', limits: None }) }),
        sup: Some(Box::new(ident("n"))),
        sub: None,
    };
    let c = compose_node(&node, Style::text(), true).unwrap();
    consistent(&c, "mathop sum");
}

#[test]
fn primes_with_exponent_and_subscript() {
    let only_prime = math_box("f'");
    let prime_and_exponent = math_box("f'^2");
    let two_primes_and_subscript = math_box("f''_n");
    assert!(prime_and_exponent.width > only_prime.width);
    consistent(&two_primes_and_subscript, "f''_n");
    // a row of primes followed by more in the superscript
    consistent(&math_box(r"f^{\prime\prime a}"), "primos + a");
    consistent(&math_box(r"f^{\prime\prime}"), "primes only, inline");
}

#[test]
fn sub_and_superscript_together_keep_the_gap() {
    let c = math_box(r"x_{\frac{a}{b}}^{\frac{c}{d}}");
    consistent(&c, "fractions in both scripts");
    let c = math_box(r"\int_{\frac{a}{b}}^{\frac{c}{d}} f");
    consistent(&c, "integral with tall scripts");
    // a low exponent with a subscript: the superscript moves up
    consistent(&math_box("x_{a}^{-}"), "x_a^-");
    consistent(&math_box(r"x_{\sum}^{\sum}"), "scripts with an operator");
}

#[test]
fn integrals_do_not_use_limits_by_default() {
    for s in [r"\int", r"\iint", r"\iiint", r"\oint", r"\oiint", r"\oiiint"] {
        let tex = format!("{s}_a^b f");
        let c = math_box(&tex);
        consistent(&c, &tex);
    }
}

#[test]
fn large_operator_without_glyph() {
    let node = Node::LargeOperator { symbol: '🙂', limits: None };
    assert!(compose_node(&node, Style::display(), true).is_err());
    let c = compose_node(&node, Style::display(), false).unwrap();
    assert!(c.width > 0.0, "a box in tolerant mode");
}

// ---------- delimiters ----------

#[test]
fn delimiters_with_middle_and_missing_side() {
    let c = math_box(r"\left\{ x \middle| \frac{a}{b} \right.");
    consistent(&c, "middle");
    let c = math_box(r"\left. \frac{a}{b} \right|_{0}^{1}");
    consistent(&c, "evaluation bar");
    for tex in [r"\big( \Big[ \bigg\{ \Bigg\langle x \Bigg\rangle \bigg\} \Big] \big)", r"\bigl( x \bigr)"] {
        consistent(&math_box(tex), tex);
    }
    // an out-of-range size is clamped
    let node = Node::FixedDelimiter { symbol: '(', size: 9, class: Class::Open };
    consistent(&compose_node(&node, Style::display(), true).unwrap(), "tamanho 9");
}

#[test]
fn tall_delimiter_assembles_parts() {
    let rows = vec!["x"; 50].join(r"\\");
    let tex = format!(r"\left( \begin{{matrix}} {rows} \end{{matrix}} \right)");
    let c = math_box(&tex);
    let parens = c.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    assert!(parens > 50 + 2, "assembly with several parts");
    assert!(total(&c) > 40.0 * 1000.0, "height of 50 rows: {}", total(&c));
    svg(&tex);
}

#[test]
fn lgroup_grows_from_the_brace_halves() {
    // the font has ⟮ ⟯ at one size: a tall \lgroup is built as cmex builds it (physics' \Pmqty)
    let matrix = math_box(r"\begin{matrix} a \\ b \\ c \\ d \end{matrix}");
    let grouped = math_box(r"\left\lgroup \begin{matrix} a \\ b \\ c \\ d \end{matrix} \right\rgroup");
    assert!(total(&grouped) >= total(&matrix), "{} < {}", total(&grouped), total(&matrix));
    assert_eq!(total(&math_box(r"\left\lgroup x \right\rgroup")), total(&math_box(r"\lgroup x \rgroup")), "at the size of a letter, the glyph");
}

// ---------- accents, bars, braces, arrows ----------

#[test]
fn simple_stretchy_and_combining_accents() {
    for tex in [r"\hat{x}", r"\vec{AB}", r"\widehat{ABC}", r"\overrightarrow{AB}", r"\dot{\frac{a}{b}}", r"\tilde{\mathrm{ab}}", r"\overleftrightarrow{PQ}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(c.height > math_box("x").height, "{tex}: the accent goes above");
    }
    // an accent without an accent attachment in the font: center of the drawing
    let node = Node::Accent { base: Box::new(ident("x")), accent: '→', stretchy: false, below: false };
    consistent(&compose_node(&node, Style::display(), true).unwrap(), "arrow as an accent");
    let node = Node::Accent { base: Box::new(ident("x")), accent: '\u{0302}', stretchy: false, below: false };
    consistent(&compose_node(&node, Style::display(), true).unwrap(), "combinante");
}

#[test]
fn drawn_wide_accents_and_marks_under() {
    let rules = |b: &MathBox| b.items.iter().filter(|i| matches!(i, Item::Rule { .. })).count();
    let ab = math_box("AB");
    // KaTeX's line segment: a line and a tick across each end, over or under the base
    let over = math_box(r"\overlinesegment{AB}");
    let under = math_box(r"\underlinesegment{AB}");
    assert_eq!((rules(&over), rules(&under)), (3, 3));
    assert!(over.height > ab.height && over.depth == ab.depth && over.width == ab.width);
    assert!(under.depth > ab.depth && under.height == ab.height);
    consistent(&over, "overlinesegment");
    // mathabx's \widebar: one rule as wide as the base, where \bar puts its macron (below \overline's bar)
    for base in ["x", "AB", "ABCDEF"] {
        let rule = |tex: String| match math_box(&tex).items.iter().find(|i| matches!(i, Item::Rule { .. })) {
            Some(Item::Rule { width, y, .. }) => (*width, *y),
            _ => panic!("{tex}: no rule"),
        };
        let (wide, overline) = (rule(format!(r"\widebar{{{base}}}")), rule(format!(r"\overline{{{base}}}")));
        assert!((wide.0 - overline.0).abs() < 1e-3 && wide.1 <= overline.1, "{base}: {wide:?} {overline:?}");
    }
    // a mark of its own size under the base (accents' \underaccent), not stretched to the base
    let x = math_box("x");
    let under_tilde = math_box(r"\underaccent{\tilde}{x}");
    assert!(under_tilde.depth > x.depth && under_tilde.height == x.height, "{under_tilde:?}");
    let glyphs = |b: &MathBox| b.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    assert_eq!(glyphs(&math_box(r"\underaccent{\hat}{ABCDEF}")), 7, "the hat's own glyph, not a stretched one");
    consistent(&under_tilde, "underaccent");
}

#[test]
fn bar_above_and_below() {
    let base = math_box("x");
    let above = math_box(r"\overline{x}");
    let below = math_box(r"\underline{x}");
    assert!(above.height > base.height && above.depth == base.depth);
    assert!(below.depth > base.depth && below.height == base.height);
    assert_eq!(bars(&above), 1);
    assert_eq!(bars(&below), 1);
}

#[test]
fn braces_with_label() {
    let base = math_box("a+b+c");
    let top = math_box(r"\overbrace{a+b+c}^{n}");
    let low = math_box(r"\underbrace{a+b+c}_{n}");
    let without = math_box(r"\overbrace{a+b+c}");
    assert!(top.height > without.height && without.height > base.height);
    assert!(low.depth > base.depth);
    // label wider than the base
    let wide = math_box(r"\underbrace{x}_{\text{a really long label}}");
    assert!(wide.width > math_box("x").width * 5.0);
    let long = math_box(&format!(r"\overbrace{{{}}}^{{k}}", "a+".repeat(60) + "a"));
    consistent(&long, "overbrace longo");
}

/// Lowest and highest ink of some glyphs of a box, from the font's outlines.
fn ink(items: &[Item]) -> (f32, f32) {
    let f = context_equation_core::font::font();
    items.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), item| match item {
        Item::Glyph { id, y, scale, .. } => {
            let (_, y0, _, y1) = f.limits(*id);
            (lo.min(y + y0 * scale), hi.max(y + y1 * scale))
        }
        _ => (lo, hi),
    })
}

/// Leftmost and rightmost ink of some glyphs of a box.
fn ink_x(items: &[Item]) -> (f32, f32) {
    let f = context_equation_core::font::font();
    items.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), item| match item {
        Item::Glyph { id, x, scale, .. } => {
            let (x0, _, x1, _) = f.limits(*id);
            (lo.min(x + x0 * scale), hi.max(x + x1 * scale))
        }
        _ => (lo, hi),
    })
}

#[test]
fn braces_have_a_minimum_width() {
    // the font's first variant wider than the base glyph: over one character the brace is that
    // wide, centered on the base, and the box widens to hold it (TeX gives braces a minimum too)
    let f = context_equation_core::font::font();
    for (tex, symbol) in [(r"\underbrace{7}_{n}", '⏟'), (r"\overbrace{7}^{n}", '⏞'), (r"\underbrace{7}", '⏟'), (r"\underbracket{7}_{n}", '⎵'), (r"\overbracket{7}^{n}", '⎴')]
    {
        let id = f.glyph(symbol).unwrap();
        let width = |g| {
            let (x0, _, x1, _) = f.limits(g);
            x1 - x0
        };
        let minimum = f.variants(id, false).into_iter().map(|(v, _)| width(v)).find(|w| *w > width(id)).unwrap();
        let base = math_box("7");
        let b = math_box(tex);
        let n = base.items.len();
        let (left, right) = ink_x(&b.items[n..n + 1]);
        assert!(right - left >= minimum - 1.0 && right - left > base.width * 1.5, "{tex}: brace {} wide, minimum {minimum}", right - left);
        assert!(b.width >= right - left - 1.0, "{tex}: box {} narrower than its brace {}", b.width, right - left);
        let (base_left, base_right) = ink_x(&b.items[..n]);
        let center = |(l, r): (f32, f32)| (l + r) / 2.0;
        assert!((center((left, right)) - center((base_left, base_right))).abs() < 0.15 * base.width, "{tex}: the brace is not centered on its base");
        consistent(&b, tex);
    }
    // in the SVG: 7 is 0.495em; under its brace the equation is as wide as the brace's 1em variant
    let width = |svg: &str| svg.split("width=\"").nth(1).unwrap().split("em\"").next().unwrap().parse::<f32>().unwrap();
    assert!(width(&svg(r"\underbrace{7}_{n}")) >= 1.0, "{}", width(&svg(r"\underbrace{7}_{n}")));
    // a sequence of short braces takes the room of their braces, not of their digits
    assert!(math_box(r"\underbrace{3}_{a} \ \underbrace{1}_{b} \ \underbrace{4}_{c}").width > 3.0 * 900.0);
    // a wide base is unchanged: the brace stretches to it
    let wide = math_box(r"\underbrace{a+b+c+d+e+f}_{n}");
    assert!((wide.width - math_box("a+b+c+d+e+f").width).abs() < 1.0);
}

#[test]
fn overbrace_mirrors_underbrace() {
    // ⏞ floats above the baseline in the font and ⏟ below it: the gaps are measured on the ink,
    // so both braces sit as far from their content, and their labels as far from the brace
    for content in ["a+b", "x", "1+2+\\cdots+n"] {
        let base = math_box(content);
        let (base_lo, base_hi) = ink(&base.items);
        let n = base.items.len();
        let top = math_box(&format!(r"\overbrace{{{content}}}^{{k}}"));
        let low = math_box(&format!(r"\underbrace{{{content}}}_{{k}}"));
        let (top_brace, top_label) = (ink(&top.items[n..top.items.len() - 1]), ink(&top.items[top.items.len() - 1..]));
        let (low_brace, low_label) = (ink(&low.items[n..low.items.len() - 1]), ink(&low.items[low.items.len() - 1..]));
        let gap_above = top_brace.0 - base_hi;
        let gap_below = base_lo - low_brace.1;
        assert!(gap_above > 100.0 && (gap_above - gap_below).abs() < 1.0, "{content}: brace {gap_above} above, {gap_below} below");
        let label_above = top_label.0 - top_brace.1;
        let label_below = low_brace.0 - low_label.1;
        assert!(label_above > 100.0 && label_below > 100.0, "{content}: label {label_above} above, {label_below} below");
    }
}

#[test]
fn labeled_arrows() {
    let top = math_box(r"\xrightarrow{\text{heat}}");
    let both = math_box(r"\xrightarrow[\text{below}]{\text{above}}");
    let short = math_box(r"\xrightarrow{}");
    assert!(top.width > short.width);
    assert!(both.depth > top.depth);
    consistent(&math_box(r"\xleftarrow[x]{}"), "below only");
}

#[test]
fn stacked_and_overlapped() {
    let s = math_box(r"\stackrel{\text{def}}{=}");
    assert!(s.height > math_box("=").height);
    let u = math_box(r"\underset{x}{\arg\min}");
    assert!(u.depth > 0.0);
    let base = math_box("x").width;
    for (tex, expected) in [(r"\rlap{x}", 0.0), (r"\llap{x}", 0.0), (r"\clap{x}", 0.0)] {
        let c = math_box(tex);
        assert_eq!(c.width, expected, "{tex} takes no width");
        assert!(!c.items.is_empty());
    }
    let node = |side| Node::Overlapped { base: Box::new(ident("x")), side };
    let xs: Vec<f32> = ['l', 'c', 'r']
        .iter()
        .map(|l| match compose_node(&node(*l), Style::display(), true).unwrap().items[0] {
            Item::Glyph { x, .. } => x,
            _ => unreachable!(),
        })
        .collect();
    assert!((xs[0] + base).abs() < 1.0 && (xs[1] + base / 2.0).abs() < 1.0 && xs[2] == 0.0, "{xs:?}");
}

#[test]
fn phantoms_smashed_and_raised() {
    let x = math_box("x");
    let ph = math_box(r"\phantom{x}");
    let hph = math_box(r"\hphantom{x}");
    let vph = math_box(r"\vphantom{x}");
    assert!(ph.items.is_empty() && ph.width == x.width && ph.height == x.height);
    assert!(hph.height == 0.0 && hph.width == x.width);
    assert!(vph.width == 0.0 && vph.height == x.height);
    let node = Node::Phantom { base: Box::new(ident("x")), width: false, height: false };
    let c = compose_node(&node, Style::display(), true).unwrap();
    assert!(c.width == 0.0 && total(&c) == 0.0);

    let y = math_box("y");
    let smash = math_box(r"\smash{y}");
    let top = math_box(r"\smash[t]{y}");
    let bottom = math_box(r"\smash[b]{y}");
    assert!(total(&smash) == 0.0);
    assert!(top.height == 0.0 && top.depth == y.depth);
    assert!(bottom.depth == 0.0 && bottom.height == y.height);

    let tall = math_box(r"\raisebox{0.5em}{x}");
    assert!(tall.height > x.height);
    let bar = math_box(r"\rule{1em}{0.5em}");
    assert!(bar.width > 0.0 && bar.height > 0.0);
}

#[test]
fn cancels_and_frames() {
    assert_eq!(lines(&math_box(r"\cancel{x}")), 1);
    assert_eq!(lines(&math_box(r"\bcancel{x}")), 1);
    assert_eq!(lines(&math_box(r"\xcancel{x}")), 2);
    let x = math_box("x");
    let boxed = math_box(r"\boxed{x}");
    assert!(boxed.width > x.width && boxed.height > x.height && boxed.depth > x.depth);
    assert_eq!(bars(&boxed), 4);
    let circ = math_box(r"\textcircled{1}");
    assert!(circ.items.iter().any(|i| matches!(i, Item::Ellipse { .. })));
}

#[test]
fn slashed_overstrikes_a_slash_and_keeps_the_width() {
    let glyphs = |b: &MathBox| b.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    // slashed.sty: the slash as a glyph over the letter, in a box as wide as the wider of the two
    for letter in ["D", "p", "A"] {
        let plain = math_box(letter);
        let slashed = math_box(&format!(r"\slashed{{{letter}}}"));
        assert_eq!(glyphs(&slashed), 2, "{letter}");
        assert_eq!(lines(&slashed), 0, "{letter}: a glyph, not a drawn line");
        assert!((slashed.width - plain.width).abs() < 1e-4, "{letter}: {} vs {}", slashed.width, plain.width);
    }
    // a slash wider than the letter (i) sets the width, both centered
    let i = math_box("i");
    let slash = math_box("/");
    assert!((math_box(r"\slashed{i}").width - i.width.max(slash.width)).abs() < 1e-4);
    // the spacing around it is a letter's
    assert!((math_box(r"a\slashed{D}b").width - math_box("aDb").width).abs() < 1e-4);
}

#[test]
fn colored_boxes_and_colored_frames() {
    let x = math_box("x");
    // \colorbox: a filled rectangle behind the text, which keeps the page color
    let c = math_box(r"\colorbox{yellow}{x}");
    assert!(c.width > x.width && c.height > x.height);
    let Item::Rule { width, height, color: Some(color), .. } = &c.items[0] else { panic!("{:?}", c.items) };
    assert_eq!(color, "yellow");
    assert!(*width >= c.width - 0.01 && *height >= total(&c) - 0.01, "the background covers the whole box");
    assert!(c.items.iter().any(|i| matches!(i, Item::Glyph { color: None, .. })), "the text is not painted");
    assert_eq!(bars(&c), 1, "no frame lines");
    // \fcolorbox: the same background, and four lines in the frame color
    let f = math_box(r"\fcolorbox{red}{yellow}{x}");
    assert!(matches!(&f.items[0], Item::Rule { color: Some(c), .. } if c == "yellow"));
    let red = f.items.iter().filter(|i| matches!(i, Item::Rule { color: Some(c), .. } if c == "red")).count();
    assert_eq!(red, 4);
    assert!(svg(r"\fcolorbox{red}{yellow}{x}").contains(r#"fill="yellow""#));
}

#[test]
fn rcases_brace_is_after_the_content() {
    let r = math_box(r"\begin{rcases} a \\ b \end{rcases}");
    let l = math_box(r"\begin{cases} a \\ b \end{cases}");
    assert!((r.width - l.width).abs() < l.width * 0.2, "same parts, mirrored");
    // the brace is the rightmost glyph
    let x_of = |c: &MathBox| -> Vec<f32> { c.items.iter().filter_map(|i| if let Item::Glyph { x, .. } = i { Some(*x) } else { None }).collect() };
    let rx = x_of(&r);
    let lx = x_of(&l);
    assert_eq!(rx.iter().cloned().fold(f32::MIN, f32::max), *rx.last().unwrap());
    assert_eq!(lx.iter().cloned().fold(f32::MAX, f32::min), lx[0]);
}

#[test]
fn bracket_above_and_arrow_below() {
    let base = math_box("abc");
    let over = math_box(r"\overbracket{abc}");
    let under = math_box(r"\underbracket{abc}");
    assert!(over.height > base.height && over.depth == base.depth);
    assert!(under.depth > base.depth && under.height == base.height);
    // a bracket is drawn with another glyph than the brace
    assert_ne!(svg(r"\overbracket{abc}"), svg(r"\overbrace{abc}"));
    let ab = math_box("AB");
    let arrow = math_box(r"\underrightarrow{AB}");
    assert!(arrow.depth > ab.depth && (arrow.height - ab.height).abs() < 0.01, "the arrow goes below");
    assert!(arrow.width >= ab.width);
    consistent(&math_box(r"\underleftarrow{\text{a long base}}"), "long arrow below");
}

#[test]
fn small_capitals_are_capitals_at_x_height() {
    let big = math_box(r"\text{S}");
    let small = math_box(r"\textsc{s}");
    let lower = math_box(r"\text{s}");
    assert!(small.height < big.height && small.height > lower.height * 0.9, "{} {} {}", small.height, big.height, lower.height);
    assert_ne!(svg(r"\textsc{s}"), svg(r"\text{s}"));
}

#[test]
fn color_and_quantity() {
    let c = math_box(r"\color{red}{x} \textcolor{blue}{y}");
    assert!(c.items.iter().any(|i| matches!(i, Item::Glyph { color: Some(_), .. })));
    consistent(&math_box(r"\SI{3}{m/s}"), "unidade");
    consistent(&math_box(r"\si{\kilo\gram}"), "si");
}

// ---------- tables ----------

#[test]
fn all_matrix_environments() {
    let x = math_box(r"\begin{matrix} a & b \\ c & d \end{matrix}");
    for env in ["pmatrix", "bmatrix", "Bmatrix", "vmatrix", "Vmatrix"] {
        let tex = format!(r"\begin{{{env}}} a & b \\ c & d \end{{{env}}}");
        let c = math_box(&tex);
        assert!(c.width > x.width, "{env}: delimiters add width");
    }
    let small = math_box(r"\begin{smallmatrix} a & b \\ c & d \end{smallmatrix}");
    assert!(small.width < x.width);
}

#[test]
fn aligned_and_centered_cases() {
    for tex in [
        r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & \text{otherwise} \end{cases}",
        r"\begin{align} a &= b \\ c &= d & e &= f \end{align}",
        r"\begin{multline} a + b \\ = c \end{multline}",
        r"\begin{gather} a \\ bb \end{gather}",
        r"\begin{flalign} a &= b \end{flalign}",
        r"\begin{align*} a &= b \intertext{e também} c &= d \end{align*}",
        r"\displaylines{a \\ b}",
        r"\sum_{\substack{i < n \\ j < m}} a_{ij}",
        r"\begin{subarray}{l} a \\ b \end{subarray}",
        r"\bordermatrix{ & x & y \cr a & 1 & 2 \cr b & 3 & 4}",
        r"\bordermatrix{ & x \cr}",
        r"\begin{CD} A @>f>> B \\ @VgVV @VVhV \\ C @>>k> D \end{CD}",
    ] {
        let c = math_box(tex);
        consistent(&c, tex);
        svg(tex);
    }
}

/// The strokes of an arrow diagram as segments.
fn segments(c: &MathBox) -> Vec<(f32, f32, f32, f32)> {
    c.items.iter().filter_map(|i| if let Item::Line { x1, y1, x2, y2, .. } = i { Some((*x1, *y1, *x2, *y2)) } else { None }).collect()
}

#[test]
fn arrow_diagram_geometry() {
    // one horizontal arrow: every stroke sits between the cells, the shaft level with the math axis
    let c = math_box(r"\begin{tikzcd} A \arrow[r] & B \end{tikzcd}");
    consistent(&c, "tikzcd");
    let shaft = segments(&c).into_iter().max_by(|a, b| (a.2 - a.0).abs().total_cmp(&(b.2 - b.0).abs())).unwrap();
    assert!((shaft.1 - shaft.3).abs() < 1e-3, "{shaft:?}");
    assert!(shaft.2 - shaft.0 > 1500.0, "at least 1.5 em long: {shaft:?}");
    let glyphs: Vec<f32> = c.items.iter().filter_map(|i| if let Item::Glyph { x, .. } = i { Some(*x) } else { None }).collect();
    assert!(shaft.0 > glyphs[0] && shaft.2 < glyphs[1], "{shaft:?} {glyphs:?}");
    // centered on the axis, like a matrix
    assert!((c.height - c.depth - 2.0 * 250.0).abs() < 200.0, "{} {}", c.height, c.depth);

    // a diagonal arrow goes right and down
    let c = math_box(r"\begin{tikzcd} A \arrow[dr] & \\ & B \end{tikzcd}");
    let s = segments(&c);
    let longest = s.iter().max_by(|a, b| ((a.2 - a.0).hypot(a.3 - a.1)).total_cmp(&(b.2 - b.0).hypot(b.3 - b.1))).unwrap();
    assert!(longest.2 > longest.0 && longest.3 < longest.1, "{longest:?}");

    // a bent arrow rises above the straight one; dashes, dots and double lines cost more strokes
    let top = |tex: &str| segments(&math_box(tex)).iter().map(|s| s.1.max(s.3)).fold(f32::MIN, f32::max);
    assert!(top(r"\begin{tikzcd} A \arrow[r, bend left] & B \end{tikzcd}") > top(r"\begin{tikzcd} A \arrow[r] & B \end{tikzcd}") + 100.0);
    let count = |style: &str| lines(&math_box(&format!(r"\begin{{tikzcd}} A \arrow[r, {style}] & B \end{{tikzcd}}")));
    let solid = count("rightarrow");
    for style in ["dashed", "dotted", "Rightarrow", "squiggly", "two heads", "hook", "mapsto", "tail", "leftrightarrow"] {
        assert!(count(style) > solid, "{style}");
    }
    assert!(count("no head") < solid);
    assert_eq!(count("phantom"), 0);

    // a label beside a horizontal arrow sits above it, and widens the column gap when it is long
    let short = math_box(r#"\begin{tikzcd} A \arrow[r, "f"] & B \end{tikzcd}"#);
    let long = math_box(r#"\begin{tikzcd} A \arrow[r, "f \circ g \circ h \circ k"] & B \end{tikzcd}"#);
    assert!(long.width > short.width + 1000.0);
    assert!(short.height > c_height(r"\begin{tikzcd} A \arrow[r] & B \end{tikzcd}"));
    // a description label breaks the line under it
    let plain = lines(&math_box(r#"\begin{tikzcd} A \arrow[r, "f"] & B \end{tikzcd}"#));
    let on = math_box(r#"\begin{tikzcd} A \arrow[r, "f" description] & B \end{tikzcd}"#);
    assert!(lines(&on) > plain, "the line is cut into pieces around the label");
    for tex in [
        r"\begin{tikzcd} A \arrow[r, shift left] \arrow[r, shift right] & B \end{tikzcd}",
        r#"\begin{tikzcd} A \arrow[d, bend right=80, "x"'] \\ B \end{tikzcd}"#,
        r"\begin{tikzcd} {} \arrow[r] & {} \end{tikzcd}",
        r#"\begin{tikzcd} A \arrow[dr, phantom, "\lrcorner" very near start] \arrow[r, "f" at end] & B \\ C & D \end{tikzcd}"#,
        r"\xymatrix{ A \ar@{.>}[r] \ar@{~>}[d] & B \\ C }",
        r"x_{\xymatrix{ A \ar[r]^f & B }}",
    ] {
        consistent(&math_box(tex), tex);
        svg(tex);
    }
}

fn c_height(tex: &str) -> f32 {
    math_box(tex).height
}

#[test]
fn array_with_rules_and_separators() {
    let tex = r"\begin{array}{|l|c|r|} \hline a & b & c \\ \cline{1-2} dd & ee & ff \\ \hline \end{array}";
    let c = math_box(tex);
    assert!(bars(&c) >= 3 + 2);
    let tex = r"\begin{array}{r@{,}l} 3 & 14 \\ 10 & 5 \end{array}";
    let c = math_box(tex);
    assert!(c.width > 0.0);
    // separator and vertical rule on the same boundary
    consistent(&math_box(r"\begin{array}{c|@{:}c} a & b \end{array}"), "| e @ juntos");
    consistent(&math_box(r"\begin{tabular}{ll} a & b \end{tabular}"), "tabular");
}

#[test]
fn multicolumn_widens_the_table() {
    let narrow = math_box(r"\begin{array}{cc} a & b \\ c & d \end{array}");
    let wide = math_box(r"\begin{array}{|c|c|} \multicolumn{2}{|c|}{\text{a fairly long title here}} \\ c & d \end{array}");
    assert!(wide.width > narrow.width * 2.0);
    for alignment in ["l", "r", "c"] {
        let tex = format!(r"\begin{{array}}{{cc}} \multicolumn{{2}}{{{alignment}}}{{x}} \\ aaa & bbb \end{{array}}");
        consistent(&math_box(&tex), &tex);
    }
}

// ---------- arithmetic ----------

#[test]
fn division_with_zero_product_steps() {
    // 1005 ÷ 5: steps with a zero product in the middle are skipped, the last one stays
    for (dividend, divisor) in [("1005", "5"), ("100", "4"), ("7", "3"), ("1000", "10"), ("3", "7")] {
        let tex = format!(r"\divchave{{{dividend}}}{{{divisor}}}");
        let c = math_box(&tex);
        consistent(&c, &tex);
        assert!(bars(&c) >= 2, "{tex}: the bracket has two rules");
    }
}

#[test]
fn column_arithmetic_layouts() {
    for tex in [r"\contaarmada{+}{123}{45}", r"\contaarmada{-}{1000}{1}", r"\contaarmada{\times}{12}{34}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(bars(&c) >= 1, "{tex}");
    }
}

// ---------- hand-built nodes (the public API accepts trees the reader does not produce) ----------

#[test]
fn hand_built_text_uses_font_or_equivalents() {
    use context_equation_core::tree::TextPart;
    let node = Node::Text { parts: vec![TextPart::Text("‘a’ “b” – — c\u{00A0}d\u{2009}e\u{202F}f ✔ ✗".into()), TextPart::Math(ident("x"))] };
    let c = compose_node(&node, Style::display(), true).expect("they all have an equivalent in the font");
    assert!(c.width > 0.0);
    let node = Node::Text { parts: vec![TextPart::Text("🙂".into())] };
    assert!(matches!(compose_node(&node, Style::display(), true), Err(Error::MissingGlyph { character: '🙂' })));
}

#[test]
fn multicolumn_outside_table_is_just_content() {
    let node = Node::Multicolumn { columns: 3, alignment: 'c', base: Box::new(ident("x")) };
    let c = compose_node(&node, Style::display(), true).unwrap();
    assert_eq!(c.width, math_box("x").width);
}

fn table(rows: Vec<Vec<Node>>, verticals: Vec<usize>, between: Vec<(usize, Node)>) -> Node {
    Node::Table {
        shape: context_equation_core::tree::TableKind::Array,
        rows,
        columns: vec!['c', 'c'],
        verticals,
        dashed: vec![],
        horizontals: vec![],
        between,
        intertexts: vec![],
        stretch: 1.0,
        colsep: 500,
    }
}

#[test]
fn rules_and_separators_past_last_column_are_ignored() {
    let rows = vec![vec![ident("a"), ident("b")]];
    let base = compose_node(&table(rows.clone(), vec![], vec![]), Style::display(), true).unwrap();
    let outside = compose_node(&table(rows, vec![7], vec![(9, ident("z"))]), Style::display(), true).unwrap();
    assert_eq!(base.width, outside.width);
    assert_eq!(bars(&outside), 0);
}

#[test]
fn wide_multicolumn_in_the_middle_pushes_later_columns() {
    let tex = r"\begin{array}{|c|c|c|} \multicolumn{2}{|c|}{\text{a really long title}} & z \\ a & b & c \end{array}";
    let c = math_box(tex);
    let short = math_box(r"\begin{array}{|c|c|c|} x & y & z \\ a & b & c \end{array}");
    assert!(c.width > short.width);
    // the last vertical rule goes at the end of the table
    let xs: Vec<f32> = c.items.iter().filter_map(|i| if let Item::Rule { x, height, width, .. } = i { (height > width).then_some(*x) } else { None }).collect();
    let max = xs.iter().copied().fold(f32::MIN, f32::max);
    assert!(max > c.width * 0.9, "{xs:?} {}", c.width);
}

#[test]
fn delimiter_without_assembly_stays_at_largest_variant() {
    // ⟨ ⟩ have no parts to assemble in the font: they stop at the largest ready-made variant
    let rows = vec!["x"; 50].join(r"\\");
    let tall = math_box(&format!(r"\left\langle \begin{{matrix}} {rows} \end{{matrix}} \right\rangle"));
    let medium = math_box(&format!(r"\left\langle \begin{{matrix}} {} \end{{matrix}} \right\rangle", vec!["x"; 25].join(r"\\")));
    let glyphs = |c: &MathBox| c.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    assert_eq!(glyphs(&tall), 52, "one glyph per side");
    let delim_width = |c: &MathBox| c.width - math_box(r"\begin{matrix} x \end{matrix}").width;
    assert!((delim_width(&tall) - delim_width(&medium)).abs() < 1.0);
}

#[test]
fn hand_built_arithmetic_with_invalid_data() {
    let e = Style::display();
    let div = |a: &str, b: &str| compose_node(&Node::LongDivision { dividend: a.into(), divisor: b.into() }, e, true);
    assert!(matches!(div("12", "0"), Err(Error::Syntax { .. })), "division by zero is an error, not a panic");
    assert!(div("12", "x").is_err());
    // long division never accumulates more than the divisor: a 60-digit dividend fits
    let long = div(&"9".repeat(60), "7").unwrap();
    assert!(long.width > 60.0 * math_box("9").width);
    assert!(div("1", &"9".repeat(40)).is_err(), "a divisor larger than u128 is an error");
    // a single digit smaller than the divisor: quotient 0
    let c = div("3", "7").unwrap();
    consistent(&c, "3÷7");
    let calculation = |op: char, t: &[&str]| compose_node(&Node::ColumnArithmetic { operation: op, terms: t.iter().map(|s| s.to_string()).collect() }, e, true);
    assert!(calculation('+', &["1"]).is_err(), "a term is missing");
    assert!(calculation('+', &["1", "2", "3"]).is_err());
    assert!(calculation('+', &[&"9".repeat(39), &"9".repeat(39)]).is_err(), "overflow in the sum");
    assert!(calculation('×', &[&"9".repeat(30), &"9".repeat(30)]).is_err(), "overflow in the product");
    let c = calculation('−', &["3", "10"]).unwrap();
    consistent(&c, "3 − 10 is negative");
}

// ---------- problem cases ----------

#[test]
fn degenerate_matrices() {
    for tex in [
        r"\begin{pmatrix} x \end{pmatrix}",
        r"\begin{pmatrix} \end{pmatrix}",
        r"\begin{matrix} & \\ & \end{matrix}",
        r"\begin{pmatrix} a & b & c \\ d \\ e & f \end{pmatrix}",
        r"\begin{array}{c} a & b & c \end{array}",
        r"\begin{array}{ccc} a \end{array}",
        r"\begin{array}{} \end{array}",
    ] {
        consistent(&math_box(tex), tex);
        svg(tex);
    }
    // short row: the matrix is as wide as the longest row
    let irregular = math_box(r"\begin{matrix} a & b & c \\ d \end{matrix}");
    let full = math_box(r"\begin{matrix} a & b & c \\ d & & \end{matrix}");
    assert!((irregular.width - full.width).abs() < 1.0);
}

#[test]
fn matrix_200_by_200() {
    let row = vec!["1"; 200].join("&");
    let tex = format!(r"\begin{{bmatrix}} {} \end{{bmatrix}}", vec![row; 200].join(r"\\"));
    let start = std::time::Instant::now();
    let c = math_box(&tex);
    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    consistent(&c, "200×200");
    assert!(c.width > 200.0 * math_box("1").width && total(&c) > 200.0 * total(&math_box("1")));
    svg(&tex);
}

#[test]
fn root_of_huge_height() {
    let rows = vec!["x"; 60].join(r"\\");
    let tex = format!(r"\sqrt{{\begin{{matrix}} {rows} \end{{matrix}}}}");
    let c = math_box(&tex);
    assert!(total(&c) > 50.0 * 1000.0);
    // the top rule covers the whole radicand
    let rule =
        c.items.iter().filter_map(|i| if let Item::Rule { width, y, .. } = i { Some((*width, *y)) } else { None }).fold((0.0f32, f32::MIN), |a, b| if b.1 > a.1 { b } else { a });
    assert!(rule.0 > 0.0 && rule.1 > c.height * 0.8, "{rule:?} {}", c.height);
    svg(&tex);
}

#[test]
fn fractions_nested_30_levels_stop_shrinking() {
    let mut tex = String::from("x");
    for _ in 0..30 {
        tex = format!(r"\frac{{{tex}}}{{y}}");
    }
    // reading recursively needs more stack than a test thread's 2 MB (debug):
    // read on a thread with a roomy stack and compose here, on the normal stack
    let tree = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn({
            let tex = tex.clone();
            move || context_equation_core::input::latex::read(&tex, Locale::PtBr, &mut Registry::default()).unwrap()
        })
        .unwrap()
        .join()
        .unwrap();
    let c = compose_node(&tree, Style::display(), true).unwrap();
    consistent(&c, "30 fractions");
    // from the third level on everything is scriptscript: the innermost letters do not vanish
    let smaller = c.items.iter().filter_map(|i| if let Item::Glyph { scale, .. } = i { Some(*scale) } else { None }).fold(f32::MAX, f32::min);
    let larger = c.items.iter().filter_map(|i| if let Item::Glyph { scale, .. } = i { Some(*scale) } else { None }).fold(0.0, f32::max);
    assert!(smaller >= larger * 0.4, "minimum scale {smaller}, maximum {larger}");
}

#[test]
fn scripts_nested_20_levels() {
    let mut sup = String::from("x");
    let mut sub = String::from("x");
    for _ in 0..20 {
        sup = format!("x^{{{sup}}}");
        sub = format!("x_{{{sub}}}");
    }
    for tex in [&sup, &sub] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(c.width < 20.0 * math_box("x").width, "{tex}: small scripts do not grow");
        svg(tex);
    }
    let tall = math_box(&sup);
    assert!(tall.height > math_box("x^x").height);
}

#[test]
fn table_rules_out_of_range() {
    for tex in [
        r"\begin{array}{cc} a & b \\ c & d \\ \hline \hline \end{array}",
        r"\begin{array}{cc} \cline{1-9} a & b \end{array}",
        r"\begin{array}{cc} \cline{5-7} a & b \end{array}",
        r"\begin{array}{cc} a & b \\ \cline{2-1} c & d \end{array}",
        r"\begin{array}{|c|} \multicolumn{5}{|c|}{x} \\ a \end{array}",
        r"\begin{array}{cc} \multicolumn{0}{c}{x} & y \end{array}",
    ] {
        let c = math_box(tex);
        consistent(&c, tex);
        for i in &c.items {
            if let Item::Rule { width, height, .. } = i {
                assert!(*width >= 0.0 && *height >= 0.0, "{tex}: negative rule {width}×{height}");
            }
        }
        svg(tex);
    }
}

#[test]
fn arithmetic_with_huge_numbers_gives_clear_error() {
    let large = "9".repeat(200);
    for tex in [format!(r"\contaarmada{{+}}{{{large}}}{{1}}"), format!(r"\divchave{{{large}}}{{7}}"), r"\divchave{12}{0}".to_string()] {
        let strict = Options { strict: true, ..options() };
        assert!(matches!(context_equation_core::typeset(&tex, &strict), Err(Error::Syntax { .. })), "{tex}");
    }
    // through the layout, the largest number that fits in u128 works
    let node = Node::ColumnArithmetic { operation: '+', terms: vec!["1".repeat(38), "1".into()] };
    consistent(&compose_node(&node, Style::display(), true).unwrap(), "38 algarismos");
}

#[test]
fn mathbb_of_character_without_double_struck_form() {
    // digits and lowercase letters have a double-struck form; Greek and symbols stay as they are
    for tex in [r"\mathbb{1}", r"\mathbb{k}", r"\mathbb{\alpha}", r"\mathbb{+}", r"\mathbb{ж}", r"\mathfrak{3}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(c.width > 0.0, "{tex}");
    }
}

#[test]
fn emoji_and_characters_outside_the_font() {
    for tex in ["🙂", r"\text{🙂 ok}", "x^{🙂}", r"\frac{🙂}{2}", "\u{10FFFF}", r"\sqrt{🎉}"] {
        let c = math_box(tex);
        consistent(&c, tex);
        assert!(!warnings(tex).is_empty(), "{tex}: warns about the box");
        svg(tex);
    }
}

#[test]
fn unbalanced_and_empty_delimiters() {
    for tex in [r"\left( \right)", r"\left. \right.", r"\left( x", r"x \right)", r"\left\{ \begin{matrix} \end{matrix} \right."] {
        let c = math_box(tex);
        consistent(&c, tex);
        svg(tex);
    }
}

#[test]
fn accents_and_braces_over_empty() {
    for tex in
        [r"\hat{}", r"\widehat{}", r"\overbrace{}^{x}", r"\underbrace{}", r"\xrightarrow{}", r"\sqrt{}", r"\frac{}{}", r"\overline{}", r"\boxed{}", r"\cancel{}", r"\textcircled{}"]
    {
        let c = math_box(tex);
        consistent(&c, tex);
        svg(tex);
    }
}

#[test]
fn accent_over_nothing_does_not_leak_left() {
    for tex in [r"\hat{}", r"\vec{}", r"\widehat{}"] {
        let c = math_box(tex);
        assert!(c.width > 0.0, "{tex}");
        for i in &c.items {
            if let Item::Glyph { x, .. } = i {
                assert!(*x >= 0.0, "{tex}: accent at x={x}");
            }
        }
    }
}

#[test]
fn wide_accent_without_assembly_stops_at_largest_variant() {
    // the wide hat has ready-made variants, but no parts to assemble
    let short = math_box(r"\widehat{ABCDEFGHIJ}");
    let long = math_box(&format!(r"\widehat{{{}}}", "ABCDEFGHIJ".repeat(5)));
    let hat = |c: &MathBox| {
        c.items
            .iter()
            .filter_map(|i| if let Item::Glyph { y, id, .. } = i { Some((*y, *id)) } else { None })
            .fold((f32::MIN, None), |a, b| if b.0 > a.0 { (b.0, Some(b.1)) } else { a })
            .1
    };
    assert_eq!(hat(&short), hat(&long), "the same glyph, the largest there is");
    consistent(&long, "widehat longo");
}

#[test]
fn scaling_a_glyph_carries_the_accent_point() {
    let x = compose_node(&ident("x"), Style::display(), true).unwrap();
    let node = Node::Scaled { factor: 3.0, vertical: None, base: Box::new(ident("x")) };
    let c = compose_node(&node, Style::display(), true).unwrap();
    assert!((c.accent_point.unwrap() - 3.0 * x.accent_point.unwrap()).abs() < 0.01);
    assert!(!c.single_glyph);
}

#[test]
fn identifier_without_glyph_in_any_form() {
    let node = Node::Identifier { name: "🙂".into(), variant: Variant::DoubleStruck };
    assert!(matches!(compose_node(&node, Style::display(), true), Err(Error::MissingGlyph { .. })));
    assert!(compose_node(&node, Style::display(), false).unwrap().width > 0.0);
    let calculation = Node::ColumnArithmetic { operation: '+', terms: vec!["1".into(), "x".into()] };
    assert!(compose_node(&calculation, Style::display(), true).is_err());
}

#[test]
fn dots_are_three_dots_set_as_tex_sets_them() {
    // \mathinner{\ldotp\ldotp\ldotp}: a thin space between the dots, none in a script
    let f = context_equation_core::font::font();
    let period = f.glyph('.').unwrap();
    let thin = f.units_per_em() / 6.0;
    let text = inline_box(r"\ldots");
    assert!((text.width - (3.0 * f.advance(period) + 2.0 * thin)).abs() < 0.5, "{}", text.width);
    let glyphs = |b: &MathBox, id| b.items.iter().filter(|i| matches!(i, Item::Glyph { id: g, .. } if *g == id)).count();
    assert_eq!(glyphs(&text, period), 3);
    let script = f.script_form(period, 1);
    assert_eq!(glyphs(&inline_box(r"x_{\ldots}"), script), 3);
    assert_eq!(glyphs(&inline_box(r"\cdots"), f.glyph('⋅').unwrap()), 3);
}

#[test]
fn text_is_kerned_as_tex_kerns_it() {
    use context_equation_core::layout::kerning::TextKerning;
    let width = |tex: &str, kerning: TextKerning| context_equation_core::typeset(tex, &Options { text_kerning: kerning, ..options() }).unwrap().0.width;
    let plain = width(r"\text{va}", TextKerning::None);
    let ot1 = width(r"\text{va}", TextKerning::Ot1);
    let t1 = width(r"\text{va}", TextKerning::T1);
    // cmr10 kerns `va` by 0.0278 em, ec-lmr10 by twice that
    assert!(plain > ot1 && ot1 > t1, "{plain} {ot1} {t1}");
    assert!(((plain - t1) - 2.0 * (plain - ot1)).abs() < 0.01 * plain, "{plain} {ot1} {t1}");
    // nothing between letters a space apart, nor in math
    assert_eq!(width(r"\text{v a}", TextKerning::None), width(r"\text{v a}", TextKerning::T1));
    assert_eq!(width("va", TextKerning::None), width("va", TextKerning::T1));
    // bold by its own table
    assert!(width(r"\text{\textbf{AV}}", TextKerning::None) > width(r"\text{\textbf{AV}}", TextKerning::Ot1));
}

/// Left edge and right edge of the glyphs of `letter` (math italic) in a box.
fn letter_span(c: &MathBox, letter: char) -> (f32, f32) {
    let f = context_equation_core::font::font();
    let id = f.glyph(letter).unwrap();
    let xs: Vec<(f32, f32)> = c.items.iter().filter_map(|i| if let Item::Glyph { id: g, x, scale, .. } = i { (*g == id).then_some((*x, *scale)) } else { None }).collect();
    assert!(!xs.is_empty(), "{letter} is drawn");
    let start = xs.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let end = xs.iter().map(|p| p.0 + f.advance(id) * p.1).fold(f32::NEG_INFINITY, f32::max);
    (start, end)
}

#[test]
fn multline_first_line_left_last_right_shoved_lines_at_their_side() {
    let tex = r"\begin{multline} aaaaaaaa \\ b \\ \shoveleft{c} \\ \shoveright{d} \\ e \end{multline}";
    let c = math_box(tex);
    consistent(&c, tex);
    let width = c.width;
    // within 1/20 em
    let near = |x: f32, at: f32| (x - at).abs() < 50.0;
    let (a0, a1) = letter_span(&c, '𝑎');
    let (b0, b1) = letter_span(&c, '𝑏');
    let (c0, _) = letter_span(&c, '𝑐');
    let (_, d1) = letter_span(&c, '𝑑');
    let (_, e1) = letter_span(&c, '𝑒');
    // the first line is the widest: it spans the box
    assert!(near(a0, 0.0) && near(a1, width), "first line {a0}..{a1} in 0..{width}");
    // a middle line is centered; a shoved one goes to its side; the last line goes right
    assert!(near((b0 + b1) / 2.0, width / 2.0), "middle line {b0}..{b1} in 0..{width}");
    assert!(near(c0, 0.0), "\\shoveleft at {c0}");
    assert!(near(d1, width), "\\shoveright ends at {d1} of {width}");
    assert!(near(e1, width), "last line ends at {e1} of {width}");
    // multline* is the same layout, without a number
    let starred = math_box(r"\begin{multline*} aaaaaaaa \\ b \\ \shoveleft{c} \\ \shoveright{d} \\ e \end{multline*}");
    assert_eq!(starred.width, width);
    assert_eq!(letter_span(&starred, '𝑒'), (letter_span(&c, '𝑒')));
    // a short first line goes left, a short last line right
    let c = math_box(r"\begin{multline} a \\ bbbbbbbb \\ e \end{multline}");
    assert!(near(letter_span(&c, '𝑎').0, 0.0) && near(letter_span(&c, '𝑒').1, c.width));
    assert!(warnings(tex).is_empty(), "{:?}", warnings(tex));
}
