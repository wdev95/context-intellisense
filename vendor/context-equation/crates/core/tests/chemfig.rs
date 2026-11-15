//! L-CHE-2: structural chemistry. Structures read from chemfig (the graph, and the molecular formula
//! counted from it, C-FOR-1), reaction schemes and mechanisms, what a screen reader says, the
//! recoveries of tolerant mode, and chemistry inside an equation.
#![cfg(feature = "chem")]

use context_equation_core::chem::model::{ArrowKind, Body, BondKind, Item, Label, Molecule, Scheme};
use context_equation_core::tree::Node;
use context_equation_core::{convert, Locale, Options};

fn strict(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options::default()).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    out
}

fn scheme(tex: &str) -> Scheme {
    match strict(tex).tree.single() {
        Node::Chemistry { scheme } => scheme.clone(),
        other => panic!("{tex}: {other:?}"),
    }
}

fn molecule(body: &str) -> Molecule {
    match &scheme(&format!("\\chemfig{{{body}}}")).items[0] {
        Item::Compound(c) => match &c.body {
            Body::Structure(m) => m.clone(),
            other => panic!("{body}: {other:?}"),
        },
        other => panic!("{body}: {other:?}"),
    }
}

fn formula(body: &str) -> String {
    context_equation_core::chem::formula::of(&molecule(body)).written()
}

#[test]
fn c_for_1_known_formulas() {
    for (body, expected) in [
        ("H_3C-CH_2-OH", "C₂H₆O"),
        ("-[:30]-[:-30]-[:30]-[:-30]OH", "C₄H₁₀O"),
        ("*6(=-=-=-)", "C₆H₆"),
        ("**6(------)", "C₆H₆"),
        ("H_3C-*6(=-=-=-)", "C₇H₈"),
        ("*6(-=-*6(-=-=-)=-=)", "C₁₀H₈"),
        ("*6(-=*6(-=-*6(-=-=-)=-)-=-=)", "C₁₄H₁₀"),
        ("*6((=O)-N(-CH_3)-*5(-N=-N(-CH_3)-=)--(=O)-N(-H_3C)-)", "C₈H₁₀N₄O₂"),
        ("*6(-=-(-COOH)=(-O-[::60]C(=[::60]O)-[::-60]CH_3)-=)", "C₉H₈O₄"),
        ("HO-C(=[1]O)-[7]CH_3", "C₂H₄O₂"),
        ("C(-[2]H)(-[4]H)(-[6]H)-H", "CH₄"),
        ("*6((-OH)-O-(-CH_2OH)-(-OH)-(-OH)-(-OH)-)", "C₆H₁₂O₆"),
        ("*5(-N(-H)-=-=)", "C₄H₅N"),
        ("H-C~C-H", "C₂H₂"),
        ("\\lewis{0:2:4:6:,O}", "O"),
        ("NH_4^+", "H₄N⁺"),
        ("Ph-C(=[1]O)-[7]OMe", "C₈H₈O₂"),
    ] {
        assert_eq!(formula(body), expected, "{body}");
    }
    let out = strict(r"\chemfig{H_3C-OH} + \chemfig{*6(=-=-=-)}");
    assert_eq!(context_equation_core::chem::formulas(&out.tree), ["CH₄O", "C₆H₆"]);
}

#[test]
fn fused_rings_share_atoms_and_bonds() {
    let m = molecule("*6(-=-*6(-=-=-)=-=)");
    assert_eq!((m.atoms.len(), m.bonds.len(), m.rings.len()), (10, 11, 2));
    let shared: Vec<_> = m.rings[0].atoms.iter().filter(|a| m.rings[1].atoms.contains(a)).collect();
    assert_eq!(shared.len(), 2);
    let m = molecule("*6((=O)-N(-CH_3)-*5(-N=-N(-CH_3)-=)--(=O)-N(-H_3C)-)");
    assert_eq!(m.rings[1].size, 5);
    assert_eq!(m.atoms.iter().filter(|a| a.label.starts_with('N')).count(), 4);
}

