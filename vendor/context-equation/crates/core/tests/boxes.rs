//! The boxes of marked groups (`Output::boxes`): what an editor reads to place its cursor. Marking a
//! piece must not move anything, and each box must sit where its content is drawn.

use context_equation_core::layout::{Item, MathBox};
use context_equation_core::output::boxes::GroupBox;
use context_equation_core::Options;

fn options() -> Options {
    Options { fallback_fonts: Vec::new(), ..Options::default() }
}

fn boxes(tex: &str) -> Vec<GroupBox> {
    context_equation_core::convert(tex, &options()).unwrap_or_else(|e| panic!("{tex}: {e}")).boxes
}

/// The box marked `m=<key>`.
fn marked<'a>(list: &'a [GroupBox], key: &str) -> &'a GroupBox {
    list.iter().find(|b| b.attributes.iter().any(|(k, v)| k == "data-m" && v == key)).unwrap_or_else(|| panic!("no box m={key} in {list:?}"))
}

/// Everything drawn, groups opened, as text rounded to a hundredth of a unit: two layouts that print
/// the same draw the same.
fn drawing(b: &MathBox) -> Vec<String> {
    fn walk(items: &[Item], out: &mut Vec<String>) {
        let r = |v: f32| (v * 100.0).round() / 100.0;
        for item in items {
            match item {
                Item::Glyph { id, x, y, scale, .. } => out.push(format!("g{} {} {} {}", id.0, r(*x), r(*y), r(*scale))),
                Item::Rule { x, y, width, height, .. } => out.push(format!("r {} {} {} {}", r(*x), r(*y), r(*width), r(*height))),
                Item::Line { x1, y1, x2, y2, .. } => out.push(format!("l {} {} {} {}", r(*x1), r(*y1), r(*x2), r(*y2))),
                Item::Ellipse { cx, cy, rx, ry, .. } => out.push(format!("e {} {} {} {}", r(*cx), r(*cy), r(*rx), r(*ry))),
                Item::Transformed { matrix, items } => {
                    out.push(format!("t {:?}", matrix.map(r)));
                    walk(items, out);
                }
                Item::Group { items, .. } => walk(items, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(&b.items, &mut out);
    out
}

fn typeset(tex: &str) -> MathBox {
    context_equation_core::typeset(tex, &options()).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

/// Marking every atom and every slot, as an editor does, draws exactly what the bare formula draws.
/// A marked operator keeps its class only through `\mathbin`, `\mathrel`…: `\htmlData{…}{=}`, like
/// `{=}`, is an ordinary atom.
#[test]
fn marking_moves_nothing() {
    let pairs = [
        (
            r"x = \frac{-b \pm \sqrt{b^2-4ac}}{2a}",
            r"\htmlData{m=1}{x}\mathrel{\htmlData{m=2}{=}}\htmlData{m=3}{\frac{\htmlData{m=4}{\mathbin{\htmlData{m=5}{-}}\htmlData{m=6}{b}\mathbin{\htmlData{m=7}{\pm}}\htmlData{m=8}{\sqrt{\htmlData{m=9}{\htmlData{m=10}{b}^{\htmlData{m=11}{2}}\mathbin{\htmlData{m=12}{-}}\htmlData{m=13}{4}\htmlData{m=14}{a}\htmlData{m=15}{c}}}}}}{\htmlData{m=16}{\htmlData{m=17}{2}\htmlData{m=18}{a}}}}",
        ),
        (r"\sum_{n=1}^{\infty} a_n", r"\htmlData{m=1}{\sum_{\htmlData{m=2}{n=1}}^{\htmlData{m=3}{\infty}}}\htmlData{m=4}{a}_{\htmlData{m=5}{n}}"),
        (
            r"\int_0^1 f(x)\,dx",
            r"\htmlData{m=1}{\int_{\htmlData{m=2}{0}}^{\htmlData{m=3}{1}}}\htmlData{m=4}{f}\mathopen{\htmlData{m=5}{(}}\htmlData{m=6}{x}\mathclose{\htmlData{m=7}{)}}\,\htmlData{m=8}{d}\htmlData{m=9}{x}",
        ),
        (r"\left(\frac{a}{b}\right)^2", r"\htmlData{m=1}{\left(\htmlData{m=2}{\frac{\htmlData{m=3}{a}}{\htmlData{m=4}{b}}}\right)}^{\htmlData{m=5}{2}}"),
        (r"a + b \le c", r"\htmlData{m=1}{a}\mathbin{\htmlData{m=2}{+}}\htmlData{m=3}{b}\mathrel{\htmlData{m=4}{\le}}\htmlData{m=5}{c}"),
        (r"\sqrt[3]{x}", r"\htmlData{m=1}{\sqrt[\htmlData{m=2}{3}]{\htmlData{m=3}{x}}}"),
        (
            r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
            r"\htmlData{m=1}{\begin{pmatrix} \htmlData{m=2}{a} & \htmlData{m=3}{b} \\ \htmlData{m=4}{c} & \htmlData{m=5}{d} \end{pmatrix}}",
        ),
        (r"\lim_{x \to 0} \frac{\sin x}{x}", r"\htmlData{m=1}{\lim_{\htmlData{m=2}{x \to 0}}}\htmlData{m=3}{\frac{\htmlData{m=4}{\sin x}}{\htmlData{m=5}{x}}}"),
    ];
    for (bare, marked) in pairs {
        let (a, b) = (typeset(bare), typeset(marked));
        assert_eq!(drawing(&a), drawing(&b), "marking moved something in {bare}");
        assert_eq!((a.width, a.height, a.depth), (b.width, b.height, b.depth), "marking changed the size of {bare}");
    }
}

#[test]
fn a_formula_without_marks_has_no_boxes() {
    assert!(boxes(r"\frac{a}{b}").is_empty());
}

/// One glyph: its box starts at the origin, its top is its height above the baseline (y down).
#[test]
fn a_single_box_is_in_svg_coordinates() {
    let b = typeset("x");
    let list = boxes(r"\htmlData{m=x}{x}");
    let x = marked(&list, "x");
    assert_eq!((x.x, x.baseline, x.level), (0.0, 0.0, 0));
    assert!((x.y + b.height).abs() < 0.01, "top {} for height {}", x.y, b.height);
    assert!((x.height - b.height - b.depth).abs() < 0.01);
    // the formula's width also counts the last glyph's italic correction; the box is the glyph's own
    assert!(x.width > 0.0 && x.width <= b.width + 0.01, "width {} in {}", x.width, b.width);
}

#[test]
fn numerator_above_denominator_inside_the_fraction() {
    let list = boxes(r"\htmlData{m=f}{\frac{\htmlData{m=n}{a+b}}{\htmlData{m=d}{c}}}");
    let (f, n, d) = (marked(&list, "f"), marked(&list, "n"), marked(&list, "d"));
    assert!(n.y + n.height <= d.y, "numerator {n:?} not above denominator {d:?}");
    for inner in [n, d] {
        assert!(inner.x >= f.x - 0.01 && inner.x + inner.width <= f.x + f.width + 0.01, "{inner:?} outside {f:?}");
        assert!(inner.y >= f.y - 0.01 && inner.y + inner.height <= f.y + f.height + 0.01, "{inner:?} outside {f:?}");
        assert_eq!(inner.level, 1);
    }
    assert!(n.baseline < 0.0 && d.baseline > 0.0, "baselines {} and {}", n.baseline, d.baseline);
}

/// Atoms of a row: left to right, without overlap, the gap between two at most an italic correction
/// (well under a tenth of an em), on one baseline.
#[test]
fn atoms_of_a_row_follow_each_other() {
    let list = boxes(r"\htmlData{m=a}{a}\htmlData{m=b}{b}\htmlData{m=c}{c}");
    let (a, b, c) = (marked(&list, "a"), marked(&list, "b"), marked(&list, "c"));
    for (l, r) in [(a, b), (b, c)] {
        let gap = r.x - (l.x + l.width);
        assert!((-0.01..100.0).contains(&gap), "gap {gap} between {l:?} and {r:?}");
    }
    assert!(a.baseline == b.baseline && b.baseline == c.baseline);
}

#[test]
fn an_exponent_sits_above_and_smaller() {
    let list = boxes(r"\htmlData{m=x}{x}^{\htmlData{m=e}{2}}");
    let (x, e) = (marked(&list, "x"), marked(&list, "e"));
    assert!(e.x >= x.x + x.width - 0.01, "exponent {e:?} not right of {x:?}");
    assert!(e.baseline < x.baseline, "exponent baseline {} not above {}", e.baseline, x.baseline);
    assert!(e.height < x.height);
}

/// `\scalebox` scales the box and a `\reflectbox` keeps it inside the mirrored width.
#[test]
fn boxes_follow_scaling_and_mirroring() {
    let one = marked(&boxes(r"\htmlData{m=x}{x}"), "x").clone();
    let two = marked(&boxes(r"\scalebox{2}{$\htmlData{m=x}{x}$}"), "x").clone();
    assert!((two.width - 2.0 * one.width).abs() < 0.01 && (two.height - 2.0 * one.height).abs() < 0.01);
    let list = boxes(r"\reflectbox{$\htmlData{m=a}{a}\htmlData{m=b}{bb}$}");
    let (a, b) = (marked(&list, "a"), marked(&list, "b"));
    assert!(b.x + b.width <= a.x + 0.01, "mirrored: {b:?} should be left of {a:?}");
}

/// A turned group's box covers its turned corners.
#[test]
fn a_turned_box_covers_the_turn() {
    let flat = marked(&boxes(r"\htmlData{m=x}{xyz}"), "x").clone();
    let turned = marked(&boxes(r"\rotatebox{90}{$\htmlData{m=x}{xyz}$}"), "x").clone();
    assert!((turned.height - flat.width).abs() < 0.5, "{turned:?} from {flat:?}");
}

#[test]
fn the_json_has_the_boxes() {
    let json: serde_json::Value = serde_json::from_str(&context_equation_core::convert(r"\htmlData{m=1}{x}", &options()).unwrap().to_json()).unwrap();
    let b = &json["boxes"][0];
    assert_eq!(b["attributes"]["data-m"], "1");
    assert_eq!(b["level"], 0);
    for k in ["x", "y", "width", "height", "baseline"] {
        assert!(b[k].is_number(), "{k}");
    }
}

/// mhchem: marks around its pieces (a species, a coefficient, a formula, a state, an arrow) are read
/// through, so an editor can edit a reaction piece by piece; the drawing is the one without them.
#[test]
fn marks_inside_ce_move_nothing() {
    let pairs = [
        (r"\ce{2H2 + O2 -> 2H2O}", r"\ce{\htmlData{m=a}{2H2} \htmlData{m=p}{+} \htmlData{m=b}{O2} \htmlData{m=r}{->} \htmlData{m=c}{2H2O}}"),
        (
            r"\ce{C3H8(g) + 5 O2(g) -> 3 CO2(g) + 4 H2O(l)}",
            r"\ce{\htmlData{m=a}{\htmlData{m=f}{C3H8}\htmlData{m=s}{(g)}} + \htmlData{m=b}{\htmlData{m=k}{5} \htmlData{m=g}{O2}\htmlData{m=t}{(g)}} -> \htmlData{m=c}{\htmlData{m=l}{3} CO2(g)} + 4 H2O(l)}",
        ),
        (
            r"\ce{6 CO2 + 6 H2O ->[\text{light}][\text{chlorophyll}] C6H12O6 + 6 O2}",
            r"\ce{\htmlData{m=a}{6 CO2} + \htmlData{m=b}{6 H2O} \htmlData{m=r}{->[\text{light}][\text{chlorophyll}]} \htmlData{m=c}{C6H12O6} + \htmlData{m=d}{6 O2}}",
        ),
        (r"\ce{2 NO2(g) <=> N2O4(g)}", r"\ce{\htmlData{m=a}{2 NO2(g)} \htmlData{m=r}{<=>} \htmlData{m=b}{N2O4(g)}}"),
        (
            r"\ce{Zn(s) | Zn^2+(aq) || Cu^2+(aq)}",
            r"\ce{\htmlData{m=a}{\htmlData{m=b}{Zn}\htmlData{m=c}{(s)}} \htmlData{m=d}{\htmlData{m=e}{|}} \htmlData{m=f}{Zn^2+(aq)} \htmlData{m=g}{\htmlData{m=h}{||}} Cu^2+(aq)}",
        ),
        (r"\ce{Fe^3+ + e- -> Fe^2+}", r"\ce{\htmlData{m=a}{Fe^3+} + \htmlData{m=b}{e-} -> \htmlData{m=c}{Fe^2+}}"),
        (r"\ce{CaCO3(s) ->[\Delta] CaO(s) + CO2 ^}", r"\ce{\htmlData{m=a}{CaCO3(s)} \htmlData{m=r}{->[\Delta]} \htmlData{m=b}{CaO(s)} + \htmlData{m=c}{CO2} \htmlData{m=g}{^}}"),
    ];
    for (bare, marked) in pairs {
        let (a, b) = (typeset(bare), typeset(marked));
        assert_eq!(drawing(&a), drawing(&b), "marks moved something in {bare}");
        assert!(boxes(marked).len() >= 3, "marks give boxes in {marked}");
        assert!(context_equation_core::convert(marked, &options()).unwrap().warnings.is_empty(), "warnings in {marked}");
    }
}

/// chemfig: a mark around an atom's label is kept on the atom, the label read through it (bonds still
/// reach the same letter), and the drawing is the one without the mark.
#[test]
fn marks_on_chemfig_atoms_move_nothing() {
    let pairs = [
        (
            r"\chemfig{H-C(-[2]H)(-[6]H)-C(-[2]H)(-[6]H)-O-H}",
            r"\chemfig{\htmlData{m=a}{H}-\htmlData{m=b}{C}(-[2]\htmlData{m=c}{H})(-[6]\htmlData{m=d}{H})-\htmlData{m=e}{C}(-[2]H)(-[6]H)-\htmlData{m=f}{O}-\htmlData{m=g}{H}}",
        ),
        (r"\chemfig{*6(-=-=(-OH)-=)}", r"\chemfig{*6(-=-=(-\htmlData{m=x}{OH})-=)}"),
        (r"\chemfig{H_3C-C(=[1]O)-[7]OH}", r"\chemfig{\htmlData{m=a}{H_3C}-\htmlData{m=b}{C}(=[1]\htmlData{m=c}{O})-[7]\htmlData{m=d}{OH}}"),
        (r"\chemfig{\charge{90=\|,0=$\oplus$}{N}-H}", r"\chemfig{\htmlData{m=a}{\charge{90=\|,0=$\oplus$}{N}}-\htmlData{m=b}{H}}"),
        (r"\chemfig{\lewis{0:2:,O}=C=\lewis{4:6:,O}}", r"\chemfig{\htmlData{m=a}{\lewis{0:2:,O}}=\htmlData{m=c}{C}=\lewis{4:6:,O}}"),
        (r"\chemfig{H_3C-NH_2^+}", r"\chemfig{\htmlData{m=a}{H_3C}-\htmlData{m=b}{NH_2^+}}"),
        (r"\chemfig{*6(-=-=-=)}", r"\chemfig{\htmlData{m=v0}{}*6(-\htmlData{m=v1}{}=\htmlData{m=v2}{}-=-=)}"),
        (r"\chemfig{-[:30]-[:-30]OH}", r"\chemfig{\htmlData{m=v}{}-[:30]\htmlData{m=w}{}-[:-30]OH}"),
        (
            r"\schemestart \chemfig{A-B} \arrow{->[heat][$\Delta$, cat]} \chemfig{C=D} \schemestop",
            r"\schemestart \chemfig{\htmlData{m=a}{A}-B} \arrow{->[\htmlData{m=u}{heat}][\htmlData{m=w}{$\Delta$, cat}]} \chemfig{C=D} \schemestop",
        ),
        (r"\schemestart \chemfig{A-B} \arrow \chemfig{C=D} \schemestop", r"\schemestart \chemfig{\htmlData{m=a}{A}-B} \arrow \chemfig{C=\htmlData{m=d}{D}} \schemestop"),
    ];
    for (bare, marked) in pairs {
        let (a, b) = (typeset(bare), typeset(marked));
        assert_eq!(drawing(&a), drawing(&b), "marks moved something in {bare}");
        assert!(!boxes(marked).is_empty(), "marks give boxes in {marked}");
        assert!(context_equation_core::convert(marked, &options()).unwrap().warnings.is_empty(), "warnings in {marked}");
    }
}

/// chemfig: each atom and bond says where it is written (characters of the structure's code), so an
/// editor can change one in place.
#[test]
fn chemfig_atoms_and_bonds_know_where_they_are_written() {
    let body = "H_3C-C(=[1]O)-[7] OH";
    let tree = serde_json::to_value(context_equation_core::convert(&format!("\\chemfig{{{body}}}"), &options()).unwrap().tree).unwrap();
    let structure = &tree["children"][0]["scheme"]["items"][0]["body"];
    let chars: Vec<char> = body.chars().collect();
    let text = |v: &serde_json::Value| -> String {
        let (a, b) = (v[0].as_u64().unwrap() as usize, v[1].as_u64().unwrap() as usize);
        chars[a..b].iter().collect()
    };
    let atoms: Vec<String> = structure["atoms"].as_array().unwrap().iter().map(|a| text(&a["span"])).collect();
    assert_eq!(atoms, ["H_3C", "C", "O", "OH"]);
    let bonds: Vec<String> = structure["bonds"].as_array().unwrap().iter().map(|b| text(&b["span"])).collect();
    assert_eq!(bonds, ["-", "=[1]", "-[7]"]);
}

/// The corners of marks in chemistry: spaces inside a mark, a coefficient before one, what is dropped
/// as unsafe, a mark command without its arguments, nested braces and a label cut short.
#[test]
fn marks_in_chemistry_at_their_corners() {
    let same = [
        (r"\ce{H2O}", r"\ce{\htmlData{m=a}{ H2O}}"),
        (r"\ce{n H2O}", r"\ce{n \htmlData{m=a}{H2O}}"),
        (r"\ce{H2O CO2}", r"\ce{\htmlData{m=a}{H2O} \htmlData{m=b}{CO2}}"),
        (r"\chemfig{O-H}", r"\chemfig{\class{<>}{O}-H}"),
        (r"\schemestart \chemfig{A}\arrow{->[x{y}]}\chemfig{B}\schemestop", r"\schemestart \chemfig{A}\arrow{->[\htmlData{m=a}{x{y}}]}\chemfig{B}\schemestop"),
    ];
    for (bare, marked) in same {
        assert_eq!(drawing(&typeset(bare)), drawing(&typeset(marked)), "marks moved something in {bare}");
    }
    // a class with nothing safe in it is dropped, with a warning, and nothing is marked
    let out = context_equation_core::convert(r"\ce{\class{<>}{H2O}}", &options()).unwrap();
    assert!(out.boxes.is_empty() && !out.warnings.is_empty());
    assert!(boxes(r"\chemfig{\class{<>}{O}-H}").is_empty());
    // a mark without its arguments is not a mark: read as anything else, without a panic
    for tex in [
        r"\ce{\htmlData x}",
        r"\ce{\htmlData{m=a} H2O}",
        r"\schemestart \chemfig{A}\arrow{->[\htmlData x]}\chemfig{B}\schemestop",
        r"\schemestart \chemfig{A}\arrow{->[\htmlData{m=a}]}\chemfig{B}\schemestop",
        r"\schemestart \chemfig{A}\arrow{->[\htmlData{m=a}{x]y}]}\chemfig{B}\schemestop",
    ] {
        let _ = context_equation_core::convert(tex, &options());
    }
}

/// A label's span leaves out the spaces around it.
#[test]
fn chemfig_spans_leave_spaces_out() {
    let body = "A -B";
    let tree = serde_json::to_value(context_equation_core::convert(&format!("\\chemfig{{{body}}}"), &options()).unwrap().tree).unwrap();
    let atoms = &tree["children"][0]["scheme"]["items"][0]["body"]["atoms"];
    assert_eq!(atoms[0]["span"], serde_json::json!([0, 1]));
}
