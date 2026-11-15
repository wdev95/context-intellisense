//! One test per rule in spec/rules.md and spec/svg.md. The test name starts with the rule id.

use context_equation_core::tree::Variant;
use context_equation_core::{convert, Class, Locale, Node, Options};

fn pt(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { locale: Locale::PtBr, ..Default::default() }).unwrap()
}

fn en(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { locale: Locale::En, ..Default::default() }).unwrap()
}

fn number(value: &str) -> Node {
    Node::Number { value: value.into(), group: false, thousands: None, variant: Variant::Upright }
}

fn rules(r: &context_equation_core::Output) -> Vec<&'static str> {
    r.applied.iter().map(|a| a.rule).collect()
}

#[test]
fn l_num_1_comma_between_digits_is_decimal_in_pt_br() {
    let r = pt("3,14");
    assert_eq!(r.tree.children(), &[number("3.14")]);
    assert!(rules(&r).contains(&"L-NUM-1"));
}

#[test]
fn l_num_1_comma_followed_by_space_stays_separator() {
    let r = pt("f(1, 5)");
    let children = r.tree.children();
    assert_eq!(children[2], number("1"));
    assert_eq!(children[3], Node::Operator { symbol: ',', class: Class::Punct });
    assert_eq!(children[4], number("5"));
}

#[test]
fn l_num_1_in_english_comma_is_never_decimal() {
    let r = en("3,14");
    assert_eq!(r.tree.children()[0], number("3"));
    assert_eq!(r.tree.children()[2], number("14"));
}

#[test]
fn l_num_1_dot_is_decimal_in_both_locales() {
    assert_eq!(pt("3.14").tree.children(), &[number("3.14")]);
    assert_eq!(en("3.14").tree.children(), &[number("3.14")]);
}

#[test]
fn l_num_2_locale_decides_how_the_same_number_is_written() {
    assert_eq!(pt("3.14").text, "3,14");
    assert_eq!(en("3.14").text, "3.14");
    assert!(rules(&pt("3.14")).contains(&"L-NUM-2"));
}

#[test]
fn l_num_1_and_2_comma_in_braces_is_ordinary_as_in_tex() {
    // {,} is not a number, but it does not get punctuation spacing either
    let r = en("3{,}14");
    assert_eq!(r.tree.children()[1], Node::Operator { symbol: ',', class: Class::Ord });
    assert_eq!(r.text, "3,14");
}

#[test]
fn l_fun_1_accepts_brazilian_name_and_writes_by_locale() {
    assert_eq!(pt(r"\sen x").tree.children()[0], Node::Function { name: "sin".into(), limits: false });
    assert_eq!(pt(r"\sin x").text, "sen x");
    assert_eq!(en(r"\sen x").text, "sin x");
    assert_eq!(pt(r"\tg x").speech, "tangente de x");
}

#[test]
fn l_esp_2_leading_minus_is_unary() {
    let r = pt("-b + c");
    assert!(rules(&r).contains(&"L-ESP-2"));
    assert_eq!(r.text, "−b + c");
}

#[test]
fn l_esp_2_minus_after_relation_is_unary() {
    assert_eq!(pt("x = -1").text, "x = −1");
}

#[test]
fn l_esp_1_relation_has_more_space_than_operator() {
    // the width grows more with '=' (5 mu on both sides) than with '+' (4 mu)
    let width = |tex: &str| {
        let svg = pt(tex).svg;
        let start = svg.find("viewBox=\"0 ").unwrap() + 11;
        let fields: Vec<f32> = svg[start..].split('"').next().unwrap().split(' ').map(|v| v.parse().unwrap()).collect();
        fields[1]
    };
    let with_rel = width("a=b");
    let with_bin = width("a+b");
    // the glyphs have similar widths; a 2 mu difference = 1000/18*2 ≈ 111 units
    assert!(with_rel > with_bin, "rel {with_rel} should be wider than bin {with_bin}");
}