#[test]
fn bonds_kinds_and_angles() {
    let m = molecule("A-B=C~D>E<F>:G<:H>|I<|J-[,,,,draw=none]K-[,,,,dash pattern=on 2pt off 2pt]L");
    let kinds: Vec<BondKind> = m.bonds.iter().map(|b| b.kind).collect();
    use BondKind::*;
    assert_eq!(kinds, [Single, Double, Triple, Wedge, WedgeBack, Hash, HashBack, Hollow, HollowBack, Invisible, Dashed]);
    let m = molecule("A-[1]B-C-[::90]D-[:-30]E-[,2]F");
    let angles: Vec<f32> = m.bonds.iter().map(|b| b.angle).collect();
    assert_eq!(angles, [45.0, 45.0, 135.0, -30.0, -30.0]);
    assert_eq!(m.bonds[4].length, 2.0);
}

#[test]
fn branches_labels_closures_lewis_and_hooks() {
    assert!(molecule("C(-[2]H)(-[6]H)-H").bonds.iter().all(|b| b.from == 0));
    // a lone hexagon starts at its left vertex: the second vertex's branch points down and left
    let m = molecule("*6(-(-X)-----)");
    let out = m.bonds.iter().find(|b| m.atoms[b.to].label == "X").unwrap();
    assert!((out.angle - -120.0).abs() < 1e-3, "{}", out.angle);
    let m = molecule("H_3C-CH_2-OH");
    assert_eq!(m.atoms[0].pieces, ["H_3", "C"]);
    assert_eq!((m.bonds[0].departure, m.atoms[1].anchor), (1, 0));
    assert_eq!(molecule("C-[4]H_3C").atoms[1].anchor, 1);
    let m = molecule("A?-B-[2]C-[4]D?");
    assert_eq!((m.bonds.len(), m.bonds[3].from, m.bonds[3].to), (4, 0, 3));
    // chemfig manual, section 2.8: the link is the later mark's; the first mark gives only the name
    assert_eq!(molecule("A?[x]-B-[2]C?[x,{=}]").bonds[2].kind, BondKind::Double);
    assert_eq!(molecule("A?[x,{=}]-B-[2]C?[x]").bonds[2].kind, BondKind::Single);
    let m = molecule("@{nu}\\lewis{0:2.4|6o,O}-H");
    assert_eq!((m.atoms[0].lewis.len(), m.atoms[0].hook.as_deref(), m.atoms[0].label.as_str()), (4, Some("nu"), "O"));
}

const NITRATION: &str = r"\schemestart \chemfig{*6(=-=-=-)} \+ \ce{HNO3} \arrow{->[\ce{H2SO4}][$\Delta$]} \chemfig{*6(=-=(-NO_2)-=-)} \+ \ce{H2O} \schemestop";

#[test]
fn schemes_and_arrows() {
    let s = scheme(NITRATION);
    let kinds: Vec<&str> = s
        .items
        .iter()
        .map(|i| match i {
            Item::Compound(c) if matches!(c.body, Body::Structure(_)) => "structure",
            Item::Compound(_) => "math",
            Item::Plus => "+",
            Item::Arrow(_) => "arrow",
            Item::Merge(_) => "merge",
        })
        .collect();
    assert_eq!(kinds, ["structure", "+", "math", "arrow", "structure", "+", "math"]);
    let Item::Arrow(a) = &s.items[3] else { panic!() };
    assert_eq!(a.above, Some(Label::Text { tex: r"\ce{H2SO4}".into() }));
    let kind = |t: &str| match &scheme(&format!(r"\schemestart A \arrow{{{t}}} B \schemestop")).items[1] {
        Item::Arrow(a) => a.kind,
        other => panic!("{other:?}"),
    };
    use ArrowKind::*;
    for (t, k) in [
        ("->", Forward),
        ("<-", Backward),
        ("<=>", Equilibrium),
        ("<->>", EquilibriumRight),
        ("<<->", EquilibriumLeft),
        ("<->", Resonance),
        ("=>", Retro),
        ("-/>", NoReaction),
        ("0", Invisible),
    ] {
        assert_eq!(kind(t), k, "{t}");
    }
    let s = scheme(r"\schemestart A \arrow{->[\chemfig{LiAlH_4}][then H$_2$O]}[-90,1.5] B \schemestop");
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert!(matches!(a.above, Some(Label::Structure(_))));
    assert_eq!((a.angle, a.length), (-90.0, 1.5));
}

