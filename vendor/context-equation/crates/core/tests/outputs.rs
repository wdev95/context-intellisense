//! Speech (screen readers) and linear text: what comes out for people who do not see the drawing.

use context_equation_core::{convert, Locale, Options};

fn options(loc: Locale) -> Options {
    Options { locale: loc, ..Options::default() }
}

fn speech(tex: &str, loc: Locale) -> String {
    convert(tex, &options(loc)).unwrap_or_else(|e| panic!("{tex}: {e}")).speech
}

fn text(tex: &str, loc: Locale) -> String {
    convert(tex, &options(loc)).unwrap_or_else(|e| panic!("{tex}: {e}")).text
}

/// (input, pt-BR snippet, English snippet)
const SPEECHES: &[(&str, &str, &str)] = &[
    (r"\sqrt{x}", "raiz quadrada de", "square root of"),
    (r"\sqrt[2]{x}", "raiz quadrada de", "square root of"),
    (r"\sqrt[3]{x}", "raiz cúbica de", "cube root of"),
    (r"\sqrt[n]{x}", "raiz de índice n de", "root of index n of"),
    (r"\vec{v}", "vetor v", "vector v"),
    (r"\hat{x}", "x chapéu", "x hat"),
    (r"\dot{x}", "x ponto", "x dot"),
    (r"\ddot{x}", "x dois pontos", "x double dot"),
    (r"\tilde{x}", "x til", "x tilde"),
    (r"\overarc{AB}", "arco", "arc"),
    (r"\breve{x}", "x", "x"),
    (r"\overline{AB}", "barra", "bar"),
    (r"\bar{x}", "x barra", "x bar"),
    (r"\max_{x} f", "índice x", "sub x"),
    (r"\underline{x}", "x", "x"),
    (r"\underbrace{a+b}_{n}", "(n)", "(n)"),
    (r"\text{if $x$ is even}", "if", "if"),
    (r"\xrightarrow{a}", "seta para a direita", "right arrow"),
    (r"\xleftarrow{a}", "seta para a esquerda", "left arrow"),
    (r"\xleftrightarrow{a}", "seta dupla", "double arrow"),
    (r"\xRightarrow{a}", "implica", "implies"),
    (r"\xrightarrow[b]{a}", "(b)", "(b)"),
    (r"\xLeftarrow{a}", "a", "a"),
    (r"\overset{\text{def}}{=}", "com def em cima", "with def above"),
    (r"\underset{n}{\max}", "com n embaixo", "with n below"),
    (r"\begin{CD} A @>>> B \\ @VVV @AAA \\ C @= D \end{CD}", "seta para baixo", "down arrow"),
    (r"\begin{CD} A @AAA B \end{CD}", "seta para cima", "up arrow"),
    (r"\begin{CD} A @| B \end{CD}", "igual", "equals"),
    (r"\sum_{i=1}^{n} i", "de", "from"),
    (r"\sum_{i} i", "sobre", "over"),
    (r"\int^{1} f", "até 1 de", "to 1 of"),
    (r"\int f", "integral", "integral"),
    (r"\lim_{x \to 0} f", "limite com x tendendo a 0", "limit as x"),
    (r"\sin^2 x", "seno ao quadrado de x", "sine squared of x"),
    (r"\log_2 x", "logaritmo na base 2 de x", "log base 2 of x"),
    (r"\sin^{-1} y", "arco seno de y", "inverse sine of y"),
    (r"\log^{-1} y", "inversa de logaritmo de y", "inverse log of y"),
    (r"\sec^2 x", "secante ao quadrado de x", "secant squared of x"),
    (r"\cot x", "cotangente de x", "cotangent of x"),
    (r"|x|", "módulo de x", "absolute value of x"),
    (r"\lvert x \rvert", "módulo de x", "absolute value of x"),
    (r"|a + b|", "módulo de, a mais b, fim do módulo", "absolute value of, a plus b, end absolute value"),
    (r"\|v\|", "norma de v", "norm of v"),
    (r"\left\| u + v \right\|", "norma de, u mais v, fim da norma", "norm of, u plus v, end norm"),
    (r"|a| + |b|", "módulo de a mais módulo de b", "absolute value of a plus absolute value of b"),
    (r"a | b", "a barra b", "a bar b"),
    (r"[a]", "fecha colchetes", "close bracket"),
    (r"\csc x", "cossecante de x", "cosecant of x"),
    (r"\sinh^2 x", "senh ao quadrado", "sinh squared x"),
    (r"\lim_{x \to 0}^{+} f", "mais", "plus"),
    (r"x_{i}", "índice i", "sub i"),
    (r"x^{2}", "ao quadrado", "squared"),
    (r"x^{3}", "ao cubo", "cubed"),
    (r"f^{\prime}", "linha", "prime"),
    (r"f''", "duas linhas", "double prime"),
    (r"30^{\circ}", "graus", "degrees"),
    (r"a^{+}", "pela direita", "from the right"),
    (r"a^{-}", "pela esquerda", "from the left"),
    (r"\begin{vmatrix} a & b \\ c & d \end{vmatrix}", "determinante", "determinant"),
    (r"\begin{cases} 1 & x>0 \\ 0 & x\le 0 \end{cases}", "casos", "cases"),
    (r"\frac{1}{2}", "um meio", "over"),
    (r"\frac{3}{4}", "três quartos", "over"),
    (r"\frac{11}{2}", "11", "11"),
    (r"\iint f", "integral dupla", "double integral"),
    (r"\iiint f", "integral tripla", "triple integral"),
    (r"\oint f", "integral de linha", "contour integral"),
    (r"\bigcup_i A", "união", "union"),
    (r"\bigcap_i A", "interseção", "intersection"),
    (r"\coprod_i A", "operador", "operator"),
    (r"a \mp b", "menos ou mais", "minus or plus"),
    (r"a \neq b", "é diferente de", "is not equal to"),
    (r"a \propto b", "é proporcional a", "is proportional to"),
    (r"p \Rightarrow q", "implica", "implies"),
    (r"p \Leftrightarrow q", "se e somente se", "if and only if"),
    (r"A \rightleftharpoons B", "em equilíbrio com", "in equilibrium with"),
    (r"x \notin A", "não pertence a", "is not in"),
    (r"A \subset B", "está contido em", "is a subset of"),
    (r"A \subseteq B", "está contido ou é igual a", "is a subset of or equal to"),
    (r"A \cup B", "união", "union"),
    (r"A \cap B", "interseção", "intersection"),
    (r"\nabla f", "nabla", "nabla"),
    (r"\forall x", "para todo", "for all"),
    (r"\exists x", "existe", "there exists"),
    (r"\emptyset", "conjunto vazio", "empty set"),
    (r"\{ a \}", "fecha chaves", "close brace"),
    (r"a ; b", "ponto e vírgula", "semicolon"),
    (r"a : b", "dois-pontos", "colon"),
    (r"n!", "fatorial", "factorial"),
    (r"30°", "graus", "degrees"),
    (r"1, \ldots, n", "e assim por diante", "and so on"),
    (r"\triangle ABC", "triângulo", "triangle"),
    (r"r \perp s", "é perpendicular a", "is perpendicular to"),
    (r"r \parallel s", "é paralelo a", "is parallel to"),
    (r"\angle A", "ângulo", "angle"),
    (r"5\%", "por cento", "percent"),
    (r"a \approx b", "é aproximadamente", "is approximately"),
    (r"a \equiv b", "é côngruo a", "is congruent to"),
    (r"x \to 0", "tende a", "tends to"),
    (r"\cancel{x}", "x cancelado", "x canceled"),
    (r"i\slashed{\partial}\psi", "derivada parcial cortado", "partial slash psi"),
    (r"\slashed{D}", "D cortado", "D slash"),
    (r"\circled{1}", "1 circulado", "circled 1"),
    (r"\vv{AB}", "vetor A B", "vector A B"),
    (r"\overlinesegment{AB}", "segmento A B", "segment A B"),
    (r"\widebar{AB}", "A B barra", "A B bar"),
    (r"\boxed{x}", "x", "x"),
];

