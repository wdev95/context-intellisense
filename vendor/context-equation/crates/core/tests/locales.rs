//! The languages beyond en and pt-BR: every one is complete, converts the whole corpus, and reads and
//! writes the conventions of its language.

use context_equation_core::{convert, Locale, Options};

fn convert_in(tex: &str, locale: Locale) -> context_equation_core::Output {
    convert(tex, &Options { locale, ..Options::default() }).unwrap_or_else(|e| panic!("{locale} {tex}: {e}"))
}

fn speech(tex: &str, locale: Locale) -> String {
    convert_in(tex, locale).speech
}

fn text(tex: &str, locale: Locale) -> String {
    convert_in(tex, locale).text
}

fn corpus() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    ["constructions.json", "packages.json"]
        .iter()
        .flat_map(|f| {
            let json = std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
            serde_json::from_str::<Vec<(String, String)>>(&json).unwrap().into_iter().map(|(_, tex)| tex)
        })
        .collect()
}

#[test]
fn every_language_has_every_phrase() {
    for locale in Locale::ALL {
        assert!(locale.missing_phrases().is_empty(), "{locale} lacks {:?}", locale.missing_phrases());
    }
}

#[test]
fn every_language_converts_the_corpus() {
    let corpus = corpus();
    for locale in Locale::ALL {
        for tex in &corpus {
            let r = convert_in(tex, locale);
            assert!(r.svg.contains(&format!(r#"lang="{}""#, locale.code())), "{locale} {tex}: SVG without lang");
            assert!(!r.speech.contains('{'), "{locale} {tex}: unfilled template in «{}»", r.speech);
        }
    }
}

#[test]
fn codes_with_regions_and_any_case() {
    for (code, locale) in [("es-MX", Locale::Es), ("zh_CN", Locale::Zh), ("PT", Locale::PtBr), ("fr-CA", Locale::Fr), ("en-GB", Locale::En), ("ja", Locale::Ja)] {
        assert_eq!(code.parse::<Locale>(), Ok(locale), "{code}");
    }
    let err = "xx".parse::<Locale>().unwrap_err();
    assert!(err.contains("unknown locale") && err.contains("pt-BR") && err.contains("zh"), "{err}");
}

/// (locale, input, expected snippet of the speech)
const SPEECHES: &[(Locale, &str, &str)] = &[
    (Locale::Es, r"\frac{3}{4}", "tres cuartos"),
    (Locale::Es, r"\sin x", "seno de x"),
    (Locale::Es, r"\lim_{x \to 0} f", "límite cuando x tiende a 0 de"),
    (Locale::Fr, r"\frac{1}{2}", "un demi"),
    (Locale::Fr, r"\binom{n}{k}", "k parmi n"),
    (Locale::Fr, r"]0, 1[", "ouvre l'intervalle 0 virgule 1 ferme l'intervalle"),
    (Locale::De, r"x^2", "x Quadrat"),
    (Locale::De, r"\lim_{x \to 0} f", "Limes für x gegen 0 von"),
    (Locale::It, r"\frac{a+b}{c}", "fratto c"),
    (Locale::It, r"\sin^{-1} x", "arco seno di x"),
    (Locale::Ru, r"\sin^{-1} x", "арксинус от x"),
    (Locale::Ru, r"\sqrt{x}", "квадратный корень из x"),
    (Locale::Zh, r"\frac{1}{2}", "2 分之 1"),
    (Locale::Zh, r"\sin^{-1} x", "反正弦 x"),
    (Locale::Ja, r"\sqrt{x}", "ルート x"),
    (Locale::Ja, r"\frac{1}{3}", "3 分の 1"),
    (Locale::Ko, r"\frac{1}{3}", "3 분의 1"),
    (Locale::Hi, r"\frac{a}{b}", "a बटा b"),
    (Locale::Ar, r"x = 1", "x يساوي 1"),
    (Locale::Id, r"\frac{1}{2}", "1 per 2"),
    (Locale::Tr, r"\frac{1}{2}", "1 bölü 2"),
    (Locale::Nl, r"\binom{n}{k}", "n boven k"),
    (Locale::Pl, r"\binom{n}{k}", "n po k"),
    (Locale::Pl, r"x^2", "x do kwadratu"),
];

#[test]
fn speech_in_each_language() {
    let failures: Vec<String> = SPEECHES
        .iter()
        .filter_map(|(locale, tex, expected)| {
            let s = speech(tex, *locale);
            (!s.contains(expected)).then(|| format!("{locale} {tex}: «{s}» lacks «{expected}»"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// L-NUM-1, L-NUM-2, L-NUM-3 and L-FUN-1 beyond pt-BR: (locale, input, linear text)
const TEXTS: &[(Locale, &str, &str)] = &[
    (Locale::De, "3,14", "3,14"),
    (Locale::De, "1.000,5", "1.000,5"),
    (Locale::Fr, "3.14", "3,14"),
    (Locale::Ru, r"\tan x", "tg x"),
    (Locale::Fr, r"\sinh x", "sh x"),
    (Locale::Es, r"\sin x", "sen x"),
    (Locale::De, r"\gcd(a, b)", "ggT"),
    (Locale::Zh, "3.14", "3.14"),
    (Locale::Ja, "f(1,5)", "f(1, 5)"),
];

#[test]
fn numbers_and_function_names_in_each_language() {
    for (locale, tex, expected) in TEXTS {
        let t = text(tex, *locale);
        assert!(t.contains(expected), "{locale} {tex}: «{t}» lacks «{expected}»");
    }
}

/// The constructions added for chemistry, physics, siunitx and amssymb, in every language: no unfilled
/// template, no warning, and the words of ranges, lists and boxes come from the language.
#[test]
fn every_language_converts_the_science_constructions() {
    let constructions = [
        r"a \nleq b \ngtr c \nsim d \nRightarrow e",
        r"\ce{A ->[\Delta][$-H_2O$] B <=>[K_w] C}",
        r"\ce{CuSO4.5H2O + HC#CH + ^{14}_{6}C + 1/2 O2}",
        r"\pu{123 kJ mol^-1}",
        r"\expval{A}{\psi} + \mel{n}{H}{m} + \comm{A}{B} + \qty(\frac{a}{b})",
        r"\grad f + \curl \vb{B} + \laplacian \phi + \vu{x} + \order{n^2}",
        r"\Tr\rho + \int f \dd x + \eval{x^2}_0^1 + \fdv{F}{\phi}",
        r"\SIrange{1}{10}{\kilo\gram} \quad \numlist{1;2;3} \quad \ang{12;30;15} \quad \SI{1}{\angstrom}",
        r"\tensor{T}{^\mu_\nu} = \prescript{14}{6}{\mathrm{C}}",
        r"\begin{rcases} a \\ b \end{rcases} \overbracket{x+y}^{n} \underrightarrow{AB}",
        r"\colorbox{yellow}{x} \fcolorbox{red}{yellow}{y} \textsc{Smith} \boxed{z}",
    ];
    for locale in Locale::ALL {
        for tex in constructions {
            let r = convert_in(tex, locale);
            assert!(r.warnings.is_empty(), "{locale} {tex}: {:?}", r.warnings);
            assert!(!r.speech.contains('{'), "{locale} {tex}: unfilled template in «{}»", r.speech);
        }
        let english = |tex: &str| convert_in(tex, Locale::En).speech;
        if locale != Locale::En {
            for tex in [r"\boxed{z}", r"\numrange{1}{2}", r"\numlist{1;2}"] {
                assert_ne!(speech(tex, locale), english(tex), "{locale} {tex}: still in English");
            }
        }
    }
}

#[test]
fn list_word_falls_back_to_a_comma_where_the_font_lacks_the_script() {
    for (locale, expected) in [
        (Locale::Ru, "1, 2 и 3"),
        (Locale::Tr, "1, 2 ve 3"),
        (Locale::Zh, "1, 2, 3"),
        (Locale::Ja, "1, 2, 3"),
        (Locale::Ko, "1, 2, 3"),
        (Locale::Hi, "1, 2, 3"),
        (Locale::Ar, "1, 2, 3"),
    ] {
        assert_eq!(text(r"\numlist{1;2;3}", locale), expected, "{locale}");
    }
}

#[test]
fn every_language_reads_mathml_and_asciimath() {
    use context_equation_core::{convert, Input, Options};
    for locale in Locale::ALL {
        for (source, input) in [
            ("<math><mfrac><mn>1</mn><mn>2</mn></mfrac><mo>+</mo><msqrt><mi>x</mi></msqrt></math>", Input::MathMl),
            ("1/2 + sqrt(x) + sum_(i=1)^n i + [[a,b],[c,d]]", Input::AsciiMath),
        ] {
            let r = convert(source, &Options { locale, input, ..Options::default() }).unwrap_or_else(|e| panic!("{locale} {source}: {e}"));
            assert!(r.warnings.is_empty(), "{locale} {source}: {:?}", r.warnings);
            // the same words as the LaTeX for the shared beginning
            let shared = convert_in(r"\frac{1}{2} + \sqrt{x}", locale).speech;
            assert!(r.speech.starts_with(&shared), "{locale} {source}: «{}» does not start with «{shared}»", r.speech);
        }
    }
}