#[test]
fn l_let_1_letters_in_math_italic() {
    assert_eq!(context_equation_core::layout::math_italic('x'), '𝑥');
    assert_eq!(context_equation_core::layout::alphabet::character('R', context_equation_core::tree::Variant::DoubleStruck), 'ℝ');
    assert_eq!(context_equation_core::layout::math_italic('h'), 'ℎ');
    assert_eq!(context_equation_core::layout::math_italic('θ'), '𝜃');
    assert_eq!(context_equation_core::layout::math_italic('2'), '2');
}

#[test]
fn l_let_1_greek_variants_in_math_italic() {
    // \phi is ϕ, which TeX sets in italic like \varphi (#66)
    for (from, to) in [('ϕ', '𝜙'), ('φ', '𝜑'), ('ϵ', '𝜖'), ('ϑ', '𝜗'), ('ϰ', '𝜘'), ('ϱ', '𝜚'), ('ϖ', '𝜛')] {
        assert_eq!(context_equation_core::layout::math_italic(from), to, "{from}");
    }
    use context_equation_core::tree::Variant::{Bold, BoldItalic};
    assert_eq!(context_equation_core::layout::alphabet::character('ϕ', Bold), '𝛟');
    assert_eq!(context_equation_core::layout::alphabet::character('ϕ', BoldItalic), '𝝓');
    // drawn in italic, not as the upright ϕ of \mathrm
    let glyphs = |tex: &str| convert(tex, &Options::default()).unwrap().svg.split("<defs>").nth(1).map(str::to_string);
    assert_ne!(glyphs(r"\phi"), glyphs(r"\mathrm{\phi}"));
}