#[test]
fn speech_text_and_braille() {
    let out = strict(NITRATION);
    assert_eq!(
        out.speech,
        "structure of C6 H6, with 1 aromatic ring plus H N O sub 3 reacts to give (H sub 2 S O sub 4 over the arrow; capital delta under the arrow) structure of C6 H5 N O2, with 1 aromatic ring plus H sub 2 O"
    );
    assert!(out.text.starts_with("C₆H₆ + ") && out.text.contains("→["), "{}", out.text);
    assert!(!out.braille.is_empty());
    let pt = |tex: &str| convert(tex, &Options { locale: Locale::PtBr, strict: true, ..Default::default() }).unwrap();
    assert!(pt(r"\schemestart \ce{N2 + 3H2} \arrow{<=>[Fe][450 °C]} \ce{2NH3} \schemestop").speech.contains("em equilíbrio com (Fe sobre a seta; 450 °C sob a seta)"));
    assert!(pt(r"\chemfig{*6(=-=-=-)}").speech.starts_with("estrutura de C6 H6, com 1 anel aromático"));
    assert!(strict(r"\chemfig{NH_4^+}").speech.contains("charge +1"));
    assert!(strict(r"\chemname{\chemfig{H_3C-OH}}{methanol}").speech.starts_with("methanol, structure of C H4 O"));
    assert!(strict(r"\schemestart A \arrow{<->} B \schemestop").speech.contains("in resonance with"));
    assert_eq!(strict(r"\schemestart A + B \arrow C \schemestop").text, "A + B → C");
}

#[test]
fn mechanism_arrows() {
    let src = r"\schemestart \chemfig{@{nu}\lewis{0:2:4:6:,HO}^{-}} \+ \chemfig{H_3C-@{c}C(-[2]H)(-[6]H)-@{cl}Cl} \arrow \chemfig{HO-CH_3} \schemestop \chemmove{\draw[->](nu)..controls +(60:1.2cm) and +(120:1.2cm)..(c); \draw[-left to](c) to[bend left] (cl);}";
    let out = strict(src);
    let Node::Chemistry { scheme } = out.tree.single() else { panic!() };
    assert_eq!(scheme.moves.len(), 2);
    let m = &scheme.moves[0];
    assert_eq!((m.from.as_str(), m.to.as_str(), m.out_angle, m.in_angle), ("nu", "c", Some(60.0), Some(120.0)));
    assert!((m.reach.unwrap().0 - 1.2 * 2.845).abs() < 1e-3);
    assert!(scheme.moves[1].single);
    assert!(out.speech.contains("curved arrow: an electron pair moves from HO^− to C"), "{}", out.speech);
    assert!(out.speech.contains("one electron moves from C to Cl"), "{}", out.speech);
}

#[test]
fn chemistry_inside_an_equation() {
    let out = strict(r"\Delta G = -RT \ln K, \quad \chemfig{*6(=-=-=-)} \rightleftharpoons \frac{1}{2} \chemfig{H_2C=CH_2}");
    assert!(out.speech.contains("structure of C6 H6") && out.speech.contains("structure of C2 H4"), "{}", out.speech);
    assert!(out.applied.iter().any(|a| a.rule == "L-CHE-2"));
    // a structure in a subscript is drawn smaller
    let full = context_equation_core::typeset(r"\chemfig{*6(------)}", &Options::default()).unwrap().0;
    let small = context_equation_core::typeset(r"x_{\chemfig{*6(------)}}", &Options::default()).unwrap().0;
    assert!(small.width < full.width, "{} {}", small.width, full.width);
}

