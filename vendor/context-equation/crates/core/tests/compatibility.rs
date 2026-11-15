//! Compatibility with LaTeX: the cases that once rendered wrong (equilibrium arrows that did not
//! stretch, ink cut at the edge of the SVG, `\phase`, `\angl`), the array features of arydshln and
//! `\arraystretch`, and the commands of the KaTeX support list that are LaTeX's own (kernel, AMS,
//! mathtools, stmaryrd…). Each case checks the reading (tree, speech) and the geometry (where the
//! ink is), not exact coordinates.

use context_equation_core::font;
use context_equation_core::layout::{Item, MathBox};
use context_equation_core::tree::{Frame, Node, TableRule};
use context_equation_core::{convert, Locale, Options};

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

/// Converted in strict mode (so every command is known) and in tolerant mode with no warnings.
fn read(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options::default()).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    out
}

fn math_box(tex: &str) -> MathBox {
    context_equation_core::typeset(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn ink(b: &MathBox) -> (f32, f32, f32, f32) {
    b.ink(font::font()).expect("something is drawn")
}

fn attr<'a>(svg: &'a str, name: &str) -> &'a str {
    svg.split(&format!(" {name}=\"")).nth(1).unwrap().split('"').next().unwrap()
}

/// The SVG's view as (x, y, width, height), in font units.
fn view(svg: &str) -> (f32, f32, f32, f32) {
    let v: Vec<f32> = attr(svg, "viewBox").split(' ').map(|n| n.parse().unwrap()).collect();
    (v[0], v[1], v[2], v[3])
}

/// S-SVG-1: every bit of ink falls inside the view (an SVG clips what falls outside it).
fn assert_ink_inside(tex: &str, options: &Options) {
    let (b, _) = context_equation_core::typeset(tex, options).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let svg = convert(tex, options).unwrap().svg;
    let (x, y, w, h) = view(&svg);
    let Some((x0, y0, x1, y1)) = b.ink(font::font()) else { return };
    let slack = 0.01;
    // the SVG's y points down: the view goes from -top to bottom
    assert!(x0 >= x - slack && x1 <= x + w + slack, "{tex}: ink {x0}..{x1} outside the view {x}..{}", x + w);
    assert!(-y1 >= y - slack && -y0 <= y + h + slack, "{tex}: ink {y0}..{y1} outside the view {}..{}", -(y + h), -y);
}

fn nodes(node: &Node) -> Vec<&Node> {
    let mut out = vec![node];
    let children: Vec<&Node> = match node {
        Node::Row { children } => children.iter().collect(),
        Node::Scripts { base, sup, sub } => std::iter::once(base.as_ref()).chain(sup.as_deref()).chain(sub.as_deref()).collect(),
        Node::Framed { base, .. } | Node::Style { base, .. } | Node::ForcedClass { base, .. } | Node::Stacked { base, .. } => vec![base.as_ref()],
        Node::Arrow { above, below, .. } => above.as_deref().into_iter().chain(below.as_deref()).collect(),
        _ => vec![],
    };
    for c in children {
        out.extend(nodes(c));
    }
    out
}

// ---------- case 1: equilibrium arrows with labels (mhchem, \xrightleftharpoons) ----------

const ZINC: &str = r"\ce{Zn^2+ <=>[+ 2OH-][+ 2H+] $\underset{\text{amphoteres Hydroxid}}{\ce{Zn(OH)2 v}}$ <=>[+ 2OH-][+ 2H+] $\underset{\text{Hydroxozikat}}{\ce{[Zn(OH)4]^2-}}$}";

#[test]
fn case_1_the_zinc_equilibria_read_as_written() {
    let out = read(ZINC);
    let all = nodes(&out.tree);
    let arrows: Vec<_> = all.iter().filter(|n| matches!(n, Node::Arrow { symbol: '⇌', above: Some(_), below: Some(_) })).collect();
    assert_eq!(arrows.len(), 2, "two equilibria, each with a label above and below: {:?}", out.tree);
    // the names go under the formulas, from \underset and \text inside $…$
    assert_eq!(all.iter().filter(|n| matches!(n, Node::Stacked { above: None, below: Some(_), .. })).count(), 2);
    assert!(out.speech.contains("in equilibrium with") && out.speech.contains("amphoteres Hydroxid below"), "{}", out.speech);
    assert_ink_inside(ZINC, &Options::default());
}

#[test]
fn case_1_equilibrium_arrows_stretch_under_their_labels() {
    // the label is a phantom: the ink left is the arrow's alone
    let alone = ink(&math_box(r"\rightleftharpoons"));
    for tex in [r"\xrightleftharpoons{\phantom{a label wider than the arrow}}", r"\ce{A <=>[\phantom{a label wider than the arrow}] B}"] {
        let b = math_box(tex);
        let (x0, y0, x1, y1) = ink(&b);
        let label = math_box(r"\scriptstyle\phantom{a label wider than the arrow}").width;
        assert!(x1 - x0 >= label * 0.95, "{tex}: arrow {} under a label {label}", x1 - x0);
        assert!(x1 - x0 > 3.0 * (alone.2 - alone.0), "{tex}: the arrow did not stretch");
        // the two harpoons keep apart as in the font's ⇌: the pair is no flatter than the glyph
        assert!(y1 - y0 >= (alone.3 - alone.1) * 0.95, "{tex}: harpoons {} apart, glyph {}", y1 - y0, alone.3 - alone.1);
    }
}

#[test]
fn case_1_every_paired_and_long_arrow_stretches() {
    for (stretched, natural) in [
        (r"\xrightleftharpoons{\phantom{long label here}}", r"\rightleftharpoons"),
        (r"\xleftrightharpoons{\phantom{long label here}}", r"\leftrightharpoons"),
        (r"\ce{A <-->[\phantom{long label here}] B}", r"\rightleftarrows"),
        (r"\ce{A <=>>[\phantom{long label here}] B}", r"\rightleftarrows"),
        (r"\ce{A <<=>[\phantom{long label here}] B}", r"\rightleftarrows"),
        (r"\xtofrom{\phantom{long label here}}", r"\rightleftarrows"),
    ] {
        let (a, _, b, _) = ink(&math_box(stretched));
        let (c, _, d, _) = ink(&math_box(natural));
        assert!(b - a > 2.0 * (d - c), "{stretched}: {} against {}", b - a, d - c);
    }
}

// ---------- case 2: ink past the box, cut by the SVG ----------

const NAVIER_STOKES: &str =
    r"\rho \left( \frac{\partial \mathbf{u}}{\partial t} + (\mathbf{u} \cdot \nabla) \mathbf{u} \right) = - \nabla p + \mu \nabla^2 \mathbf{u} + \mathbf{f}";