#[test]
fn s_svg_2_portuguese_speech_in_title() {
    let r = pt(r"x \le 2,5");
    assert_eq!(r.speech, "x é menor ou igual a 2,5");
    assert!(r.svg.contains(">x é menor ou igual a 2,5</title>"));
    assert!(r.svg.contains(r#"role="img""#));
}

#[test]
fn s_svg_3_source_recoverable_from_metadata() {
    let svg = pt(r"a < b").svg;
    assert!(svg.contains(r#"<contextEquation:source type="latex">a &lt; b</contextEquation:source>"#));
    assert!(svg.contains("<contextEquation:tree>"));
}

#[test]
fn s_svg_4_style_does_not_leak_into_the_page() {
    let svg = pt("x").svg;
    let style = svg.split("<style>").nth(1).unwrap().split("</style>").next().unwrap();
    assert_eq!(style, "svg:root{color:#111}");
}

#[test]
fn s_svg_5_repeated_glyph_is_drawn_once() {
    let svg = pt("1111").svg;
    assert_eq!(svg.matches("<path ").count(), 1);
    assert_eq!(svg.matches("<use ").count(), 4);
}

#[test]
fn s_svg_6_transparent_text_layer() {
    let svg = pt("3,14 + x").svg;
    assert!(svg.contains(r#"fill="transparent" aria-hidden="true">3,14 + x</text>"#));
}

#[test]
fn s_svg_7_different_ids_for_different_equations() {
    let id = |svg: &str| svg.split("aria-labelledby=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    assert_ne!(id(&pt("x").svg), id(&pt("y").svg));
    assert_ne!(id(&pt("x").svg), id(&en("x").svg));
}

#[test]
fn d_det_1_same_input_same_bytes() {
    let a = pt(r"f(1, 5) = \sen x - 2,5").svg;
    let b = pt(r"f(1, 5) = \sen x - 2,5").svg;
    assert_eq!(a, b);
}

#[test]
fn error_points_to_column() {
    let strict = Options { strict: true, ..Default::default() };
    let e = convert(r"x + \foo", &strict).unwrap_err();
    assert_eq!(e.to_string(), r"column 5: unsupported command: \foo");
    let e = convert("{x", &strict).unwrap_err();
    assert_eq!(e.to_string(), "column 1: '{' without a matching '}'");
}

#[test]
fn l_txt_1_tex_quotes_become_typographic_in_text() {
    let r = en(r"\text{``quoted'' and `single' don't}");
    assert_eq!(r.text, "“quoted” and ‘single’ don’t");
    assert!(rules(&r).contains(&"L-TXT-1"));
    // in math mode ' is still a prime
    let r = en("f'");
    assert!(!rules(&r).contains(&"L-TXT-1"));
    assert!(r.speech.contains("prime"), "{}", r.speech);
}

#[test]
fn l_txt_6_text_accents_on_a_letter() {
    // one character where Unicode has it, braces or not; i without its dot
    assert_eq!(en(r"\text{caf\'e, \~n, \c{c}, \v s, \'i}").text, "café, ñ, ç, š, í");
    // the letter and the combining accent where it has none
    assert_eq!(en(r"\text{\k{x}}").text, "x\u{0328}");
}

#[test]
fn l_txt_6_an_empty_argument_is_the_accent_alone() {
    // #306: the accent's spacing character, never an accent on the `}` that closes its argument
    for (mark, alone) in [
        (r"\'", "\u{00B4}"),
        (r"\`", "`"),
        (r"\^", "\u{02C6}"),
        (r"\~", "\u{02DC}"),
        ("\\\"", "\u{00A8}"),
        (r"\=", "\u{00AF}"),
        (r"\.", "\u{02D9}"),
        (r"\c", "\u{00B8}"),
        (r"\u", "\u{02D8}"),
        (r"\v", "\u{02C7}"),
        (r"\H", "\u{02DD}"),
        (r"\r", "\u{02DA}"),
        (r"\k", "\u{02DB}"),
        // no spacing character: the combining accent on a no-break space (a space in the linear text)
        (r"\b", " \u{0331}"),
        (r"\d", " \u{0323}"),
        (r"\t", " \u{0361}"),
    ] {
        let r = en(&format!(r"\text{{a{mark}{{}}b}}"));
        assert_eq!(r.text, format!("a{alone}b"), "{mark}{{}}");
        assert!(r.warnings.is_empty(), "{mark}{{}}: {:?}", r.warnings);
    }
    // a caret and a tilde as text is written with them; the group around keeps its brace
    assert_eq!(en(r"\text{x\^{}2 and \~{}user}").text, "x\u{02C6}2 and \u{02DC}user");
    assert_eq!(en(r"\text{{a\^{}}b}").text, "a\u{02C6}b");
    assert_eq!(en(r"\text{a\^ {} b}").text, "a\u{02C6} b");
}

#[test]
fn l_uni_4_siunitx_in_text() {
    // in text as in math, with no warning
    for (tex, math) in [
        (r"\text{\SI{12}{\volt}}", r"\SI{12}{\volt}"),
        (r"\text{\qty[mode=text]{12}{\volt}}", r"\qty{12}{\volt}"),
        (r"\text{\si{\ohm}}", r"\si{\ohm}"),
        (r"\text{\unit{\kilo\ohm}}", r"\unit{\kilo\ohm}"),
        (r"\text{\num{12345}}", r"\num{12345}"),
        (r"\text{\qtyrange{1}{5}{\volt}}", r"\qtyrange{1}{5}{\volt}"),
        (r"\text{\SIrange{1}{5}{\volt}}", r"\SIrange{1}{5}{\volt}"),
        (r"\text{\numrange{1}{5}}", r"\numrange{1}{5}"),
        (r"\text{\SIlist{1;2}{\volt}}", r"\SIlist{1;2}{\volt}"),
    ] {
        let (t, m) = (en(tex), en(math));
        assert!(t.warnings.is_empty(), "{tex}: {:?}", t.warnings);
        assert_eq!((t.text, t.speech), (m.text, m.speech), "{tex}");
    }
    let r = en(r"\text{from \SI{12}{\volt} to \qtyrange{1}{5}{\volt}}");
    assert_eq!(r.text, "from 12 V to 1 V to 5 V");
    assert_eq!(r.speech, "from 12 volts to 1 volt to 5 volts");
}

#[test]
fn l_uni_2_ranges_and_lists_repeat_the_unit() {
    // siunitx's range-phrase: the language's "to", or an en dash where the font cannot write it
    let r = en(r"\SIrange{1}{10}{\kilo\gram}");
    assert_eq!(r.text, "1 kg to 10 kg");
    assert!(rules(&r).contains(&"L-UNI-2"));
    let r = convert(r"\qtyrange{1}{5}{\volt}", &Options { locale: Locale::Ja, ..Options::default() }).unwrap();
    assert_eq!(r.text, "1 V – 5 V");
    let r = pt(r"\SIlist{1;2;3}{\meter}");
    assert_eq!(r.text, "1 m, 2 m e 3 m");
    assert!(rules(&r).contains(&"L-UNI-2"));
}

#[test]
fn l_che_1_labeled_arrow_in_chemistry() {
    let r = en(r"\ce{A ->[\Delta] B}");
    assert!(matches!(r.tree.children()[2], Node::Arrow { symbol: '→', above: Some(_), below: None }), "{:?}", r.tree);
    assert!(rules(&r).contains(&"L-CHE-1"));
}

#[cfg(feature = "chem")]
#[test]
fn l_che_2_structure_read_by_its_formula() {
    let r = en(r"\chemfig{H_3C-CH_2-OH}");
    assert!(rules(&r).contains(&"L-CHE-2"));
    assert_eq!(r.speech, "structure of C2 H6 O");
    assert_eq!(r.text, "C₂H₆O");
}

#[test]
fn l_ent_3_size_commands_and_packages_are_ignored() {
    for tex in [r"\Large x", r"\usepackage{amsmath} x", r"\usepackage[utf8]{inputenc} x", r"\require{mhchem} x"] {
        let r = en(tex);
        assert_eq!(r.text, "x", "{tex}");
        assert!(rules(&r).contains(&"L-ENT-3"), "{tex}");
        assert!(r.warnings.is_empty(), "{tex}: {:?}", r.warnings);
    }
}

#[test]
fn l_uni_3_units_are_spoken_by_name() {
    let r = en(r"\SI{9.81}{\meter\per\second\squared}");
    assert_eq!(r.speech, "9.81 meters per second squared");
    assert_eq!(r.text, "9.81 m/s^2");
    assert!(rules(&r).contains(&"L-UNI-3"));
    assert_eq!(pt(r"\SI{9,81}{\meter\per\second\squared}").speech, "9,81 metros por segundo ao quadrado");
}

#[test]
fn l_let_2_double_struck_letters_name_the_number_sets() {
    assert_eq!(en(r"x \in \mathbb{R}").speech, "x is in the real numbers");
    assert_eq!(pt(r"x \in \mathbb{R}").speech, "x pertence aos reais");
    assert_eq!(en(r"\mathbb{P}").speech, "double-struck P");
    // the drawing and the text keep the letter
    assert_eq!(en(r"\mathbb{R}").text, "ℝ");
}

#[test]
fn l_ent_4_mathml_pasted_as_latex_is_read_as_mathml() {
    let r = en("<math><msup><mi>x</mi><mn>2</mn></msup></math>");
    assert_eq!(r.text, "x^2");
    assert!(rules(&r).contains(&"L-ENT-4"));
}

#[test]
fn s_svg_9_braille_in_metadata() {
    let r = pt(r"\sqrt{x}");
    assert!(r.svg.contains(r#"<contextEquation:braille code="cmu">⠫⠱⠭</contextEquation:braille>"#), "{}", r.svg);
    assert!(en(r"\sqrt{x}").svg.contains(r#"<contextEquation:braille code="nemeth">⠜⠭⠻</contextEquation:braille>"#));
}

#[test]
fn l_opr_1_the_dot_between_vectors_is_the_dot_product() {
    assert_eq!(en(r"\vec{v} \cdot \vec{w}").speech, "vector v dot vector w");
    assert_eq!(pt(r"\mathbf{u} \cdot \mathbf{v}").speech, "u escalar v");
    assert_eq!(en(r"2 \cdot 3").speech, "2 times 3");
}

fn with(tex: &str, animate: bool, steps: bool) -> String {
    convert(tex, &Options { animate, steps, ..Default::default() }).unwrap().svg
}

/// The drawn elements of an SVG, in order: `use`, `rect`, `line`, `ellipse`.
fn drawn(svg: &str) -> Vec<String> {
    let body = &svg[svg.find("transform=\"scale(1,-1)\">").unwrap()..svg.find("</g><text").unwrap()];
    body.split('<').filter(|e| ["use ", "rect ", "line ", "ellipse "].iter().any(|t| e.starts_with(t))).map(|e| format!("<{e}")).collect()
}

const WRITTEN: &str = r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a} + \cancel{c} + \textcircled{1} + \begin{array}{c|c} 1 & 2 \\ \hline 3 & 4 \end{array}";

#[test]
fn s_svg_10_off_by_default() {
    let plain = en(WRITTEN).svg;
    assert_eq!(plain, with(WRITTEN, false, false));
    for mark in ["data-step", "mr-", "--i:", "@keyframes", "pathLength"] {
        assert!(!plain.contains(mark), "{mark} in an SVG without steps");
    }
}

#[test]
fn s_svg_10_steps_number_every_drawn_element_in_order() {
    let svg = with(WRITTEN, false, true);
    let elements = drawn(&svg);
    assert!(elements.len() > 20);
    for (k, e) in elements.iter().enumerate() {
        assert!(e.contains(&format!(r#" data-step="{k}""#)), "{k}: {e}");
    }
    assert!(svg.contains(&format!(r#" data-steps="{}""#, elements.len())));
    // no CSS: the page walks through it
    assert!(!svg.contains("@keyframes") && !svg.contains("mr-"));
    // and nothing else changes: without the step numbers, it is the plain SVG
    let unnumbered = svg.replace(&format!(r#" data-steps="{}""#, elements.len()), "");
    let unnumbered = (0..elements.len()).fold(unnumbered, |s, k| s.replacen(&format!(r#" data-step="{k}""#), "", 1));
    assert_eq!(unnumbered, en(WRITTEN).svg);
}

#[test]
fn s_svg_10_each_kind_enters_its_own_way() {
    let svg = with(WRITTEN, true, false);
    let elements = drawn(&svg);
    let kind = |tag: &str| elements.iter().filter(|e| e.starts_with(tag)).collect::<Vec<_>>();
    assert!(kind("<use ").iter().all(|e| e.contains(r#"class="mr-f""#)), "glyphs fade in");
    assert!(kind("<line ").iter().all(|e| e.contains(r#"class="mr-d" pathLength="1""#)), "lines are drawn");
    assert!(kind("<ellipse ").iter().all(|e| e.contains(r#"class="mr-d" pathLength="1""#)), "the circle is drawn");
    let rects = kind("<rect ");
    // the fraction bar, the radical's rule and the \hline grow from the left; the | from the top
    assert!(rects.iter().filter(|e| e.contains(r#"class="mr-w""#)).count() >= 3);
    assert!(rects.iter().any(|e| e.contains(r#"class="mr-v""#)));
    for (k, e) in elements.iter().enumerate() {
        assert!(e.contains(&format!(r#" data-step="{k}""#)) && e.contains(&format!(r#"style="--i:{k}""#)), "{e}");
    }
    assert!(svg.contains("@media (prefers-reduced-motion:reduce)"));
    assert_eq!(svg.matches("@keyframes mr-f").count(), 1);
}

#[test]
fn s_svg_10_about_three_seconds_and_the_same_rules_everywhere() {
    let interval = |tex: &str| -> f32 {
        let svg = with(tex, true, false);
        svg.split("--mr-step: ").nth(1).unwrap().split('s').next().unwrap().parse().unwrap()
    };
    let steps = |tex: &str| drawn(&with(tex, true, false)).len() as f32;
    // a short equation at the most 0.15 s apart; a long one within three seconds
    assert_eq!(interval("x+1"), 0.15);
    assert!(((steps(WRITTEN) - 1.0) * interval(WRITTEN) - 3.0).abs() < 0.2);
    // the <style> does not depend on the equation: two on one page do not retime each other
    let style = |tex: &str| {
        let svg = with(tex, true, false);
        svg[svg.find("<style>@keyframes").unwrap()..].split("</style>").next().unwrap().to_string()
    };
    assert_eq!(style("x+1"), style(WRITTEN));
}

#[test]
fn s_svg_10_still_valid_xml_and_still_accessible() {
    for tex in [WRITTEN, r"\ce{A <=>[K] B}", r"\phase{\alpha}", r"\reflectbox{abc}", "x"] {
        let svg = with(tex, true, true);
        roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert!(svg.contains("role=\"img\"") && svg.contains("<title"), "{tex}");
    }
}

#[test]
fn s_svg_11_html_attributes_on_the_content_group_and_nowhere_else() {
    for (tex, plain, attributes) in [
        (r"\class{highlight}{x^2} + 1", r"x^2 + 1", r#"<g class="highlight">"#),
        (r"\cssId{step-2}{a + b}", r"a + b", r#"<g id="step-2">"#),
        (r"\htmlData{step=3,kind=sum}{c}", r"c", r#"<g data-step="3" data-kind="sum">"#),
        (r"\htmlClass{term}{x} + y", r"x + y", r#"<g class="term">"#),
        (r"\style{opacity:0.5}{\frac{a}{b}}", r"\frac{a}{b}", r#"<g style="opacity:0.5">"#),
    ] {
        let with = convert(tex, &Options { strict: true, locale: Locale::En, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
        let without = en(plain);
        assert!(with.svg.contains(attributes), "{tex}: {}", with.svg);
        assert_eq!(with.speech, without.speech, "{tex}");
        assert_eq!(with.text, without.text, "{tex}");
        assert_eq!(with.braille, without.braille, "{tex}");
        // the group closes around the content's glyphs and nothing moves
        let body = |svg: &str| svg.split("<defs>").nth(1).unwrap().split("</defs>").nth(1).unwrap().replace(attributes, "").replace("<g>", "").replace("</g>", "").to_string();
        let ids = |svg: &str| svg.split("aria-labelledby=\"").nth(1).unwrap().split("-speech").next().unwrap().to_string();
        assert_eq!(body(&with.svg).replace(&ids(&with.svg), ""), body(&without.svg).replace(&ids(&without.svg), ""), "{tex}");
    }
    // a style's color colors the glyphs too, as \color does, so outputs without CSS have it
    let red = en(r"\style{color:red}{x}");
    assert!(red.svg.contains(r#"<g style="color:red"><use"#) && red.svg.contains(r#"fill="red""#), "{}", red.svg);
    assert_eq!(red.speech, "x");
}

// ---------- L-BRK-1, L-BRK-2 and S-SVG-12: automatic line breaking ----------

const POLYNOMIAL: &str = r"f(x) = a_0 + a_1 x + a_2 x^2 + a_3 x^3 + a_4 x^4 + a_5 x^5 + a_6 x^6 + a_7 x^7 + a_8 x^8 + a_9 x^9 + a_{10} x^{10}";

fn at(tex: &str, max_width: f32, display: bool) -> context_equation_core::Output {
    convert(tex, &Options { max_width: Some(max_width), display, ..Default::default() }).unwrap()
}

/// The drawing of each line of a broken SVG (S-SVG-12), in order.
fn lines(svg: &str) -> Vec<String> {
    svg.split(r#"<g data-line=""#).skip(1).map(|l| l.split("</g>").next().unwrap().to_string()).collect()
}

/// (x, y) of the glyphs a line draws without scaling, in order.
fn glyph_positions(line: &str) -> Vec<(f32, f32)> {
    line.split("<use ")
        .skip(1)
        .filter_map(|u| {
            let attr = |name: &str| u.split(&format!(r#" {name}=""#)).nth(1).map(|v| v.split('"').next().unwrap().parse::<f32>().unwrap());
            Some((attr("x")?, attr("y")?))
        })
        .collect()
}

fn notes(r: &context_equation_core::Output, rule: &str) -> Vec<String> {
    r.applied.iter().filter(|a| a.rule == rule).map(|a| a.description.clone()).collect()
}

#[test]
fn l_brk_1_without_the_option_or_when_it_fits_nothing_changes() {
    for tex in [POLYNOMIAL, WRITTEN, "x"] {
        let plain = en(tex);
        assert!(!plain.svg.contains("data-line"));
        let wide = at(tex, 1000.0, true);
        assert_eq!(wide.svg, plain.svg, "{tex}");
        assert_eq!(wide.applied, plain.applied);
    }
}

#[test]
fn l_brk_1_speech_text_braille_and_tree_do_not_change() {
    for (tex, width) in [(POLYNOMIAL, 12.0), (WRITTEN, 6.0)] {
        let (plain, broken) = (en(tex), at(tex, width, true));
        assert!(broken.svg.contains("data-line=\"1\""), "{tex}");
        assert_eq!((&plain.speech, &plain.text, &plain.braille, &plain.tree), (&broken.speech, &broken.text, &broken.braille, &broken.tree));
        // and inside the SVG too: the same title and metadata
        let head = |svg: &str| svg[svg.find("<title").unwrap()..svg.find("<defs>").unwrap()].to_string();
        assert_eq!(head(&plain.svg), head(&broken.svg));
        assert!(broken.warnings.is_empty());
    }
}

#[test]
fn l_brk_1_after_a_relation_first() {
    // the relations alone make it fit: no break after a +
    let r = at(r"a + b + c + d = e + f + g + h = i + j + k + l", 9.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 3 lines within 9em, after '=', '='"]);
    // an arrow is a relation too
    let r = at(r"a + b + c + d \to e + f + g + h", 8.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 2 lines within 8em, after '→'"]);
}

#[test]
fn l_brk_1_then_after_plus_and_minus_and_the_operator_is_not_repeated() {
    let r = at(POLYNOMIAL, 12.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 5 lines within 12em, after '+', '+', '+', '+'"]);
    let r = at(r"x = a - b - c - d - e - f - g - h", 5.0, true);
    let note = &notes(&r, "L-BRK-1")[0];
    assert!(note.contains("'−'") && !note.contains("'+'"), "{note}");
    // as many glyphs as without breaking: no operator repeated, in pt-BR either
    for locale in [Locale::En, Locale::PtBr] {
        let broken = convert(POLYNOMIAL, &Options { max_width: Some(12.0), locale, ..Default::default() }).unwrap().svg;
        let plain = convert(POLYNOMIAL, &Options { locale, ..Default::default() }).unwrap().svg;
        assert!(broken.contains("data-line=\"1\""));
        assert_eq!(broken.matches("<use ").count(), plain.matches("<use ").count());
    }
    // a unary minus, ·, × and a comma are never break points
    let r = at(r"x = -a \cdot b \times c , d", 1.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 2 lines within 1em, after '='"]);
}

#[test]
fn l_brk_1_never_inside_a_fraction_root_delimiters_scripts_or_a_group() {
    // every top-level break point taken (1 em): 5 points, 6 lines, and no more
    let tex = r"a = \frac{b+c}{d+e} + \sqrt{f+g} + \left(h+i\right) + x^{j+k} + {l+m}";
    let r = at(tex, 1.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 6 lines within 1em, after '=', '+', '+', '+', '+'"]);
    assert_eq!(lines(&r.svg).len(), 6);
    // what does not fit in its line is drawn as it is, and said so
    assert!(notes(&r, "L-BRK-2").iter().any(|n| n.starts_with("line 2 cannot be broken within 1em")), "{:?}", r.applied);
    // a single piece
    let r = at(r"\frac{a+b+c+d+e+f+g+h}{2}", 2.0, true);
    assert!(!r.svg.contains("data-line"));
    let note = &notes(&r, "L-BRK-2")[0];
    assert!(note.starts_with("the formula cannot be broken within 2em: drawn ") && note.ends_with("em wider"), "{note}");
}

#[test]
fn l_brk_1_continuation_under_the_right_hand_side() {
    let r = at(POLYNOMIAL, 12.0, true);
    let lines = lines(&r.svg);
    // f ( x ) = a: the sixth glyph of the first line starts the right-hand side
    let rhs = glyph_positions(&lines[0])[5].0;
    for line in &lines[1..] {
        assert!((glyph_positions(line)[0].0 - rhs).abs() < 0.01);
    }
}

#[test]
fn l_brk_1_fixed_indent_without_a_relation_or_with_one_far_in() {
    for tex in [r"a + b + c + d + e + f + g + h", r"aaaaaaaaaaaa = b + c + d + e + f + g"] {
        let r = at(tex, 8.0, true);
        let lines = lines(&r.svg);
        assert!(lines.len() >= 2, "{tex}");
        for line in &lines[1..] {
            assert_eq!(glyph_positions(line)[0].0, 2000.0, "{tex}");
        }
    }
}

#[test]
fn l_brk_1_line_gap_and_size() {
    // simple lines sit a strut apart (0.84 + 0.36 em, \baselineskip), plus \jot (0.3 em) in display
    for (display, gap) in [(true, -1500.0), (false, -1200.0)] {
        let r = at(r"a + b + c + d + e + f", 3.5, display);
        let lines = lines(&r.svg);
        assert_eq!(glyph_positions(&lines[0])[0].1, 0.0);
        assert_eq!(glyph_positions(&lines[1])[0].1, gap);
    }
    // S-SVG-1: the baseline is the first line's; the depth covers the lines below
    let svg = at(r"a + b + c + d + e + f", 3.5, true).svg;
    let lowered: f32 = svg.split("vertical-align: ").nth(1).unwrap().split("em").next().unwrap().parse().unwrap();
    assert!(lowered < -2.9, "{lowered}");
}

#[test]
fn l_brk_2_tables_are_never_broken() {
    for tex in [
        r"\begin{pmatrix} a + b + c + d + e + f & g + h + i + j \end{pmatrix}",
        r"\begin{aligned} a &= b + c + d + e + f + g + h \\ i &= j + k + l + m + n \end{aligned}",
        r"x + \begin{align} a &= b + c + d + e + f + g + h \\ i &= j \end{align} + y + z",
    ] {
        let r = at(tex, 3.0, true);
        assert!(!r.svg.contains("data-line"), "{tex}");
        assert!(notes(&r, "L-BRK-2")[0].starts_with("the table cannot be broken within 3em"), "{tex}: {:?}", r.applied);
        assert!(r.warnings.is_empty(), "a layout fact, not an input problem");
    }
    // a matrix among other atoms is one of them: the line breaks around it
    let r = at(r"A = \begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix} + \begin{pmatrix} 5 & 6 \\ 7 & 8 \end{pmatrix}", 6.0, true);
    assert_eq!(notes(&r, "L-BRK-1"), ["broken into 2 lines within 6em, after '+'"]);
}

#[test]
fn s_svg_12_one_group_per_line() {
    for display in [true, false] {
        let r = at(POLYNOMIAL, 20.0, display);
        let n = lines(&r.svg).len();
        assert_eq!(n, 3);
        assert!(r.svg.contains(r#" data-lines="3""#));
        for k in 0..n {
            assert_eq!(r.svg.matches(&format!(r#"<g data-line="{k}">"#)).count(), 1);
        }
        roxmltree::Document::parse(&r.svg).unwrap();
    }
    // steps still count the whole equation, in order, across the lines
    let svg = convert(POLYNOMIAL, &Options { max_width: Some(12.0), steps: true, ..Default::default() }).unwrap().svg;
    let numbers: Vec<usize> = svg.split(r#" data-step=""#).skip(1).map(|s| s.split('"').next().unwrap().parse().unwrap()).collect();
    assert_eq!(numbers, (0..numbers.len()).collect::<Vec<_>>());
}

#[test]
fn l_brk_1_asciimath_breaks_too() {
    let r = convert("a+b+c+d+e+f+g+h", &Options { input: context_equation_core::Input::AsciiMath, max_width: Some(3.0), ..Default::default() }).unwrap();
    assert!(lines(&r.svg).len() >= 2);
}