#[test]
fn recoveries() {
    let warns = |tex: &str, rule: &str| {
        let out = convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert!(out.warnings.iter().any(|w| w.rule == rule), "{tex}: expected {rule}, got {:?}", out.warnings);
        assert!(convert(tex, &Options { strict: true, ..Default::default() }).is_err(), "{tex}: strict mode accepted it");
    };
    warns(r"\chemfig{A-(-B}", "E-REC-4");
    warns(r"\chemfig{A)-B}", "E-REC-9");
    warns(r"\chemfig{*6(-=-}", "E-REC-4");
    warns(r"\chemfig{*6(-------)}", "E-REC-9");
    warns(r"\chemfig{*99(-)}", "E-REC-9");
    warns(r"\chemfig{*6}", "E-REC-4");
    warns(r"\schemestart A \arrow{~>} B \schemestop", "E-REC-1");
    warns(r"\schemestart A \arrow B \schemestop \chemmove{\draw[->](x)..(y);}", "E-REC-9");
    warns(r"\schemestart A \schemestop \chemmove{\node at (0,0) {x};}", "E-REC-1");
    warns(r"\schemestart A \arrow B", "E-REC-5");
    warns(r"\chemfig{A-\unknowncommand}", "E-REC-1");
    for tex in [r"\chemfig{(((}", r"\chemfig{-=~<>}", r"\chemfig{*}", r"\chemfig{?}", r"\chemfig{@}", r"\chemfig{}", r"\schemestart \arrow \arrow \schemestop"] {
        let out = convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert!(out.svg.starts_with("<svg"), "{tex}");
    }
}

/// #210: chemfig's offset double bonds (`=^` to the left, `=_` to the right) are double bonds, not a
/// script on the next atom: the manual's `*6(-=_-=_-=_)` panicked counting a `)` the subscript took.
#[test]
fn offset_double_bonds() {
    let m = molecule("*6(-=_-=_-=_)");
    assert_eq!(m.bonds.iter().filter(|b| b.kind == BondKind::DoubleRight).count(), 3);
    assert_eq!(m.atoms.len(), 6);
    assert_eq!(strict(r"\chemfig{*6(-=_-=_-=_)}").text, "C₆H₆");
    let m = molecule("A-=^-B");
    assert_eq!(m.bonds.iter().map(|b| b.kind).collect::<Vec<_>>(), [BondKind::Single, BondKind::DoubleLeft, BondKind::Single]);
    // drawn: the offset line goes outside the ring, so the drawing differs from the plain ring's
    assert_ne!(strict(r"\chemfig{*6(-=_-=_-=_)}").svg, strict(r"\chemfig{*6(-=-=-=)}").svg);
    // still a Kekulé benzene for the speech
    assert_eq!(strict(r"\chemfig{*6(-=_-=_-=_)}").speech, strict(r"\chemfig{*6(-=-=-=)}").speech);
}

/// #210: no structure panics, however malformed its rings, branches and scripts: tolerant mode draws it
/// with warnings and strict mode draws it or refuses it with an error. Every string of up to four
/// characters from the structure's alphabet, inside a ring and alone, plus a fixed pseudo-random set of
/// longer ones.
#[test]
fn malformed_structures_never_panic() {
    const ALPHABET: [char; 12] = ['(', ')', '*', '6', '-', '=', '_', '^', 'A', '[', '{', '?'];
    let mut bodies: Vec<String> = vec![String::new()];
    let mut layer: Vec<String> = vec![String::new()];
    for _ in 0..4 {
        layer = layer.iter().flat_map(|s| ALPHABET.iter().map(move |c| format!("{s}{c}"))).collect();
        bodies.extend(layer.iter().cloned());
    }
    // a linear congruential generator: the same longer strings on every run
    let mut seed: u64 = 0x2010;
    for _ in 0..3000 {
        let len = 5 + (seed >> 33) as usize % 12;
        let mut s = String::new();
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            s.push(ALPHABET[(seed >> 33) as usize % ALPHABET.len()]);
        }
        bodies.push(s);
    }
    for body in &bodies {
        for tex in [format!(r"\chemfig{{{body}}}"), format!(r"\chemfig{{*6({body})}}")] {
            for strict in [false, true] {
                let options = Options { strict, ..Default::default() };
                let result = std::panic::catch_unwind(|| convert(&tex, &options));
                assert!(result.is_ok(), "{tex} (strict: {strict}) panicked");
            }
        }
    }
}