#[test]
fn speech_in_portuguese_and_english() {
    let mut failures = Vec::new();
    for (tex, pt, en) in SPEECHES {
        let f = speech(tex, Locale::PtBr);
        if !f.contains(pt) {
            failures.push(format!("pt {tex}: «{f}» lacks «{pt}»"));
        }
        let f = speech(tex, Locale::En);
        if !f.contains(en) {
            failures.push(format!("en {tex}: «{f}» lacks «{en}»"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_feynman_slash_is_not_a_cancellation_in_any_language() {
    for locale in Locale::ALL {
        let slashed = speech(r"\slashed{D}", locale);
        assert!(slashed.contains('D') && slashed != speech(r"\cancel{D}", locale), "{locale}: «{slashed}»");
    }
}

/// (input, expected snippet of the linear text)
const TEXTS: &[(&str, &str)] = &[
    (r"\sqrt[3]{x}", "∛"),
    (r"\sqrt[4]{x}", "∜"),
    // the combining long solidus overlay after the letter
    (r"\slashed{D}", "D\u{0338}"),
    (r"\sqrt[n]{x}", "√[n]"),
    (r"f'^{2}", "′^"),
    (r"f''", "″"),
    (r"\tan^{-1} x", "tg^(−1) x"),
    (r"\log_2 x", "log_2 x"),
    (r"\sin^2 x + 1", "sen^2 x + 1"),
    (r"\lim_{x \to 0} f(x)", "lim_(x → 0) f(x)"),
    (r"\begin{pmatrix} a & b \end{pmatrix}", "([a, b])"),
    (r"\begin{vmatrix} a & b \end{vmatrix}", "|[a, b]|"),
    (r"\begin{bmatrix} a & b \end{bmatrix}", "[[a, b]]"),
    (r"\vec{v}", "v⃗"),
    (r"\overline{AB}", "‾"),
    (r"\underline{x}", "x"),
    (r"\underbrace{a+b}_{n}", " [n]"),
    (r"\text{if $x$}", "if x"),
    (r"a\quad b", "a b"),
    (r"a\,b", "a"),
    (r"\phantom{x}y", "y"),
    (r"\circled{1}", "(1)"),
    (r"\boxed{x}", "x"),
    (r"\xrightarrow{a}", "→[a]"),
    (r"\overset{a}{=}", "="),
    (r"\begin{CD} A @VVV B \end{CD}", "↓"),
    (r"\divchave{17}{5}", "resto"),
];

#[test]
fn linear_text() {
    let mut failures = Vec::new();
    for (tex, snippet) in TEXTS {
        let t = text(tex, Locale::PtBr);
        if !t.contains(snippet) {
            failures.push(format!("{tex}: «{t}» lacks «{snippet}»"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(text(r"\divchave{17}{5}", Locale::En).contains("remainder"));
}

#[test]
fn public_shortcuts() {
    assert!(context_equation_core::tex_to_svg("x").unwrap().starts_with("<svg"));
    let (math_box, warnings) = context_equation_core::typeset(r"\frac{a}{b}", &Options::default()).unwrap();
    assert!(math_box.width > 0.0 && warnings.is_empty());
    let (_, warnings) = context_equation_core::typeset(r"\madeupcommand", &Options::default()).unwrap();
    assert!(!warnings.is_empty());
    assert!(context_equation_core::typeset(r"\frac{a}", &Options { strict: true, ..Options::default() }).is_err());
}

#[test]
fn errors_have_readable_message() {
    let e = context_equation_core::Error::MissingGlyph { character: '\u{10FFFF}' };
    assert!(e.to_string().contains("U+10FFFF"), "{e}");
    let e = convert(r"\frac{a}", &Options { strict: true, ..Options::default() }).unwrap_err();
    assert!(e.to_string().starts_with("column "), "{e}");
    let _: &dyn std::error::Error = &e;
}

#[test]
fn locale_from_text() {
    for s in ["pt-BR", "pt", "PT_BR"] {
        assert_eq!(s.parse::<Locale>().unwrap(), Locale::PtBr);
    }
    for s in ["en", "en-US", "en_us"] {
        assert_eq!(s.parse::<Locale>().unwrap(), Locale::En);
    }
    assert!("xx".parse::<Locale>().unwrap_err().contains("xx"));
    assert_eq!(Locale::En.to_string(), "en");
}

#[test]
fn spelled_out_functions_in_english() {
    for (tex, snippet) in [(r"\sin x", "sine of"), (r"\cos x", "cosine of"), (r"\tan x", "tangent of"), (r"\log x", "log of"), (r"\ln x", "natural log of"), (r"\sec x", "sec")] {
        let f = speech(tex, Locale::En);
        assert!(f.contains(snippet), "{tex}: {f}");
    }
}

#[test]
fn hand_built_tree_with_impossible_division() {
    // the reader refuses \divchave without integers, but the tree is public and may come from elsewhere
    let node = context_equation_core::Node::LongDivision { dividend: "a".into(), divisor: "0".into() };
    assert!(context_equation_core::output::speech::speak(&node, Locale::PtBr).contains('?'));
    assert!(context_equation_core::output::text::linear(&node, Locale::PtBr).contains("resto"));
}

#[test]
fn hand_built_large_operator_without_limits() {
    let base = context_equation_core::Node::LargeOperator { symbol: '∑', limits: None };
    let node = context_equation_core::Node::Scripts { base: Box::new(base), sup: None, sub: None };
    assert_eq!(context_equation_core::output::speech::speak(&node, Locale::PtBr), "somatório");
    assert!(node.children().is_empty());
}

#[test]
fn greek_letters_by_locale() {
    for (tex, pt, en) in [
        (r"\gamma", "gama", "gamma"),
        (r"\varepsilon", "épsilon", "epsilon"),
        (r"\xi", "csi", "xi"),
        (r"\rho", "rô", "rho"),
        (r"\varphi", "fi", "phi"),
        (r"\chi", "qui", "chi"),
        (r"\omega", "ômega", "omega"),
        (r"\Gamma", "gama maiúsculo", "capital gamma"),
        (r"\Theta", "teta maiúsculo", "capital theta"),
        (r"\Lambda", "lambda maiúsculo", "capital lambda"),
        (r"\Pi", "pi maiúsculo", "capital pi"),
        (r"\Sigma", "sigma maiúsculo", "capital sigma"),
        (r"\Phi", "fi maiúsculo", "capital phi"),
        (r"\Psi", "psi maiúsculo", "capital psi"),
    ] {
        assert_eq!(speech(tex, Locale::PtBr), pt, "{tex}");
        assert_eq!(speech(tex, Locale::En), en, "{tex}");
    }
}

#[test]
fn framed_boxes_ranges_and_lists_are_spoken_in_each_language() {
    for (loc, boxed, range, list) in [
        (Locale::En, "boxed x", "1 meter to 2 meters", "and"),
        (Locale::PtBr, "x em uma caixa", "1 metro a 2 metros", "e"),
        (Locale::Es, "x en un recuadro", "1 m a 2 m", "y"),
        (Locale::Fr, "x encadré", "1 m à 2 m", "et"),
        (Locale::De, "x eingerahmt", "1 m bis 2 m", "und"),
    ] {
        assert_eq!(speech(r"\boxed{x}", loc), boxed, "{loc}");
        assert_eq!(speech(r"\fbox{x}", loc), boxed, "{loc}");
        assert_eq!(speech(r"\SIrange{1}{2}{\meter}", loc), range, "{loc}");
        assert!(speech(r"\numlist{1;2;3}", loc).ends_with(&format!("2 {list} 3")), "{loc}: {}", speech(r"\numlist{1;2;3}", loc));
    }
    // a colored background is emphasis, like a color: only the content is read
    assert_eq!(speech(r"\colorbox{yellow}{x}", Locale::En), "x");
    assert_eq!(speech(r"\fcolorbox{red}{yellow}{x}", Locale::En), "boxed x");
    // the circle keeps its own phrase
    assert_eq!(speech(r"\textcircled{1}", Locale::En), "circled 1");
}

#[test]
fn physics_nabla_angles_and_integrals() {
    assert_eq!(speech(r"\grad f", Locale::PtBr), speech(r"\nabla f", Locale::PtBr));
    assert_eq!(speech(r"\ang{1;2;3}", Locale::En), "1 degrees 2 prime 3 double prime");
    assert_eq!(speech(r"\iiiint f", Locale::En), "integral f");
    assert_eq!(text(r"\overbracket{a}^{n}", Locale::En), "a [n]");
}

#[test]
fn binomials_say_their_parentheses_once() {
    // the parentheses are part of the binomial: "n choose k" and (n ¦ k), with no second pair
    for tex in [r"\binom{n}{k}", r"\dbinom{n}{k}", r"\tbinom{n}{k}", r"{n \choose k}", r"\left( n \atop k \right)"] {
        assert_eq!(speech(tex, Locale::En), "n choose k", "{tex}");
        assert_eq!(speech(tex, Locale::PtBr), "n escolhe k", "{tex}");
        assert_eq!(text(tex, Locale::En), "(n ¦ k)", "{tex}");
    }
    let mathml = r#"<math><mrow><mo>(</mo><mfrac linethickness="0"><mi>n</mi><mi>k</mi></mfrac><mo>)</mo></mrow></math>"#;
    assert_eq!(speech(mathml, Locale::En), "n choose k");
    // other brackets around a fraction without a rule keep their own words and symbols
    assert_eq!(text(r"{n \brack k}", Locale::En), "[n ¦ k]");
    assert!(speech(r"{n \brack k}", Locale::En).starts_with("open bracket"));
}

#[test]
fn large_operators_are_spaced_from_their_operand() {
    // like sin^2 x: the limits stick to the operator, and a space separates it from what it applies to
    for (tex, expected) in [
        (r"\int_0^1 x^2\,dx", "∫_0^1 x^2 dx"),
        (r"\sum_{i=1}^n i", "∑_(i = 1)^n i"),
        (r"\iint_D f\,dA", "∬_D f dA"),
        (r"\oint_C F", "∮_C F"),
        (r"\prod_{k} a_k", "∏_k a_k"),
        (r"\bigcup_{i} A_i", "⋃_i A_i"),
        (r"\int f\,dx", "∫ f dx"),
    ] {
        assert_eq!(text(tex, Locale::En), expected, "{tex}");
    }
}

#[test]
fn units_are_spoken_by_name() {
    for (tex, english, portuguese) in [
        (r"\qty{5}{\kilo\gram}", "5 kilograms", "5 quilogramas"),
        (r"\SI{1}{\meter}", "1 meter", "1 metro"),
        (r"\SI{1.5}{\meter}", "1.5 meters", "1,5 metro"),
        (r"\SI{2}{\kilo\meter}", "2 kilometers", "2 quilômetros"),
        (r"\SI{3}{\milli\second}", "3 milliseconds", "3 milissegundos"),
        (r"\si{\meter\per\second}", "meters per second", "metros por segundo"),
        (r"\SI{8.314}{\joule\per\mole\per\kelvin}", "8.314 joules per mole kelvin", "8,314 joules por mol kelvin"),
        (r"\SI{1}{\per\second}", "1 per second", "1 por segundo"),
        (r"\SI{20}{\degreeCelsius}", "20 degrees Celsius", "20 graus Celsius"),
        (r"\SI{2}{\metre}", "2 meters", "2 metros"),
        (r"\SIrange{1}{10}{\kilo\gram}", "1 kilogram to 10 kilograms", "1 quilograma a 10 quilogramas"),
    ] {
        assert_eq!(speech(tex, Locale::En), english, "{tex}");
        assert_eq!(speech(tex, Locale::PtBr), portuguese, "{tex}");
    }
    // a unit the reader does not know keeps the name it was written with
    assert_eq!(speech(r"\SI{2}{\foo}", Locale::En), "2 foo");
    // a language without unit names reads the symbols, as before
    assert_eq!(speech(r"\qty{5}{\kilo\gram}", Locale::Fr), "5 kg");
    // units written as letters, not as siunitx commands, are not named
    assert_eq!(speech(r"\si{km}", Locale::En), speech(r"\mathrm{km}", Locale::En));
}

#[test]
fn double_struck_letters_are_read_as_number_sets() {
    for (tex, english, portuguese) in [
        (r"n \in \mathbb{N}", "n is in the natural numbers", "n pertence aos naturais"),
        (r"k \notin \mathbb{Z}", "k is not in the integers", "k não pertence aos inteiros"),
        (r"\mathbb{Q} \subset \mathbb{R}", "the rational numbers is a subset of the real numbers", "os racionais está contido nos reais"),
        (r"z \in \mathbb{C}", "z is in the complex numbers", "z pertence aos complexos"),
        (r"\mathbb{H}", "double-struck H", "H duplo"),
        ("x ∈ ℝ", "x is in the real numbers", "x pertence aos reais"),
        ("x ∈ 𝔸", "x is in double-struck A", "x pertence a A duplo"),
    ] {
        assert_eq!(speech(tex, Locale::En), english, "{tex}");
        assert_eq!(speech(tex, Locale::PtBr), portuguese, "{tex}");
    }
    let mathml = r#"<math><mi>x</mi><mo>∈</mo><mi mathvariant="double-struck">R</mi></math>"#;
    assert_eq!(speech(mathml, Locale::En), "x is in the real numbers");
    let asciimath = convert("x in RR", &Options { input: context_equation_core::Input::AsciiMath, ..options(Locale::PtBr) }).unwrap();
    assert_eq!(asciimath.speech, "x pertence aos reais");
    // a language without the names reads the letter, as before
    assert_eq!(speech(r"x \in \mathbb{R}", Locale::Fr), "x appartient à R");
    // only a preposition that ends a word contracts: a variable named a stays
    assert_eq!(speech(r"a \mathbb{R}", Locale::PtBr), "a os reais");
}

#[test]
fn the_dot_between_vectors_is_spoken_dot() {
    for (tex, english, portuguese) in [
        (r"\vec{v} \cdot \vec{w}", "vector v dot vector w", "vetor v escalar vetor w"),
        (r"\overrightarrow{AB} \cdot \overrightarrow{CD}", "vector A B dot vector C D", "vetor A B escalar vetor C D"),
        (r"\mathbf{u} \cdot \mathbf{v} = 0", "u dot v equals 0", "u escalar v é igual a 0"),
        (r"\boldsymbol{u} \cdot \boldsymbol{v}", "u dot v", "u escalar v"),
        (r"\vb{a} \vdot \vb{b}", "a dot b", "a escalar b"),
        (r"\vec{v}_1 \cdot \vec{v}_2", "vector v sub 1 dot vector v sub 2", "vetor v índice 1 escalar vetor v índice 2"),
        // a scalar on either side: the dot is a product of numbers
        (r"a \cdot b", "a times b", "a vezes b"),
        (r"2 \cdot \vec{v}", "2 times vector v", "2 vezes vetor v"),
        (r"\vec{a} \cdot \vec{b} + 2 \cdot 3", "vector a dot vector b plus 2 times 3", "vetor a escalar vetor b mais 2 vezes 3"),
    ] {
        assert_eq!(speech(tex, Locale::En), english, "{tex}");
        assert_eq!(speech(tex, Locale::PtBr), portuguese, "{tex}");
    }
    let asciimath = convert("vec u * vec v", &Options { input: context_equation_core::Input::AsciiMath, ..options(Locale::En) }).unwrap();
    assert_eq!(asciimath.speech, "vector u dot vector v");
    // a language without the word keeps "times"
    assert_eq!(speech(r"\vec{v} \cdot \vec{w}", Locale::De), "Vektor v mal Vektor w");
}
