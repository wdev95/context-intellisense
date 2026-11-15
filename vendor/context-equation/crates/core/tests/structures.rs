//! Two-dimensional structures. Layout tests check geometric properties
//! (what sits above or below what), not exact coordinates.

use context_equation_core::layout::{compose, Context, Item, MathBox, Style};
use context_equation_core::rules::Registry;
use context_equation_core::tree::{Node, TableKind, Variant};
use context_equation_core::{convert, Locale, Options};

fn pt(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { locale: Locale::PtBr, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn math_box(tex: &str) -> MathBox {
    let mut registry = Registry::default();
    let tree = context_equation_core::input::latex::read(tex, Locale::PtBr, &mut registry).unwrap();
    let mut ctx = Context { locale: Locale::PtBr, registry: &mut registry, strict: true, kerning: Default::default(), fallbacks: &[] };
    compose(&tree, Style::display(), &mut ctx).unwrap()
}

fn rules(r: &context_equation_core::Output) -> Vec<&'static str> {
    r.applied.iter().map(|a| a.rule).collect()
}

/// y of the drawn glyphs, in order.
fn ys(c: &MathBox) -> Vec<f32> {
    c.items.iter().filter_map(|i| if let Item::Glyph { y, .. } = i { Some(*y) } else { None }).collect()
}

#[test]
fn l_num_3_thousands_dot_in_pt_br_is_kept() {
    let r = pt(r"5,3 \times 1.000");
    assert_eq!(r.text, "5,3 × 1.000");
    assert!(rules(&r).contains(&"L-NUM-3"));
    assert_eq!(pt("12.345,67").tree.children(), &[Node::Number { value: "12345.67".into(), group: false, thousands: Some('.'), variant: Variant::Upright }]);
    // a dot followed by two digits stays decimal
    assert_eq!(pt("3.14").text, "3,14");
}

#[test]
fn l_num_3_thousands_comma_in_english() {
    let r = convert("1,000.5", &Options { locale: Locale::En, ..Default::default() }).unwrap();
    assert_eq!(r.text, "1,000.5");
}

#[test]
fn l_int_1_brazilian_open_interval() {
    let r = pt("x \\in ]2, 5[");
    assert!(rules(&r).contains(&"L-INT-1"));
    assert_eq!(r.speech, "x pertence a abre intervalo 2 vírgula 5 fecha intervalo");
}

#[test]
fn l_uni_1_siunitx_with_slash() {
    let r = pt(r"\SI{9,8}{\meter\per\second\squared}");
    assert_eq!(r.text, "9,8 m/s^2");
    let r = pt(r"\SI{8,31}{\joule\per\mole\per\kelvin}");
    assert_eq!(r.text, "8,31 J/(mol·K)");
    assert_eq!(pt(r"\num{1234567}").text, "1234567");
}

#[test]
fn l_ent_1_and_2_html_entities_and_outer_delimiters() {
    let r = pt(r"$\frac{1}{2} &lt; 1$");
    assert!(rules(&r).contains(&"L-ENT-1"));
    assert!(rules(&r).contains(&"L-ENT-2"));
    assert_eq!(r.text, "1/2 < 1");
}

#[test]
fn l_conta_1_long_division_computes_quotient_and_remainder() {
    let r = pt(r"\divchave{247}{5}");
    assert_eq!(r.text, "247 ÷ 5 = 49 (resto 2)");
    assert_eq!(r.speech, "divisão armada de 247 por 5: quociente 49, resto 2");
    assert!(convert(r"\divchave{10}{0}", &Options { strict: true, ..Default::default() }).is_err());
}

#[test]
fn l_conta_2_column_arithmetic_computes_the_result() {
    assert_eq!(pt(r"\contaarmada{+}{347}{285}").text, "347 + 285 = 632");
    assert_eq!(pt(r"\contaarmada{\times}{123}{45}").text, "123 × 45 = 5535");
    assert_eq!(pt(r"\contaarmada{-}{50}{8}").text, "50 − 8 = 42");
}

#[test]
fn fraction_numerator_above_and_denominator_below_axis() {
    let c = math_box(r"\frac{a}{b}");
    let y = ys(&c);
    assert!(y[0] > 0.0, "numerador sobe: {y:?}");
    assert!(y[1] < 0.0, "denominador desce: {y:?}");
    assert!(c.items.iter().any(|i| matches!(i, Item::Rule { .. })), "has a rule");
}

#[test]
fn binomial_has_no_rule() {
    let c = math_box(r"\binom{n}{k}");
    assert!(!c.items.iter().any(|i| matches!(i, Item::Rule { .. })));
}

#[test]
fn root_has_rule_above_radicand() {
    let c = math_box(r"\sqrt{x}");
    let rule_top = c.items.iter().find_map(|i| if let Item::Rule { y, .. } = i { Some(*y) } else { None }).unwrap();
    assert!(rule_top > 300.0, "rule above the x: {rule_top}");
}

#[test]
fn parenthesis_grows_with_content() {
    let low = math_box(r"\left( x \right)").total();
    let tall = math_box(r"\left( \frac{\frac{a}{b}}{c} \right)").total();
    assert!(tall > 1.8 * low, "{tall} should be much larger than {low}");
}

#[test]
fn integral_subscript_stays_below_baseline() {
    let c = math_box(r"\int_a^b");
    let y = ys(&c);
    assert!(y[1] > 0.0, "b on top: {y:?}");
    assert!(y[2] < -300.0, "a below, far from the baseline: {y:?}");
}

#[test]
fn display_sum_has_limits_above_and_below() {
    let r = pt(r"\sum_{i=1}^{n} i");
    assert_eq!(r.speech, "somatório de i é igual a 1 até n de i");
    let c = math_box(r"\sum_{i=1}^{n}");
    // the n is centered over the ∑, not to its right
    let xs: Vec<f32> = c.items.iter().filter_map(|i| if let Item::Glyph { x, .. } = i { Some(*x) } else { None }).collect();
    assert!(xs[1] < c.width, "{xs:?}");
}

#[test]
fn f_prime_row_stays_normal_size() {
    let c = math_box("f'");
    let scales: Vec<f32> = c.items.iter().filter_map(|i| if let Item::Glyph { scale, .. } = i { Some(*scale) } else { None }).collect();
    assert!(scales.iter().all(|e| (*e - 1.0).abs() < 1e-6), "{scales:?}");
    assert_eq!(pt("f'(x)").text, "f′(x)");
}

#[test]
fn matrix_is_read_by_rows() {
    let r = pt(r"A = \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}");
    assert_eq!(r.speech, "A é igual a matriz 2 por 2: linha 1: 1, 2; linha 2: 3, 4");
    assert_eq!(r.text, "A = ([1, 2], [3, 4])");
}

#[test]
fn array_with_vertical_rule_and_cline_for_factoring() {
    let r = pt(r"\begin{array}{r|l} 30 & 2 \\ \cline{2-2} 15 & 3 \\ 5 & 5 \\ 1 & \end{array}");
    let Node::Table { shape, verticals, horizontals, .. } = &r.tree.children()[0] else { panic!("{:?}", r.tree) };
    assert_eq!(*shape, TableKind::Array);
    assert_eq!(verticals, &[1]);
    assert_eq!(horizontals.len(), 1);
}

#[test]
fn cases_with_text() {
    let r = pt(r"|x| = \begin{cases} x, & x \ge 0 \\ -x, & x < 0 \end{cases}");
    assert!(r.speech.starts_with("módulo de x é igual a casos: caso 1: x"), "{}", r.speech);
}

#[test]
fn speech_for_school_fractions_and_roots() {
    assert_eq!(pt(r"\frac{3}{4}").speech, "três quartos");
    assert_eq!(pt(r"\frac{1}{2}").speech, "um meio");
    assert_eq!(pt(r"\sqrt[3]{8} = 2").speech, "raiz cúbica de 8 é igual a 2");
    assert_eq!(pt(r"x^2").speech, "x ao quadrado");
    assert_eq!(pt(r"\lim_{x \to 0} f(x)").speech, "limite com x tendendo a 0 de f abre parênteses x fecha parênteses");
}

#[test]
fn absolute_value_with_stretchy_bars() {
    assert_eq!(pt(r"\left| x \right|").speech, "módulo de x");
}

#[test]
fn text_with_old_style_accents() {
    assert_eq!(pt(r#"\text{caf\'e na\"ive fa\c{c}ade}"#).text, "café naïve façade");
}

#[test]
fn basic_chemistry() {
    assert_eq!(pt(r"\ce{2H2 + O2 -> 2H2O}").text, "2 H_2 + O_2 → 2 H_2O");
}

#[test]
fn tex_errors_stay_errors() {
    let strict = Options { strict: true, ..Default::default() };
    for tex in [r"x_1_2", r"\frac{1}", r"\left( x", r"\begin{pmatrix} 1 \end{bmatrix}"] {
        assert!(convert(tex, &strict).is_err(), "{tex} should fail in strict mode");
        assert!(!convert(tex, &Options::default()).unwrap().warnings.is_empty(), "{tex} should warn in tolerant mode");
    }
}

#[test]
fn d_det_1_complex_equations_are_deterministic() {
    let tex = r"\int_0^\infty \frac{\sin x}{x}\,dx = \frac{\pi}{2} \quad \begin{pmatrix} a & b \\ c & d \end{pmatrix}";
    assert_eq!(pt(tex).svg, pt(tex).svg);
}

#[test]
fn assorted_constructions_render() {
    // one invented equation per kind of construction
    for tex in [
        r"u = \frac{-k \pm \sqrt{k^2 - 8m}}{4}",
        r"g(s) = \begin{cases} s - 1 & \text{if } s < 0 \\ \frac{s}{2} & \text{otherwise} \end{cases}",
        r"D = \det \begin{pmatrix} 2 & 1 & 0 \\ 0 & 3 & 1 \\ 1 & 0 & 2 \end{pmatrix}",
        r"\text{X}_2 \xrightarrow{\text{heat}} 2\,\text{X}",
        r"\underbrace{\text{AB}}_{\text{left}} + \underbrace{\text{C}}_{\text{right}}",
        r"\boxed{v = \frac{d}{t} = \frac{12}{2{,}5}}",
        r"\left\{ \begin{array}{cl} p = 2q \\ p + q = 6 \end{array} \right.",
        r"\begin{array}{r|l} 85 & 4 \\ \cline{2-2} - 8 & 21 \\ \hline 5 & \end{array}",
        r"\textcircled{\scriptsize{3}}25\%",
        r"\frac{3}{\cancel{\sqrt{2}}} \times \cancel{\sqrt{2}} = 3",
        r"n\,\text{A} \xrightarrow{\text{light},\;\Delta} \text{A}_n",
        r"\lim_{h \to 0} \frac{\frac{d}{dh}(e^h - 1)}{\frac{d}{dh}(h)}",
        r"\begin{aligned} q & = \frac{4,0}{2,0} = 2,0 \text{~units} \end{aligned}",
        r"y \equiv 2 \pmod{5}",
    ] {
        pt(tex);
    }
}