#[test]
fn case_2_the_bold_f_at_the_end_is_not_cut() {
    let b = math_box(NAVIER_STOKES);
    let (x0, _, x1, _) = ink(&b);
    // the hook of the bold f passes its advance: the box ends before the ink
    assert!(x1 > b.width, "the premise of the case: ink {x1}, box {}", b.width);
    let svg = read(NAVIER_STOKES).svg;
    let (x, _, w, _) = view(&svg);
    assert!(x + w >= x1 && x <= x0, "view {x}..{} for ink {x0}..{x1}", x + w);
    assert_ink_inside(NAVIER_STOKES, &Options::default());
    assert_ink_inside(NAVIER_STOKES, &Options { display: false, ..Options::default() });
}

#[test]
fn case_2_ink_past_the_box_on_every_side() {
    for tex in [
        r"\mathbf{f}",
        r"x + \mathbf{f}",
        r"\rho",
        r"f",
        r"\cancel{x}",
        r"\bcancel{abc} + \xcancel{y}",
        r"\cancelto{0}{x}",
        r"\int_0^1 f\,dx",
        r"V_{\mathbf{f}}^{\mathbf{f}}",
        r"\textcircled{a}",
        r"\phase{\frac{\alpha}{2}}",
        r"a_{\angl{n}}",
    ] {
        assert_ink_inside(tex, &Options::default());
        assert_ink_inside(tex, &Options { display: false, ..Options::default() });
    }
}

#[test]
fn case_2_a_box_without_overhang_keeps_its_view() {
    // the view only grows where ink passes the box: 3.14 starts at 0, as before
    let svg = read("3.14").svg;
    assert_eq!(attr(&svg, "viewBox").split(' ').next(), Some("0"));
}

#[test]
fn case_2_size_in_em_follows_the_view() {
    // the SVG scales with the surrounding font size (and zoom) as one piece: width, height and the
    // baseline shift are the view's, in em, so no size makes the ink fall outside
    for tex in [NAVIER_STOKES, ZINC, r"\cancel{x}", r"\sum_{n=1}^\infty \frac{1}{n^2}"] {
        let svg = read(tex).svg;
        let (_, y, w, h) = view(&svg);
        let em = |name: &str| attr(&svg, name).trim_end_matches("em").parse::<f32>().unwrap();
        assert!((em("width") - w / 1000.0).abs() < 0.002, "{tex}");
        assert!((em("height") - h / 1000.0).abs() < 0.002, "{tex}");
        let shift: f32 = attr(&svg, "style").trim_start_matches("vertical-align: ").trim_end_matches("em").parse().unwrap();
        assert!((shift + (y + h) / 1000.0).abs() < 0.002, "{tex}: the baseline is not where the view puts it");
    }
}

#[test]
fn case_2_no_clipping_anywhere_in_the_corpus() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");
    let mut equations = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(serde_json::Value::String(tex)) = m.get("tex") {
                    out.push(tex.clone());
                }
                m.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    for f in ["gallery.json", "reference.json"] {
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{f}")).unwrap()).unwrap();
        walk(&json, &mut equations);
    }
    for f in ["constructions.json", "packages.json"] {
        let pairs: Vec<(String, String)> = serde_json::from_str(&std::fs::read_to_string(format!("{dir}/{f}")).unwrap()).unwrap();
        equations.extend(pairs.into_iter().map(|(_, tex)| tex));
    }
    assert!(equations.len() > 300, "{}", equations.len());
    for tex in &equations {
        assert_ink_inside(tex, &Options::default());
    }
}

// ---------- case 3: \phase (steinmetz) ----------

#[test]
fn case_3_phase_is_the_steinmetz_angle() {
    let out = read(r"\phase{-48\degree}");
    let Node::Row { children } = &out.tree else { panic!() };
    let Node::Framed { base, shape: Frame::Phase, .. } = &children[0] else { panic!("{:?}", out.tree) };
    // steinmetz sets the angle in text style unless the option says otherwise
    assert!(matches!(base.as_ref(), Node::Style { level: 1, .. }), "{base:?}");
    assert_eq!(out.text, "∠−48°");
    assert_eq!(out.speech, "angle negative 48 degrees");
    let pt = convert(r"\phase{-48\degree}", &Options { locale: Locale::PtBr, strict: true, ..Options::default() }).unwrap();
    assert!(pt.speech.starts_with("ângulo"), "{}", pt.speech);
    assert!(out.braille.starts_with("⠫⠪"), "Nemeth angle sign: {}", out.braille);
    assert!(pt.braille.starts_with("⠘⠒"), "CMU angle sign (§8.1): {}", pt.braille);
    assert!(!out.svg.contains("#c0392b"), "nothing drawn in red");
}

#[test]
fn case_3_phase_draws_a_foot_under_the_angle_and_a_side_left_of_it() {
    let content = math_box(r"\textstyle{-48\degree}");
    let b = math_box(r"\phase{-48\degree}");
    let lines: Vec<_> = b.items.iter().filter_map(|i| if let Item::Line { x1, y1, x2, y2, .. } = i { Some((*x1, *y1, *x2, *y2)) } else { None }).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    let foot = lines.iter().find(|l| (l.1 - l.3).abs() < 0.01).expect("a horizontal foot");
    let side = lines.iter().find(|l| (l.1 - l.3).abs() > 1.0).expect("a rising side");
    // the foot runs under the whole angle, below its depth (.35ex in steinmetz)
    assert!(foot.1 < -content.depth && foot.0 <= 0.01 && foot.2 >= b.width - 0.01, "{foot:?}");
    // the side rises from the foot with slope 2 to the content's height
    assert!((side.0 - foot.0).abs() < 0.01 && (side.3 - content.height).abs() < 1.0, "{side:?}");
    assert!(((side.3 - side.1) / (side.2 - side.0) - 2.0).abs() < 0.1, "{side:?}");
    // and the content starts where the side ends
    assert!(b.width > content.width + (side.2 - side.0) - 1.0);
}

#[test]
fn case_3_phase_option_chooses_the_style() {
    let level = |tex: &str| match &read(tex).tree {
        Node::Row { children } => match &children[0] {
            Node::Framed { base, .. } => match base.as_ref() {
                Node::Style { level, .. } => Some(*level),
                _ => None,
            },
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    assert_eq!(level(r"\phase{\alpha}"), Some(1));
    assert_eq!(level(r"\phase[0]{\alpha}"), Some(0));
    assert_eq!(level(r"\phase[3]{\alpha}"), Some(3));
    // the display style makes the fraction, and so the angle, taller
    assert!(math_box(r"\phase[0]{\frac{\alpha}{2}}").height > math_box(r"\phase[1]{\frac{\alpha}{2}}").height);
}

// ---------- case 4: \angl (actuarialangle) ----------

#[test]
fn case_4_angl_is_the_actuarial_angle() {
    let out = read(r"a_{\angl{n}}");
    let Node::Row { children } = &out.tree else { panic!() };
    let Node::Scripts { sub: Some(sub), .. } = &children[0] else { panic!("{:?}", out.tree) };
    assert!(matches!(sub.as_ref(), Node::Framed { shape: Frame::ActuarialAngle, .. }), "{sub:?}");
    assert_eq!(out.text, "a_(n⌉)");
    assert_eq!(out.speech, "a sub angle n");
    // the shortcuts of the package
    assert_eq!(read(r"a_{\angln}").tree, out.tree);
    assert_eq!(read(r"\ddot{a}_{\anglr}").tree, read(r"\ddot{a}_{\angl{r}}").tree);
    assert_eq!(read(r"\anglk").tree, read(r"\angl{k}").tree);
    // not \angle: the actuarial angle is a frame, \angle the symbol ∠
    assert_ne!(read(r"\angl{n}").tree, read(r"\angle n").tree);
}

#[test]
fn case_4_angl_draws_a_bar_over_and_a_rule_down_the_right() {
    let n = math_box(r"\scriptstyle n");
    let b = math_box(r"\scriptstyle\angl{n}");
    let rules: Vec<_> = b.items.iter().filter_map(|i| if let Item::Rule { x, y, width, height, .. } = i { Some((*x, *y, *width, *height)) } else { None }).collect();
    assert_eq!(rules.len(), 2, "{rules:?}");
    let bar = rules.iter().find(|r| r.2 > r.3).unwrap();
    let side = rules.iter().find(|r| r.3 > r.2).unwrap();
    assert!(bar.1 > n.height, "the bar sits over the letter: {bar:?}");
    // the side rule closes the corner at the right and goes below the baseline (to a parenthesis' depth)
    assert!((side.0 + side.2 - (bar.0 + bar.2)).abs() < 0.01, "{side:?} {bar:?}");
    assert!(side.1 < 0.0 && side.1 + side.3 >= bar.1, "{side:?}");
    // 1mu on each side, 2mu inside: wider than the letter
    assert!(b.width > n.width);
}

// ---------- the compatibility block of section 3 ----------

const BLOCK: &str = r"\usepackage{amsmath}
\usepackage{amssymb}
\usepackage{amsfonts}
\usepackage{mathtools}
\usepackage{cancel}
\usepackage{mhchem}

\def\arraystretch{1.5}

\begin{array}{c:c:c}
a & b & c \\ \hline
d & e & f \\
\hdashline
g & h & i
\end{array}";

fn table(tex: &str) -> (Vec<usize>, Vec<usize>, Vec<TableRule>, f32) {
    let out = read(tex);
    let found = nodes(&out.tree).into_iter().find_map(|n| match n {
        Node::Table { verticals, dashed, horizontals, stretch, .. } => Some((verticals.clone(), dashed.clone(), horizontals.clone(), *stretch)),
        _ => None,
    });
    found.unwrap_or_else(|| panic!("no table in {:?}", out.tree))
}

#[test]
fn section_3_the_block_reads_without_warnings() {
    let (verticals, dashed, horizontals, stretch) = table(BLOCK);
    assert!(verticals.is_empty());
    assert_eq!(dashed, vec![1, 2], "the ':' between the columns");
    assert_eq!(horizontals, vec![TableRule { row: 1, from: 0, to: usize::MAX, dashed: false }, TableRule { row: 2, from: 0, to: usize::MAX, dashed: true }]);
    assert_eq!(stretch, 1.5);
    assert_ink_inside(BLOCK, &Options::default());
}

#[test]
fn section_3_arraystretch_makes_the_rows_taller() {
    let body = r"\begin{array}{cc} a & b \\ c & d \end{array}";
    let plain = math_box(body).total();
    let stretched = math_box(&format!(r"\def\arraystretch{{1.5}} {body}")).total();
    let renewed = math_box(&format!(r"\renewcommand{{\arraystretch}}{{1.5}} {body}")).total();
    assert!((stretched / plain - 1.5).abs() < 0.05, "{stretched} against {plain}");
    assert_eq!(stretched, renewed);
    // the matrices of amsmath are arrays too; the alignments space their rows with \jot instead
    let matrix = |s: &str| math_box(&format!(r"{s} \begin{{pmatrix}} a \\ b \end{{pmatrix}}")).total();
    assert!(matrix(r"\def\arraystretch{2}") > matrix(""));
    let aligned = |s: &str| math_box(&format!(r"{s} \begin{{aligned}} a &= b \\ c &= d \end{{aligned}}")).total();
    assert_eq!(aligned(r"\def\arraystretch{2}"), aligned(""));
}

#[test]
fn section_3_dashed_rules_are_dashes() {
    let rules = |tex: &str| math_box(tex).items.iter().filter(|i| matches!(i, Item::Rule { .. })).count();
    let solid = rules(r"\begin{array}{c|c} a & b \\ \hline c & d \end{array}");
    let dashed = rules(r"\begin{array}{c:c} a & b \\ \hdashline c & d \end{array}");
    assert_eq!(solid, 3, "one rule per row segment and the \\hline");
    assert!(dashed > 2 * solid, "{dashed} dashes");
    let (_, _, horizontals, _) = table(r"\begin{array}{ccc} a & b & c \\ \cdashline{2-3} d & e & f \end{array}");
    assert_eq!(horizontals, vec![TableRule { row: 1, from: 1, to: 2, dashed: true }]);
}

#[test]
fn section_3_vertical_rules_stay_inside_the_table() {
    // they once fell below the table, where only the view's clipping hid them
    for tex in [
        r"\begin{array}{|c|c|} \hline 1 & 2 \\ \hline 3 & 4 \\ \hline \end{array}",
        r"\begin{array}{c:c:c} a & b & c \\ d & e & f \\ g & h & i \end{array}",
        r"\left( \begin{array}{c|c} a & b \\ c & d \end{array} \right)",
    ] {
        let b = math_box(tex);
        let (_, y0, _, y1) = ink(&b);
        assert!(y0 >= -b.depth - 0.01 && y1 <= b.height + 0.01, "{tex}: ink {y0}..{y1}, box {}..{}", -b.depth, b.height);
        assert_ink_inside(tex, &Options::default());
    }
}

#[test]
fn section_3_outer_rules_sit_at_the_edge() {
    // LaTeX's array puts an outer | at the edge, the \arraycolsep inside it: the \hline ends at the rules
    let b = math_box(r"\begin{array}{|c|} \hline x \\ \hline \end{array}");
    let xs: Vec<(f32, f32)> =
        b.items.iter().filter_map(|i| if let Item::Rule { x, width, height, .. } = i { (height > width).then_some((*x, *x + *width)) } else { None }).collect();
    assert_eq!(xs.len(), 2);
    assert!(xs.iter().any(|r| r.0.abs() < 0.01), "{xs:?}");
    assert!(xs.iter().any(|r| (r.1 - b.width).abs() < 0.01), "{xs:?} in {}", b.width);
}

#[test]
fn section_3_each_package_on_its_own() {
    for tex in [
        // amsmath
        r"\begin{align} a &= b \\ c &= d \end{align}",
        r"\begin{gather} a \\ b \end{gather}",
        r"\begin{cases} 1 & x > 0 \\ 0 & x \le 0 \end{cases}",
        r"\dfrac{a}{b} + \tfrac{c}{d} + \binom{n}{k} + \operatorname*{arg\,max}_x f",
        r"\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix} \begin{vmatrix} a & b \\ c & d \end{vmatrix}",
        r"\overset{!}{=} \underset{x}{\lim} \sideset{}{'}\sum_{n} a_n",
        r"\varGamma \varOmega \injlim_i \projlim_j \varliminf_k \varlimsup_k",
        // amssymb and amsfonts
        r"\mathbb{R} \mathfrak{g} \mathcal{F} \leqslant \geqslant \nleq \subsetneq \therefore \square \blacksquare",
        r"\backepsilon \gtreqqless \lesseqqgtr \circledR \maltese \circledcirc",
        // mathtools
        r"a \coloneqq b \eqqcolon c \Coloneq d \dblcolon e \colonapprox f \Colonsim g",
        r"\xtwoheadrightarrow{f} \xhookleftarrow{g} \xrightharpoonup{h} \xleftharpoondown{i} \lparen x \rparen",
        r"\begin{pmatrix*}[r] -1 & 2 \\ 3 & -4 \end{pmatrix*} \smashoperator{\sum_{i=1}^n} \mathclap{x}",
        // cancel
        r"\cancel{a} \bcancel{b} \xcancel{c} \cancelto{0}{d}",
        // mhchem
        r"\ce{CO2 + C -> 2 CO} \ce{A <=>[K] B} \ce{SO4^2- + Ba^2+ -> BaSO4 v} \pu{123 kJ/mol}",
    ] {
        read(tex);
        assert_ink_inside(tex, &Options::default());
    }
}

// ---------- commands of the KaTeX list that are LaTeX's own ----------

#[test]
fn katex_list_latex_commands_now_known() {
    for tex in [
        // kernel
        r"\lgroup x \rgroup \lmoustache y \rmoustache",
        r"\mathellipsis \mathsterling \ldotp \cdotp \lq \rq",
        r"a\space b \nobreakspace c",
        r"\intop_0^1 \ointop",
        r"\underbar{X}",
        r"\char`a \char65 \char'101",
        r"\global\def\add#1#2{#1+#2} \add 2 3",
        // stmaryrd, unicode-math
        r"\lBrace x \rBrace \mapsfrom \imageof \origof",
        // colonequals
        r"a \colonequals b \equalscolon c \coloncolon d \ratio e \colonminus f \minuscoloncolon g",
        // harpoon, undertilde, braket, amsmath
        r"\overleftharpoon{AB} \overrightharpoon{AB} \utilde{AB} \Bra{\psi} \Ket{\phi} a \And b",
        r"\operatornamewithlimits{argmax}_x",
        // hyperref: the text of the link
        r"\href{https://example.org}{x^2}",
        // text mode
        r"\text{\AE \ae \OE \oe \i \j \L \H{o} \r{a} \textdollar \textunderscore \textbar \textellipsis \textquotedblleft x\textquotedblright}",
    ] {
        read(tex);
    }
}

#[test]
fn katex_list_readings_follow_latex() {
    let tree = |tex: &str| format!("{:?}", read(tex).tree);
    // amssymb: \circledcirc is the operator ⊚, not a frame
    assert!(tree(r"a \circledcirc b").contains("symbol: '⊚', class: Bin"));
    // mathtools since 2022: \coloneq is := and \eqcolon is =:, as \coloneqq and \eqqcolon
    assert_eq!(read(r"a \coloneq b").tree, read(r"a \coloneqq b").tree);
    assert_eq!(read(r"a \eqcolon b").tree, read(r"a \eqqcolon b").tree);
    assert_eq!(read(r"a \Coloneq b").tree, read(r"a \Coloneqq b").tree);
    // amsmath: \varGamma is the italic capital, \Gamma the upright one
    assert!(tree(r"\varGamma").contains("variant: Italic") && tree(r"\Gamma").contains("variant: Upright"));
    // \intop takes limits in display style, not inline
    let limits = |display: bool| {
        let o = Options { display, ..strict() };
        let b = context_equation_core::typeset(r"\intop_0^1", &o).unwrap().0;
        let side = context_equation_core::typeset(r"\int_0^1", &o).unwrap().0;
        b.width < side.width
    };
    assert!(limits(true) && !limits(false));
    // \char is the character with that code
    assert_eq!(read(r#"\char"263A"#).text, "☺");
    // \operatorname keeps a thin space: inj lim
    assert_eq!(read(r"\injlim_i").speech, read(r"\operatorname*{inj\,lim}_i").speech);
}

// ---------- unknown and unsupported commands ----------

#[test]
fn unsupported_commands_are_refused_or_shown_in_red() {
    // unknown names are refused in strict mode, and with a warning otherwise, never replaced in
    // silence by a look-alike (KaTeX's texvc aliases, \R or \larr, are known since #172: texvc_aliases.rs)
    for tex in [r"\mathreflectbox{x}", r"\foocommand"] {
        assert!(convert(tex, &strict()).is_err(), "{tex} accepted in strict mode");
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1"), "{tex}: {:?}", out.warnings);
    }
    // what has no reading at all is drawn in red, as written
    assert!(convert(r"\foocommand", &Options::default()).unwrap().svg.contains("#c0392b"), "\\foocommand: not drawn in red");
}

// ---------- TeX primitives and package commands added after the audit ----------

fn glyphs(b: &MathBox) -> usize {
    b.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count()
}

#[test]
fn mathchoice_sets_the_branch_of_its_style() {
    let out = read(r"\mathchoice{D}{T}{S}{SS}");
    assert!(matches!(&out.tree, Node::Row { children } if matches!(&children[0], Node::Choice { branches } if branches.len() == 4)), "{:?}", out.tree);
    // one glyph in display and text style, two in the scriptscript branch of a second-level script
    let display = math_box(r"\mathchoice{D}{T}{S}{SS}");
    let inline = context_equation_core::typeset(r"\mathchoice{D}{T}{S}{SS}", &Options { display: false, ..strict() }).unwrap().0;
    let script = math_box(r"x_{\mathchoice{D}{T}{S}{SS}}");
    let scriptscript = math_box(r"x_{y_{\mathchoice{D}{T}{S}{SS}}}");
    assert_eq!((glyphs(&display), glyphs(&inline)), (1, 1));
    assert_eq!(glyphs(&script), 2, "x and S");
    assert_eq!(glyphs(&scriptscript), 4, "x, y and the two letters of SS");
    assert_ne!(display.items, inline.items, "D and T are different letters");
    // KaTeX's example: one thin space in display style, two inline
    let spaced = r"a\mathchoice{\,}{\,\,}{\,\,\,}{\,\,\,\,}b";
    let gap = context_equation_core::typeset(spaced, &Options { display: false, ..strict() }).unwrap().0.width - math_box(spaced).width;
    assert!((gap - 1000.0 / 6.0).abs() < 1.0, "a thin space (3mu) more inline: {gap}");
    // speech and text, which do not follow the style, read the text style's branch
    assert_eq!(out.text, "T");
}

#[test]
fn vcenter_puts_the_middle_of_its_box_on_the_axis() {
    let axis = 250.0_f32; // STIX Two Math's axis height, in font units
    for tex in [r"\vcenter{x}", r"\vcenter{\frac{\frac a b}c}", r"\vcenter{\rule{1em}{2em}}"] {
        let b = math_box(tex);
        let middle = (b.height - b.depth) / 2.0;
        assert!((middle - axis).abs() < 10.0, "{tex}: middle {middle}");
    }
    read(r"a+\left(\vcenter{\frac{\frac a b}c}\right)");
}

#[test]
fn above_draws_a_rule_of_the_given_thickness() {
    let rule = |tex: &str| {
        math_box(tex).items.iter().filter_map(|i| if let Item::Rule { width, height, .. } = i { Some((*width, *height)) } else { None }).find(|(w, h)| w > h).map(|(_, h)| h)
    };
    // 2pt is 0.2 em, absolute: the same in a script
    let thick = rule(r"{a \above{2pt} b+1}").unwrap();
    assert!((thick - 200.0).abs() < 1.0, "{thick}");
    assert!((rule(r"{a \above 2pt b}").unwrap() - thick).abs() < 0.01, "bare dimension, as TeX reads it");
    assert!((rule(r"x^{a \above{2pt} b}").unwrap() - thick).abs() < 0.01);
    // 0pt is \atop: no rule
    assert!(rule(r"{a \above 0pt b}").is_none());
    // \genfrac's thickness is drawn too
    assert!((rule(r"\genfrac{}{}{1pt}{}{n}{k}").unwrap() - 100.0).abs() < 1.0);
    assert_eq!(read(r"{a \above{2pt} b}").speech, read(r"\frac{a}{b}").speech);
    assert!(convert(r"{a \above b}", &strict()).is_err(), "a dimension is required");
}

#[test]
fn reflectbox_mirrors_its_content() {
    let plain = math_box(r"\text{abc}");
    let mirrored = math_box(r"\reflectbox{abc}");
    assert_eq!(plain.width, mirrored.width);
    let flipped = mirrored.items.iter().filter(|i| matches!(i, Item::Glyph { flip: true, .. })).count();
    assert_eq!(flipped, 3);
    // the first letter is now on the right, its ink where the last one's was
    let (x0, _, x1, _) = ink(&plain);
    let (m0, _, m1, _) = ink(&mirrored);
    assert!((m0 - (plain.width - x1)).abs() < 0.5 && (m1 - (plain.width - x0)).abs() < 0.5, "{m0}..{m1} against {x0}..{x1}");
    // drawn with a negative x scale; read as what it says
    let out = read(r"\reflectbox{$x^2$}");
    assert!(out.svg.contains("scale(-1 1)"), "no mirrored glyph in the SVG");
    assert_eq!(out.speech, read(r"x^2").speech);
    assert_ink_inside(r"\reflectbox{$x^2$} + \reflectbox{f}", &Options::default());
}

/// Width, height and depth of a box.
fn size(b: &MathBox) -> (f32, f32, f32) {
    (b.width, b.height, b.depth)
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn l_box_1_scalebox_across_up_and_mirrored() {
    let (w, h, d) = size(&math_box(r"\text{Qy}"));
    // \scalebox{-1}[1] is \reflectbox: the same glyphs, flipped (the CircuiTikZ manual's \ctikzflipx)
    let mirrored = math_box(r"\scalebox{-1}[1]{Qy}");
    assert_eq!(size(&mirrored), (w, h, d));
    assert_eq!(mirrored.items.iter().filter(|i| matches!(i, Item::Glyph { flip: true, .. })).count(), 2);
    let wide = math_box(r"\scalebox{2}[1]{Qy}");
    assert!(close(wide.width, 2.0 * w) && close(wide.height, h) && close(wide.depth, d), "{:?}", size(&wide));
    // upside down: the height becomes the depth
    let turned = math_box(r"\scalebox{1}[-1]{Qy}");
    assert!(close(turned.height, d) && close(turned.depth, h), "{:?}", size(&turned));
    for tex in [r"\scalebox{2}[1]{Qy}", r"\scalebox{-1}[2]{Qy}", r"\scalebox{1}[-1]{Qy}"] {
        assert!(read(tex).svg.contains("matrix("), "{tex}");
        assert_ink_inside(tex, &Options::default());
    }
    // one factor, as before: glyphs scaled, no transform
    assert!(!read(r"\scalebox{2}{Qy}").svg.contains("matrix("));
}

#[test]
fn l_box_1_rotatebox_takes_the_box_around_the_turned_one() {
    let (w, h, d) = size(&math_box(r"\text{abc}"));
    // a quarter turn around the left end of the baseline: the width is now the height
    let up = math_box(r"\rotatebox{90}{abc}");
    assert!(close(up.width, h + d) && close(up.height, w) && close(up.depth, 0.0), "{:?}", size(&up));
    let down = math_box(r"\rotatebox{-90}{abc}");
    assert!(close(down.width, h + d) && close(down.height, 0.0) && close(down.depth, w), "{:?}", size(&down));
    let half = math_box(r"\rotatebox{180}{abc}");
    assert!(close(half.width, w) && close(half.height, d) && close(half.depth, h), "{:?}", size(&half));
    // around the center, the turned box is centered where the box was
    let centered = math_box(r"\rotatebox[origin=c]{180}{abc}");
    assert!(close(centered.height - centered.depth, h - d), "{:?}", size(&centered));
    let svg = read(r"\rotatebox{90}{abc}").svg;
    assert!(svg.contains(r#"<g transform="matrix(0 1 -1 0 "#), "{svg}");
    for tex in [r"\rotatebox{90}{$I_0$}", r"\rotatebox{45}{abc}", r"\rotatebox[origin=c]{180}{abc} + x", r"\rotatebox[origin=lt]{-30}{abc}"] {
        assert_ink_inside(tex, &Options::default());
    }
    assert_eq!(read(r"\rotatebox{90}{$I_0$}").speech, read("I_0").speech);
}

#[test]
fn l_box_1_raisebox_height_and_depth() {
    let (_, h, d) = size(&math_box(r"\text{gy}"));
    assert!(d > 0.0);
    // the CircuiTikZ manual's \killdepth: same height, no depth
    let killed = math_box(r"\raisebox{0pt}[\height][0pt]{gy}");
    assert!(close(killed.height, h) && killed.depth == 0.0, "{:?}", size(&killed));
    let raised = math_box(r"\raisebox{1em}[0.5\height][\depth]{gy}");
    let lifted = math_box(r"\raisebox{1em}{gy}");
    assert!(close(raised.height, lifted.height / 2.0) && close(raised.depth, lifted.depth), "{:?}", size(&raised));
    assert!(close(math_box(r"\raisebox{0pt}[\totalheight]{gy}").height, h + d));
}

#[test]
fn l_box_1_resizebox_to_the_size_given() {
    let (w, h, d) = size(&math_box(r"\text{abc}"));
    let one_cm = math_box(r"\resizebox{1cm}{!}{abc}");
    let em = math_box(r"\rule{1em}{1em}").width;
    assert!(close(one_cm.width, 2.845 * em) && close(one_cm.height / one_cm.width, h / w), "{:?}", size(&one_cm));
    let both = math_box(r"\resizebox{2\width}{\height}{abc}");
    assert!(close(both.width, 2.0 * w) && close(both.height, h) && close(both.depth, d), "{:?}", size(&both));
    let total = math_box(r"\resizebox*{!}{2\totalheight}{abc}");
    assert!(close(total.height + total.depth, 2.0 * (h + d)) && close(total.width, 2.0 * w), "{:?}", size(&total));
    assert_ink_inside(r"\resizebox{2cm}{0.5cm}{abc}", &Options::default());
}

#[test]
fn l_box_1_graphicx_boxes_inside_text() {
    // in a label (text mode) as in math, with no warning
    for (text, math) in [
        (r"\text{\scalebox{-1}[1]{Q1}}", r"\scalebox{-1}[1]{Q1}"),
        (r"\text{\reflectbox{Q1}}", r"\reflectbox{Q1}"),
        (r"\text{\rotatebox{90}{$I_0$}}", r"\rotatebox{90}{$I_0$}"),
        (r"\text{\rotatebox[origin=c]{90}{$I_0$}}", r"\rotatebox[origin=c]{90}{$I_0$}"),
        (r"\text{\raisebox{0pt}[\height][0pt]{gy}}", r"\raisebox{0pt}[\height][0pt]{gy}"),
        (r"\text{\resizebox{1cm}{!}{abc}}", r"\resizebox{1cm}{!}{abc}"),
        (r"\text{\resizebox*{1cm}{!}{abc}}", r"\resizebox*{1cm}{!}{abc}"),
    ] {
        assert_eq!(size(&math_box(text)), size(&math_box(math)), "{text}");
    }
    let r = read(r"\text{x \scalebox{2}[1]{y} z}");
    assert_eq!(r.text, "x y z");
}

#[test]
fn url_is_the_address_as_written_in_typewriter() {
    let out = read(r"\url{https://katex.org/a_b%20c#x}");
    assert_eq!(out.text, "https://katex.org/a_b%20c#x");
    assert_eq!(out.speech, "https://katex.org/a_b%20c#x");
    // the letters are the typewriter ones; \texttt and \textbf are no longer plain text
    let font = font::font();
    let mono = font.glyph('𝚊').unwrap();
    let has = |tex: &str, id| math_box(tex).items.iter().any(|i| matches!(i, Item::Glyph { id: g, .. } if *g == id));
    assert!(has(r"\url{a}", mono) && has(r"\texttt{a}", mono) && has(r"\text{x \texttt{a}}", mono) && has(r"\text{\tt a}", mono));
    assert!(has(r"\textbf{a}", font.glyph('𝐚').unwrap()));
    assert!(!has(r"\text{a}", mono));
}

// ---------- MathJax's \stackbin (#121) ----------

#[test]
fn stackbin_is_stackrel_with_the_spacing_of_a_binary_operator() {
    let tree = |tex: &str| format!("{:?}", read(tex).tree);
    assert!(tree(r"a \stackbin{*}{+} b").contains("ForcedClass { class: Bin"), "{}", tree(r"a \stackbin{*}{+} b"));
    // the same drawing as \stackrel; medium spaces around it (4mu), as around +, not thick ones (5mu)
    let width = |tex: &str| math_box(tex).width;
    assert_eq!(math_box(r"\stackbin{*}{+}").width, math_box(r"\stackrel{*}{+}").width);
    let around = |tex: &str, alone: &str| width(&format!("a {tex} b")) - width(alone);
    assert!((around(r"\stackbin{*}{+}", r"\stackbin{*}{+}") - around("+", "+")).abs() < 0.01);
    assert!(width(r"a \stackbin{*}{+} b") < width(r"a \stackrel{*}{+} b"));
    assert_eq!(width(r"x \stackbin{\text{def}}{\times} y"), width(r"x \overset{\text{def}}{\times} y"));
    // with nothing before it, a binary operator is ordinary (TeX's rule 5)
    assert_eq!(width(r"\stackbin{*}{+} b"), width(r"\overset{*}{\mathord{+}} b"));
    assert_eq!(read(r"a \stackbin{*}{+} b").speech, read(r"a \overset{*}{+} b").speech);
}

// ---------- stmaryrd, marvosym, wasysym and \mathbold (#124) ----------

#[test]
fn stmaryrd_marvosym_wasysym_symbols_and_mathbold() {
    let first = |tex: &str| {
        let out = read(tex);
        format!("{:?}", out.tree.children().first().unwrap_or(&out.tree))
    };
    // stmaryrd declares \lightning \mathord; marvosym's \Lightning is the same sign
    for tex in [r"\lightning", r"\Lightning"] {
        assert!(first(tex).contains("symbol: '↯', class: Ord"), "{tex}: {}", first(tex));
    }
    for (tex, symbol, class) in [
        (r"\Mapsto", '⤇', "Rel"),
        (r"\Mapsfrom", '⤆', "Rel"),
        (r"\Longmapsto", '⟾', "Rel"),
        (r"\Longmapsfrom", '⟽', "Rel"),
        (r"\longmapsfrom", '⟻', "Rel"),
        (r"\boxbox", '⧈', "Bin"),
        (r"\boxslash", '⧄', "Bin"),
        (r"\boxbslash", '⧅', "Bin"),
        (r"\boxast", '⧆', "Bin"),
        (r"\boxcircle", '⧇', "Bin"),
        (r"\obar", '⦶', "Bin"),
        (r"\obslash", '⦸', "Bin"),
        (r"\olessthan", '⧀', "Bin"),
        (r"\ogreaterthan", '⧁', "Bin"),
        (r"\fatsemi", '⨟', "Bin"),
        (r"\interleave", '⫴', "Bin"),
        (r"\talloblong", '⫾', "Bin"),
        (r"\sslash", '⫽', "Bin"),
        (r"\Lbag", '⟅', "Open"),
        (r"\Rbag", '⟆', "Close"),
        (r"\llparenthesis", '⦇', "Open"),
        (r"\rrparenthesis", '⦈', "Close"),
        (r"\invneg", '⌐', "Rel"),
        (r"\checked", '✓', "Ord"),
        (r"\diameter", '⌀', "Ord"),
    ] {
        let tex = format!("a {tex} b");
        let out = read(&tex);
        let tree = format!("{:?}", out.tree);
        assert!(tree.contains(&format!("symbol: '{symbol}', class: {class}")), "{tex}: {tree}");
        // the font has it
        assert!(font::font().glyph(symbol).is_some(), "{symbol}");
    }
    // the bags and the image brackets grow with \left and \right
    read(r"\left\Lbag \frac{a}{b} \right\Rbag \left\llparenthesis \frac{a}{b} \right\rrparenthesis");
    // \mathbold is \boldsymbol: bold italic, Latin and Greek
    assert_eq!(read(r"\mathbold{x} + \mathbold{\alpha}").tree, read(r"\boldsymbol{x} + \boldsymbol{\alpha}").tree);
    assert_eq!(read(r"\mathbold\alpha").tree, read(r"\boldsymbol\alpha").tree);
    // in text, where tolerant mode reads it as math, as \boldsymbol
    let tolerant = |tex: &str| convert(tex, &Options::default()).unwrap();
    assert_eq!(tolerant(r"\text{a \mathbold{x}}").tree, tolerant(r"\text{a \boldsymbol{x}}").tree);
}

// ---------- group accents (#118) ----------

#[test]
fn overgroup_and_undergroup_are_stretchy_accents_over_and_under() {
    let accent = |tex: &str| {
        let out = read(tex);
        match nodes(&out.tree).into_iter().find(|n| matches!(n, Node::Accent { .. })) {
            Some(Node::Accent { accent, stretchy, below, .. }) => (*accent, *stretchy, *below),
            _ => panic!("{tex}: {:?}", out.tree),
        }
    };
    assert_eq!(accent(r"\overgroup{AB}"), ('⏠', true, false));
    assert_eq!(accent(r"\undergroup{AB}"), ('⏡', true, true));
    // \underparen goes under its base too, as the "under" in its name says
    assert_eq!(accent(r"\underparen{AB}"), ('⏝', true, true));
    // over: ink above the base and none below; under: the other way round
    let (base, over, under) = (math_box("AB"), math_box(r"\overgroup{AB}"), math_box(r"\undergroup{AB}"));
    assert!(over.height > base.height && over.depth <= base.depth + 0.01);
    assert!(under.depth > base.depth && under.height <= base.height + 0.01);
    // the speech reads the arc, as \overparen
    assert_eq!(read(r"\overgroup{AB}").speech, read(r"\overparen{AB}").speech);
}

#[test]
fn overgroup_covers_a_base_wider_than_the_widest_size_of_the_font() {
    // STIX Two Math has sizes of ⏠ and ⏡ up to 2.6 em and no assembly: past that, the corners and
    // a rule as long as needed
    for tex in [r"\overgroup{ABCDEFGHIJKL}", r"\undergroup{a+b+c+d+e+f+g+h}", r"\overgroup{x+y+z}", r"\overgroup{A}"] {
        let base = math_box(tex.split_once('{').unwrap().1.trim_end_matches('}'));
        let b = math_box(tex);
        let (x0, _, x1, _) = ink(&b);
        let (b0, _, b1, _) = ink(&base);
        assert!(x1 - x0 >= (b1 - b0) * 0.9, "{tex}: accent {x0}..{x1}, base {b0}..{b1}");
        assert_ink_inside(tex, &Options::default());
    }
    // a wide one has its corners at the ends, the rule between them
    let wide = math_box(r"\overgroup{ABCDEFGHIJKL}");
    let drawn = wide.items.iter().filter(|i| matches!(i, Item::Line { .. } | Item::Rule { .. })).count();
    assert_eq!(drawn, 3, "{:?}", wide.items);
}

// ---------- mathtools' \vdotswithin (#120) ----------

/// Middles (x of the ink's center) of the glyphs of `c` in a box, top to bottom.
fn glyph_middles(b: &MathBox, c: char) -> Vec<(f32, f32)> {
    let f = font::font();
    let id = f.glyph(c).unwrap();
    let mut found: Vec<(f32, f32)> = b
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyph { id: g, x, y, scale, .. } if *g == id => {
                let (x0, _, x1, _) = f.limits(id);
                Some((x + (x0 + x1) / 2.0 * scale, *y))
            }
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| b.1.total_cmp(&a.1));
    found
}

#[test]
fn vdotswithin_centers_the_dots_under_the_symbol_above() {
    for (tex, symbol) in [
        (r"\begin{align*} a &= b \\ &\vdotswithin{=} \\ &= c \end{align*}", '='),
        (r"\begin{align*} x + y &\le z \\ &\vdotswithin{\le} \\ &\le w \end{align*}", '≤'),
        (r"\begin{align*} a &= b \\ \shortvdotswithin{=} &= c \end{align*}", '='),
    ] {
        let b = math_box(tex);
        let symbols = glyph_middles(&b, symbol);
        let dots = glyph_middles(&b, '⋮');
        assert_eq!((symbols.len(), dots.len()), (2, 1), "{tex}");
        assert!((dots[0].0 - symbols[0].0).abs() < 1.0, "{tex}: ⋮ at {}, {symbol} at {}", dots[0].0, symbols[0].0);
        // between the two lines
        assert!(symbols[0].1 > dots[0].1 && dots[0].1 > symbols[1].1, "{tex}");
        assert_ink_inside(tex, &Options::default());
    }
    // the box is as wide as the symbol with its spaces, `{}={}`
    assert_eq!(math_box(r"\vdotswithin{=}").width, math_box(r"{}={}").width);
    // read as "and so on", as \vdots
    let out = read(r"\begin{align*} a &= b \\ &\vdotswithin{=} \\ &= c \end{align*}");
    assert_eq!(out.speech, "a equals b; and so on; equals c");
    assert_eq!(out.text, "a = b; ⋮; = c");
}

#[test]
fn shortvdotswithin_is_a_row_of_its_own_set_closer() {
    let short = read(r"\begin{align*} a &= b \\ \shortvdotswithin{=} &= c \end{align*}");
    let long = read(r"\begin{align*} a &= b \\ &\vdotswithin{=} \\ &= c \end{align*}");
    assert_eq!(short.speech, long.speech);
    let Node::Table { rows, .. } = short.tree.children().first().unwrap_or(&short.tree) else { panic!("{:?}", short.tree) };
    assert_eq!(rows.len(), 3);
    assert!(matches!(&rows[1][1], Node::VdotsWithin { short: true, .. }), "{:?}", rows[1]);
    let Node::Table { rows, .. } = long.tree.children().first().unwrap_or(&long.tree) else { panic!("{:?}", long.tree) };
    assert!(matches!(&rows[1][1], Node::VdotsWithin { short: false, .. }), "{:?}", rows[1]);
    // the short row ends with its own row: a \vdotswithin after it is not short
    let after = read(r"\begin{align*} a &= b \\ \shortvdotswithin{=} &= c \\ &\vdotswithin{=} \\ &= d \end{align*}");
    let Node::Table { rows, .. } = after.tree.children().first().unwrap_or(&after.tree) else { panic!("{:?}", after.tree) };
    assert!(matches!(&rows[3][1], Node::VdotsWithin { short: false, .. }), "{:?}", rows[3]);
    let (s, l) = (math_box(r"\begin{align*} a &= b \\ \shortvdotswithin{=} &= c \end{align*}"), math_box(r"\begin{align*} a &= b \\ &\vdotswithin{=} \\ &= c \end{align*}"));
    assert!(s.height + s.depth < l.height + l.depth - 100.0, "{} vs {}", s.height + s.depth, l.height + l.depth);
    assert_eq!(s.width, l.width);
    // the star puts the dots in the first column, under a symbol that ends it
    let b = math_box(r"\begin{align*} a = {} &b \\ \shortvdotswithin*{=} c = {} &d \end{align*}");
    let (symbols, dots) = (glyph_middles(&b, '='), glyph_middles(&b, '⋮'));
    assert!((dots[0].0 - symbols[0].0).abs() < 1.0, "⋮ at {}, = at {}", dots[0].0, symbols[0].0);
}

/// Small reading slips found against the KaTeX and MathJax command lists (#164).
#[test]
fn text_dashes_cfrac_alignment_unicode_font_and_centernot() {
    // LaTeX's (and KaTeX's) dash ligatures in text; typewriter type has none
    assert_eq!(read(r"\text{a -- b --- c}").text, "a – b — c");
    assert_eq!(read(r"\text{x - y}").text, "x - y");
    assert_eq!(read(r"\texttt{a -- b}").text, "a -- b");
    assert_eq!(read(r"\text{\ttfamily a -- b}").text, "a -- b");
    // \cfrac[l], \cfrac[r]: the numerator at one side (MathML's numalign, as MathJax writes it)
    let one = |tex: &str| glyph_middles(&math_box(tex), '1')[0].0;
    let (left, center, right) = (one(r"\cfrac[l]{1}{x+x+x}"), one(r"\cfrac{1}{x+x+x}"), one(r"\cfrac[r]{1}{x+x+x}"));
    assert!(left < center - 300.0 && right > center + 300.0, "{left} {center} {right}");
    assert_eq!(one(r"\cfrac[c]{1}{x+x+x}"), center);
    for (tex, align) in [(r"\cfrac[l]{1}{1+x}", "left"), (r"\cfrac[r]{1}{1+x}", "right")] {
        let r = read(tex);
        assert!(r.mathml.contains(&format!(r#"numalign="{align}""#)), "{}", r.mathml);
        let back = convert(&r.mathml, &Options { input: context_equation_core::Input::MathMl, ..Options::default() }).unwrap();
        assert_eq!(back.tree, r.tree, "{tex}");
    }
    // MathJax's \unicode[font]{…} and \unicode[height,depth][font]{…}: the code point, the font ignored
    for tex in [r"\unicode[Arial]{x22D6}", r"\unicode[1,0.2][Arial]{x22D6}", r"\unicode{0x22D6}"] {
        assert_eq!(read(tex).tree, read(r"\unicode{x22D6}").tree, "{tex}");
    }
    // \centernot: \not's own slash over the middle, not a slash as long as the arrow
    let r = read(r"A \centernot\longrightarrow B");
    assert!(format!("{:?}", r.tree).contains("CenterNot"), "{:?}", r.tree);
    let b = math_box(r"\centernot\longrightarrow");
    let (slash, arrow) = (glyph_middles(&b, '⧸'), glyph_middles(&b, '⟶'));
    assert_eq!(slash.len(), 1);
    assert!((slash[0].0 - arrow[0].0).abs() < 1.0, "⧸ at {}, ⟶ at {}", slash[0].0, arrow[0].0);
    // a negated symbol stays the symbol, as with \not
    assert_eq!(read(r"a \centernot= b").tree, read(r"a \not= b").tree);
}

#[test]
fn a_tree_node_stays_small() {
    // every level of nesting holds nodes on the stack: a bigger node makes the deep-nesting cases
    // overflow the 1 MiB and 2 MiB stacks of the tests (176 bytes when \vdotswithin came)
    let size = std::mem::size_of::<Node>();
    assert!(size <= 176, "Node is {size} bytes");
}

/// #204: MathJax's \unicode reads a bare number as decimal and `x…` or `0x…` as hexadecimal; TeX's \char
/// keeps its own reading (decimal, "hex, 'octal), and the commands that expand to \unicode keep theirs.
#[test]
fn unicode_reads_decimal_as_mathjax() {
    for options in [strict(), Options::default()] {
        for (tex, text) in [
            (r"\unicode{8918}", "⋖"),
            (r"\unicode{x22D6}", "⋖"),
            (r"\unicode{0x22D6}", "⋖"),
            (r"\unicode{X22D6}", "⋖"),
            (r"\unicode{U+22D6}", "⋖"),
            (r"\unicode{65}", "A"),
            (r"\unicode{8918} \quad \unicode{x22D6}", "⋖ ⋖"),
            (r#"\char65 \char"41 \char'101 \char`A"#, "AAAA"),
            (r"\grad f", "𝛁f"),
        ] {
            let out = convert(tex, &options).unwrap_or_else(|e| panic!("{tex}: {e}"));
            assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
            assert_eq!(out.text, text, "{tex}");
        }
    }
    // a decimal code point out of Unicode's range is not a character
    assert!(convert(r"\unicode{99999999}", &strict()).is_err());
}

/// #205: `\not` before a long arrow is TeX's zero-width relation: its slash over the arrow's start, where it
/// would cross an `=`, the arrow its own length and the whole a relation spaced as one.
#[test]
fn not_before_a_long_arrow_slashes_its_start() {
    let equals = glyph_middles(&math_box("="), '=')[0].0;
    for (tex, arrow) in [(r"\not\longrightarrow", '⟶'), (r"\not\Longrightarrow", '⟹'), (r"\not\longleftarrow", '⟵'), (r"\not\longmapsto", '⟼')] {
        let b = math_box(tex);
        let plain = math_box(&tex.replace(r"\not", ""));
        let slash = glyph_middles(&b, '⧸');
        assert_eq!(slash.len(), 1, "{tex}");
        // the middle of an = set where the arrow starts (both boxes start at 0)
        assert!((slash[0].0 - equals).abs() < 1.0, "{tex}: ⧸ at {}, = at {equals}", slash[0].0);
        assert!(slash[0].0 < glyph_middles(&b, arrow)[0].0 - 100.0, "{tex}");
        assert!(!b.items.iter().any(|i| matches!(i, Item::Line { .. })), "{tex}: no slash across the whole arrow");
        assert_eq!(b.width, plain.width, "{tex}: the arrow keeps its length");
        // a relation: spaced as the arrow alone
        let spaced = math_box(&format!("A {tex} B")).width;
        assert_eq!(spaced, math_box(&format!("A {} B", tex.replace(r"\not", ""))).width, "{tex}: spaced as a relation");
    }
    let _ = read(r"A \not\longrightarrow B \quad A \not\Longrightarrow B");
    // a short relation without a negated character, and \cancel, are still crossed out whole
    for tex in [r"\not\approxeq", r"\cancel{\longrightarrow}"] {
        let b = math_box(tex);
        assert!(b.items.iter().any(|i| matches!(i, Item::Line { .. })), "{tex}");
        assert!(glyph_middles(&b, '⧸').is_empty(), "{tex}");
    }
    // \not before a relation with a negated character is that character
    assert_eq!(read(r"a \not= b").text, "a ≠ b");
}
