//! Issue #214: chemfig's default bond values at the start of a structure, `\charge`, `-U>`,
//! `\chemleft`/`\chemright`, `\chemup`/`\chemdown`, `\chemskipalign`, distant bonds whose mark sits
//! inside an atom group, and (#257) `\merge`, with the compound names it joins and the branching arrows
//! that use them. Section numbers are the chemfig manual's (v1.83).
#![cfg(feature = "chem")]

use context_equation_core::chem::model::{ArrowKind, Body, BondKind, ChargeContent, Item, LewisMark, Merge, Molecule, Scheme, UArc};
use context_equation_core::tree::Node;
use context_equation_core::{convert, Input, Options};

fn tolerant(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// Read in strict mode, and with no warning in tolerant mode.
fn strict(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    assert!(tolerant(tex).warnings.is_empty(), "{tex}: {:?}", tolerant(tex).warnings);
    out
}

fn scheme_of(node: &Node) -> Scheme {
    match node {
        Node::Chemistry { scheme } => scheme.clone(),
        Node::Row { children } => children.iter().find_map(|c| matches!(c, Node::Chemistry { .. }).then(|| scheme_of(c))).expect("a scheme"),
        other => panic!("{other:?}"),
    }
}

fn molecule(body: &str) -> Molecule {
    match &scheme_of(strict(&format!("\\chemfig{{{body}}}")).tree.single()).items[0] {
        Item::Compound(c) => match &c.body {
            Body::Structure(m) => m.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

/// Tolerant mode warns with `rule`; strict mode refuses.
fn warns(tex: &str, rule: &str) -> context_equation_core::Output {
    let out = tolerant(tex);
    assert!(out.warnings.iter().any(|w| w.rule == rule), "{tex}: expected {rule}, got {:?}", out.warnings);
    assert!(convert(tex, &Options { strict: true, ..Default::default() }).is_err(), "{tex}: strict mode accepted it");
    out
}

#[test]
fn defaults_at_the_start_of_a_structure() {
    // section 2.6: every bond 20° and 1.5 times as long, unless it says otherwise
    let m = molecule("[:20,1.5]A-B-C-[:-80,0.7]D-E-F");
    let angles: Vec<f32> = m.bonds.iter().map(|b| b.angle).collect();
    let lengths: Vec<f32> = m.bonds.iter().map(|b| b.length).collect();
    assert_eq!(angles, [20.0, 20.0, -80.0, 20.0, 20.0]);
    assert_eq!(lengths, [1.5, 1.5, 0.7, 1.5, 1.5]);
    let m = molecule("[:20,1.5]A-B");
    assert_eq!((m.bonds[0].angle, m.bonds[0].length), (20.0, 1.5));
    // the issue's other form: an ordinary bond argument, as before
    let m = molecule("A-[:20,1.5]B");
    assert_eq!((m.bonds[0].angle, m.bonds[0].length), (20.0, 1.5));
    // all five fields: departure and arrival on the second atom of each group; a predefined angle
    let m = molecule("[:30,1.5,2,2,red,thick]ABC-DEF=GHI");
    assert!(m.bonds.iter().all(|b| (b.angle, b.departure, b.arrival) == (30.0, 1, 1)), "{:?}", m.bonds);
    assert_eq!(molecule("[1]A-B").bonds[0].angle, 45.0);
    // the TikZ field applies to every bond: draw=none hides them
    assert!(molecule("[,,,,draw=none]A-B-C").bonds.iter().all(|b| b.kind == BondKind::Invisible));
    // a relative angle still counts from the previous bond
    let m = molecule("[:10]A-[::30]B-[::30]C-D");
    assert_eq!(m.bonds.iter().map(|b| b.angle).collect::<Vec<_>>(), [40.0, 70.0, 10.0]);
    // a branch takes the structure's default too
    let m = molecule("[:90]A-B(-C)-D");
    assert!(m.bonds.iter().all(|b| b.angle == 90.0), "{:?}", m.bonds);
}

#[test]
fn charges_around_an_atom() {
    // section 5.8: \charge{position=charge,…}{atom}, the Lewis shortcuts and text
    let m = molecule(r"R-\charge{90=\|,-90=\|}{O}-H");
    assert_eq!(m.atoms[1].label, "O");
    let marks: Vec<_> = m.atoms[1].charges.iter().map(|c| (c.angle, c.content.clone())).collect();
    assert_eq!(marks, [(90.0, ChargeContent::Mark { mark: LewisMark::Line }), (-90.0, ChargeContent::Mark { mark: LewisMark::Line })]);
    let m = molecule(r#"\charge{30=\:,120=$\ominus$,210:5pt=$\delta^+$,north east=\.,0=\"}{Fe}"#);
    let c = &m.atoms[0].charges;
    assert_eq!(c.len(), 5);
    assert_eq!(c[0].content, ChargeContent::Mark { mark: LewisMark::Pair });
    assert_eq!(c[1].content, ChargeContent::Text { tex: r"$\ominus$".into() });
    assert_eq!((c[2].angle, c[2].offset), (-150.0, 0.5));
    assert_eq!((c[3].angle, &c[3].content), (45.0, &ChargeContent::Mark { mark: LewisMark::Radical }));
    assert_eq!(c[4].content, ChargeContent::Mark { mark: LewisMark::Vacancy });
    // the piece it is around: N in NH_3, the second piece after H_3
    assert_eq!(molecule(r"\charge{90=\.}{N}H_3").atoms[0].charges[0].piece, 0);
    assert_eq!(molecule(r"H_3\charge{90=\.}{N}").atoms[0].charges[0].piece, 1);
    // the anchor option and extra sep
    let m = molecule(r"\charge{[extra sep=0pt]45[anchor=180+\chargeangle]=$\scriptstyle\oplus$}{A}-[2]B");
    let c = &m.atoms[0].charges[0];
    assert_eq!(c.anchor, Some(-135.0));
    assert!((c.offset + 0.15).abs() < 1e-6, "{}", c.offset);
    // on an empty vertex of a ring
    let m = molecule(r"*5(---\charge{90:2pt=$\scriptstyle\oplus$}{}-\charge{135:2pt=$\scriptstyle-$}{}-)");
    assert_eq!(m.atoms.iter().map(|a| a.charges.len()).sum::<usize>(), 2);
    // a formal charge counts in the formula; a partial one does not
    assert_eq!(context_equation_core::chem::formula::of(&molecule(r"\charge{90=$\oplus$}{N}H_4")).written(), "H₄N⁺");
    assert_eq!(context_equation_core::chem::formula::of(&molecule(r"\charge{90=$\delta^+$}{C}")).written(), "C");
    // outside \chemfig, around one atom
    let out = strict(r"\charge{90=\|}{X}");
    let s = scheme_of(out.tree.single());
    assert!(matches!(&s.items[0], Item::Compound(c) if matches!(&c.body, Body::Structure(m) if m.atoms[0].charges.len() == 1)));
    assert_ne!(out.svg, strict(r"\chemfig{X}").svg);
    // an unknown position is left out with a warning
    warns(r"\chemfig{\charge{somewhere=\.}{X}}", "E-REC-9");
}

#[test]
fn u_arrow() {
    // sections 6.2 and 6.9: a forward arrow with an arc; radius 0.333 of the length, 60° each way
    let s = scheme_of(strict(r"\schemestart A\arrow{-U>[$x$][$y$]}B\schemestop").tree.single());
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!((a.kind, a.arc), (ArrowKind::Forward, Some(UArc { radius: 0.333, half_angle: 60.0 })));
    assert!(a.above.is_some() && a.below.is_some());
    let s = scheme_of(strict(r"\schemestart A\arrow{-U>[123][456][][-0.25][-90]}B\schemestop").tree.single());
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!(a.arc, Some(UArc { radius: -0.25, half_angle: 90.0 }));
    // drawn: the arc adds strokes to the plain arrow; read as "reacts to give"
    let u = strict(r"\schemestart A\arrow{-U>[$x$][$y$]}B\schemestop");
    let plain = strict(r"\schemestart A\arrow{->[$x$][$y$]}B\schemestop");
    assert!(u.svg.matches("<path").count() + u.svg.matches("<line").count() > plain.svg.matches("<path").count() + plain.svg.matches("<line").count());
    assert_eq!(u.speech, plain.speech);
    // no longer an unknown arrow type
    assert!(tolerant(r"\schemestart A\arrow{-U>}B\schemestop").warnings.is_empty());
}

#[test]
fn chemfig_delimiters() {
    // section 6.8: \chemleft and \chemright are \left and \right around the material
    assert_eq!(strict(r"\chemleft[ \chemfig{A-B} \chemright]").tree, strict(r"\left[ \chemfig{A-B} \right]").tree);
    assert_eq!(strict(r"\chemleft\{\chemfig{A-[1]B}\chemright\}").tree, strict(r"\left\{\chemfig{A-[1]B}\right\}").tree);
    // \chemup and \chemdown: over and under it
    assert_eq!(strict(r"\chemup[\chemfig{A-[1]B-[7]C}\chemdown]").tree, strict(r"\overbracket{\underbracket{{\chemfig{A-[1]B-[7]C}}}}").tree);
    assert_eq!(strict(r"\chemup\{X\chemdown\}").tree, strict(r"\overbrace{\underbrace{{X}}}").tree);
    assert_eq!(strict(r"\chemup(X\chemdown.").tree, strict(r"\overparen{{X}}").tree);
    // inside a scheme, the delimited material is one compound, a sub-scheme in it
    let s = scheme_of(strict(r"\schemestart A\arrow \chemleft[\subscheme{B\arrow[-90,2]C}\chemright] \arrow D\schemestop").tree.single());
    assert_eq!(s.items.len(), 5, "{:?}", s.items);
    assert!(matches!(&s.items[2], Item::Compound(c) if matches!(&c.body, Body::Math { tex } if tex.starts_with(r"\chemleft[") && tex.ends_with(r"\chemright]"))));
    let s = scheme_of(strict(r"\schemestart X\arrow \chemup\{\chemfig{A-[1]B-[7]C}\chemdown\} \arrow Y\schemestop").tree.single());
    assert_eq!(s.items.len(), 5, "{:?}", s.items);
    // a \chemdown alone is ignored with a warning, as chemfig does; \chemup without one too
    warns(r"x \chemdown]", "E-REC-9");
    warns(r"\chemup[ x", "E-REC-5");
}

#[test]
fn skip_align() {
    // section 5.4.3: the group's baseline on the bond's line
    let m = molecule(r"A-\chemskipalign.-B");
    assert!(m.atoms[1].skip_align && m.atoms[1].label == ".");
    assert_ne!(strict(r"\chemfig{A-\chemskipalign.-B}").svg, strict(r"\chemfig{A-.-B}").svg);
    // outside a structure it changes nothing
    assert_eq!(strict(r"x\chemskipalign y").tree, strict("xy").tree);
}

#[test]
fn distant_bonds() {
    // the issue's example: the mark at the start of a group belongs to its atom, B
    let m = molecule("A-?[a]B-C-?[a]");
    assert_eq!(m.atoms.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["A", "B", "C", ""]);
    assert_eq!((m.bonds.len(), m.bonds[3].from, m.bonds[3].to), (4, 1, 3));
    // section 2.8: every later mark of a name is bonded to the first, with its own link
    let m = molecule("A-B(-[1]W-X?)(-[7]Y-Z?)-C?");
    let distant: Vec<(String, String)> = m.bonds.iter().filter(|b| m.atoms[b.from].label == "X").map(|b| (m.atoms[b.from].label.clone(), m.atoms[b.to].label.clone())).collect();
    assert_eq!(distant, [("X".to_string(), "Z".to_string()), ("X".to_string(), "C".to_string())]);
    let m = molecule("A?[a]-B(-[1]W?[a,2]-X?[b])(-[7]Y-Z?[b,1])-C?[b,{>}]");
    let kind = |from: &str, to: &str| m.bonds.iter().find(|b| m.atoms[b.from].label == from && m.atoms[b.to].label == to).map(|b| b.kind);
    assert_eq!((kind("A", "W"), kind("X", "Z"), kind("X", "C")), (Some(BondKind::Double), Some(BondKind::Single), Some(BondKind::Wedge)));
    // several marks after one atom; the piece the mark follows
    assert_eq!(molecule("A?[a]-[:-72]B-C?[a]?[b]-[:72]D-[:144]E?[a]?[b]").bonds.len(), 7);
    let m = molecule("H_3C?-A-B?");
    assert_eq!(m.bonds[2].departure, 1);
    // a mark no other one names draws nothing, as in chemfig
    assert_eq!(molecule("A?[z]-B").bonds.len(), 1);
}

fn merges(tex: &str) -> Vec<Merge> {
    scheme_of(strict(tex).tree.single())
        .items
        .into_iter()
        .filter_map(|i| match i {
            Item::Merge(m) => Some(m),
            _ => None,
        })
        .collect()
}

#[test]
fn merge_joins_named_compounds() {
    // section 6.11, the manual's examples: chemfig's own names c1, c2…, the direction (> by default)
    // and the coefficients [c1,c2,c]
    let m = merges(r"\schemestart A\arrow{<=>}[90]B\merge(c1)(c2)--()C\schemestop");
    assert_eq!((m[0].from.clone(), m[0].angle, m[0].coefficients), (vec![(0, None), (1, None)], 0.0, (0.5, 0.5, 0.5)));
    let m = merges(r"\schemestart A\arrow{<=>}B\merge^(c1)(c2)--()[0.25,1,0.75]C\schemestop");
    assert_eq!((m[0].angle, m[0].coefficients), (90.0, (0.25, 1.0, 0.75)));
    let m = merges(r"\schemestart ABC\arrow[30]EFGHIJ\arrow[45]KLM\arrow[30]NO \merge>(c1)(c2)(c3)--()suite 1 \arrow suite 2 \schemestop");
    assert_eq!(m[0].from.iter().map(|f| f.0).collect::<Vec<_>>(), [0, 1, 2]);
    // names given by \arrow(a--b) (section 6.4), and an anchor's angle
    let m = merges(r"\schemestart Foooo\arrow(foo--bar){<=>}Bar\arrow(--baz){<=>}Bz \merge^(foo)(bar)(baz)--()suite \schemestop");
    assert_eq!(m[0].from.iter().map(|f| f.0).collect::<Vec<_>>(), [0, 1, 2]);
    let m = merges(r"\schemestart A\arrow{<->}[90]B \merge<(c1.120)(c2)--()CCC \schemestop");
    assert_eq!((m[0].from.clone(), m[0].angle), (vec![(0, Some(120.0)), (1, None)], 180.0));
    // the labels above and below the arrow
    let m = merges(r"\schemestart ABC\arrow{<=>}DEF\merge v[sur][sous](c1)(c2)--()[0.25,1,0.75]GHIJ \schemestop");
    assert_eq!(m[0].angle, -90.0);
    assert!(m[0].above.is_some() && m[0].below.is_some());
    // the next compound's name, for a later merge; a \subscheme's box takes a number (section 6.8) and
    // what \+ joins is one compound (section 6.12)
    let m = merges(r"\schemestart A\arrow{<=>}[90]B\merge(c1)(c2)--(x)C\arrow D\merge(x)(c4)--()E\schemestop");
    assert_eq!(m[1].from.iter().map(|f| f.0).collect::<Vec<_>>(), [2, 3]);
    let m = merges(r"\schemestart A\arrow \subscheme{B\arrow[-90,2]C} \arrow D \merge v(c1)(c3)(c5)--()E\schemestop");
    assert_eq!(m[0].from.iter().map(|f| f.0).collect::<Vec<_>>(), [0, 1, 3]);
    let m = merges(r"\schemestart A\+B\arrow[90] C \merge(c1)(c2)--()E\schemestop");
    assert_eq!(m[0].from.iter().map(|f| f.0).collect::<Vec<_>>(), [0, 2]);
    // drawn: the lines and the arrow change the drawing; read as the arrow it ends in
    let out = strict(r"\schemestart A\arrow{<=>}[90]B\merge(c1)(c2)--()C\schemestop");
    assert_ne!(out.svg, strict(r"\schemestart A\arrow{<=>}[90]B C\schemestop").svg);
    assert_eq!(out.text, "A ⇌ B → C");
    assert!(out.speech.ends_with("reacts to give C"), "{}", out.speech);
    // what contextEquation does not draw warns, and strict mode refuses it: a style, a label's rotation, a
    // name no compound has
    for tex in [
        r"\schemestart A\arrow{<=>}[90]B\merge(c1)(c2)--()[,,0.9,red,thick]C\schemestop",
        r"\schemestart ABC\arrow{<=>}[90]DEF\merge>[*{90}sur][*{90}sous](c1)(c2)--()GHIJ\schemestop",
        r"\schemestart A\arrow B\merge(c1)(zz)--()C\schemestop",
    ] {
        let out = warns(tex, "E-REC-9");
        assert_eq!(scheme_of(out.tree.single()).items.iter().filter(|i| matches!(i, Item::Merge(_))).count(), 1, "{tex}");
    }
    // outside a scheme chemfig does not define it: left out with its arguments
    let out = warns(r"\chemfig{A-B}\merge{->}(c1)(c2)--(c3)", "E-REC-1");
    assert!(!out.text.contains("c1"), "{}", out.text);
}

#[test]
fn arrows_name_and_branch() {
    // section 6.4: \arrow(a--b) names the compounds and is not text of the scheme
    let out = strict(r"\schemestart A\arrow(aa--bb)B\arrow(--cc)C\arrow D\schemestop");
    assert_eq!(out.text, "A → B → C → D");
    // section 6.7: an arrow from a named compound, and one between two compounds already placed
    let s = scheme_of(strict(r"\schemestart A\arrow(aa--bb)B\arrow(--cc)C\arrow D \arrow(@bb--xx1)[-90]X\arrow(@xx1--@cc)\schemestop").tree.single());
    let arrows: Vec<(Option<usize>, Option<usize>)> = s
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Arrow(a) => Some((a.from, a.to)),
            _ => None,
        })
        .collect();
    assert_eq!(arrows, [(None, None), (None, None), (None, None), (Some(1), None), (Some(4), Some(2))]);
    // section 6.1: \schemestart[arrow angle=…, arrow coeff=…] sets the arrows that give none
    let s = scheme_of(strict(r"\schemestart[arrow angle=-90,arrow coeff=1.5] A\arrow B\arrow[0] C\schemestop").tree.single());
    let set: Vec<(f32, f32)> = s
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Arrow(a) => Some((a.angle, a.length)),
            _ => None,
        })
        .collect();
    assert_eq!(set, [(-90.0, 1.5), (0.0, 1.5)]);
    assert_eq!(strict(r"\schemestart[arrow angle=0,init anchor=west] A\arrow B\schemestop").text, "A → B");
}

#[test]
fn mathml_keeps_the_fallback() {
    for tex in [r"\chemfig{[:20,1.5]A-B}", r"\charge{90=\|}{X}", r"\schemestart A\arrow{-U>[$x$]}B\schemestop", r"\chemleft[\chemfig{A-B}\chemright]"] {
        let out = strict(tex);
        let back = convert(&out.mathml, &Options { input: Input::MathMl, ..Default::default() });
        assert!(back.is_ok(), "{tex}: {}", out.mathml);
    }
}

#[test]
fn labels_that_are_no_element_keep_their_order_and_letters() {
    // #271: the text is in Hill order: C, then the elements alphabetically, then what is no element
    // as written. In A-B-C, B is boron and C carbon, so CBA is that order, not a reversal
    assert_eq!(strict(r"\chemfig{A-B-C}").text, "CBA");
    assert_eq!(strict(r"\chemfig{R-X-Q}").text, "RXQ");
    assert_eq!(strict(r"\chemfig{Q-X-R}").text, "QXR");
    // a capital and a lowercase letter that are no element stay whole: Nu is not N with a lost u
    assert_eq!(strict(r"\chemfig{Nu-C}").text, "CNu");
    assert_eq!(strict(r"\chemfig{Nu-C}").speech, "structure of C Nu");
}

/// The angle of the bond that reaches the atom labelled `label`.
fn angle_to(m: &Molecule, label: &str) -> f32 {
    let id = m.atoms.iter().position(|a| a.label == label).unwrap_or_else(|| panic!("no atom {label}"));
    m.bonds.iter().find(|b| b.to == id).unwrap_or_else(|| panic!("no bond to {label}")).angle
}

#[test]
fn a_branch_starts_with_defaults_of_its_own() {
    // chemfig manual, section 2.6 ("Valeur par défaut dans les ramifications", "Ramifications imbriquées"):
    // a branch's [angle,length,…] holds in all of it, its own branches included, and a relative angle
    // is relative to the bond that reached the atom
    let m = molecule("A-B([1]-X([4]-Z)-Y)(-[7]D)-C");
    assert_eq!((angle_to(&m, "X"), angle_to(&m, "Y"), angle_to(&m, "Z"), angle_to(&m, "D"), angle_to(&m, "C")), (45.0, 45.0, 180.0, -45.0, 0.0));
    let m = molecule("A-B([:60]-D-E)([::-30,1.5]-X-Y)-C");
    assert_eq!((angle_to(&m, "D"), angle_to(&m, "E"), angle_to(&m, "X"), angle_to(&m, "Y"), angle_to(&m, "C")), (60.0, 60.0, -30.0, -30.0, 0.0));
    let x = m.atoms.iter().position(|a| a.label == "X").unwrap();
    assert_eq!(m.bonds.iter().find(|b| b.to == x).unwrap().length, 1.5);
    // the defaults end with the branch; a bond with its own angle keeps it
    let m = molecule("A-B([:60]-D-[:0]E)-C");
    assert_eq!((angle_to(&m, "E"), angle_to(&m, "C")), (0.0, 0.0));
    // from a ring vertex, the branch's own angle replaces the bisector
    let m = molecule("*5(---([:90]-A-B)--)");
    assert_eq!((angle_to(&m, "A"), angle_to(&m, "B")), (90.0, 90.0));
}

#[test]
fn a_bond_sets_its_own_spacing() {
    // chemfig manual, section 2.2: #(dim1,dim2) right after the bond sign, also \# or \CFhash; an empty
    // field or a missing dim2 keeps the standard space
    let gaps = |body: &str| molecule(body).bonds.iter().map(|b| (b.start_gap, b.end_gap)).collect::<Vec<_>>();
    assert_eq!(gaps("A-#(,0pt)B-C"), [(None, Some(0.0)), (None, None)]);
    assert_eq!(gaps("A-B-#(0pt)C"), [(None, None), (Some(0.0), None)]);
    assert_eq!(gaps(r"A-\#(,0pt)B-\CFhash(0pt,1em)C"), [(None, Some(0.0)), (Some(0.0), Some(1.0))]);
    assert_eq!(gaps("A=#(2pt,2pt)[1]B"), [(Some(0.2), Some(0.2))]);
    // the atoms stay where they are; only the bond reaches further, up to the letters
    let (plain, touching) = (strict(r"\chemfig{A-B}"), strict(r"\chemfig{A-#(,0pt)B}"));
    assert_ne!(plain.svg, touching.svg);
    assert_eq!(context_equation_core::typeset(r"\chemfig{A-B}", &Options::default()).unwrap().0.width, context_equation_core::typeset(r"\chemfig{A-#(,0pt)B}", &Options::default()).unwrap().0.width);
    assert_eq!(strict(r"\chemfig{A-#(,0pt)B}").tree.to_json().matches("end_gap").count(), 1);
    // a dimension it cannot read keeps the standard space, with a warning
    warns(r"\chemfig{A-#(abc)B}", "E-REC-9");
}

#[test]
fn an_aromatic_circle_can_be_an_arc() {
    // chemfig manual, section 3.2: **[angle1,angle2,tikz]n(…), the circle from angle1 to angle2
    let ring = |body: &str| molecule(body).rings[0].clone();
    let r = ring("**[30,330]5(-----)");
    assert_eq!((r.arc, r.dashed, r.aromatic), (Some((30.0, 330.0)), false, true));
    let r = ring("**[0,270,dash pattern=on 2pt off 2pt]4(----)");
    assert_eq!((r.arc, r.dashed), (Some((0.0, 270.0)), true));
    // the defaults are the whole circle
    assert_eq!(ring("**[,]6(------)").arc, None);
    assert_eq!(ring("**6(------)"), ring("**[]6(------)"));
    // an arc draws less ink than the circle, and both keep the formula
    assert_ne!(strict(r"\chemfig{**[30,330]5(-----)}").svg, strict(r"\chemfig{**5(-----)}").svg);
    assert_eq!(strict(r"\chemfig{**[30,330]5(-----)}").text, strict(r"\chemfig{**5(-----)}").text);
}

#[test]
fn a_mechanism_arrow_can_point_at_a_bond() {
    // chemfig manual, section 5.8.1: @{name,coeff} at the start of a bond's first field puts a node on
    // the bond, coeff of the way along it (0.5 by default); the rest of the field is the angle
    let m = molecule("@{a1}=_[@{db}::30]-[@{sb,0.25}::-60]@{dnl}X");
    let hooks: Vec<_> = m.bonds.iter().map(|b| b.hook.clone()).collect();
    assert_eq!(hooks, [Some(("db".to_string(), 0.5)), Some(("sb".to_string(), 0.25))]);
    assert_eq!((m.bonds[0].angle, m.bonds[1].angle), (30.0, -30.0));
    // the manual's "Effet mésomère" draws both arrows, with no warning, and says what they join
    let tex = r"\chemfig{@{a1}=_[@{db}::30]-[@{sb}::-60]@{dnl}\charge{90=\|}{X}} \chemmove[->]{\draw(db)..controls +(100:5mm) and +(145:5mm)..(a1); \draw(dnl)..controls +(90:4mm) and +(45:4mm)..(sb);}";
    let out = strict(tex);
    assert!(out.speech.contains("from the bond between C and C to a1"), "{}", out.speech);
    assert!(out.speech.contains("from X to the bond between C and X"), "{}", out.speech);
    let pt = convert(tex, &Options { locale: "pt-BR".parse().unwrap(), strict: true, ..Default::default() }).unwrap();
    assert!(pt.speech.contains("vai de ligação entre C e C para a1"), "{}", pt.speech);
    // a bond's node in an angle field written without a comma, as the manual does: [@{b1}6]
    let m = molecule("A-[@{b1}6]B");
    assert_eq!((m.bonds[0].hook.as_ref().map(|h| h.0.as_str()), m.bonds[0].angle), (Some("b1"), -90.0));
}

#[test]
fn definesubmol_stands_for_its_code() {
    // chemfig manual, section 9, its examples one by one
    assert_eq!(strict(r"\definesubmol{xy}{CH_2} \chemfig{H_3C-!{xy}-!{xy}-!{xy}-CH_3}").text, "C₅H₁₂");
    assert_eq!(strict(r"\definesubmol\xx{C(-[::+90]H)(-[::-90]H)} \chemfig{[:15]H-!\xx-!\xx-!\xx-!\xx-H}").text, "C₄H₁₀");
    // a dual alias: [code1] when the bond comes from the right, so the carbon is next to the ring
    let tex = r"\definesubmol\Me[H_3C]{CH_3} \chemfig{*6((-!\Me)=(-!\Me)-(-!\Me)=(-!\Me)-(-!\Me)=(-!\Me)-)}";
    let labels: Vec<String> = scheme_of(strict(tex).tree.single())
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Compound(c) => match &c.body {
                Body::Structure(m) => Some(m.atoms.iter().filter(|a| !a.label.is_empty()).map(|a| a.label.clone()).collect::<Vec<_>>()),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(labels.iter().filter(|l| *l == "H_3C").count(), 3, "{labels:?}");
    assert_eq!(labels.iter().filter(|l| *l == "CH_3").count(), 3, "{labels:?}");
    // arguments, #1…#n; an argument after a control word stays a token of its own
    let m = molecule_after(r"\definesubmol\X1{-[,-0.2,,,draw=none]{\scriptstyle#1}}", r"*6((!\X A)-(!\X B)-(!\X C)-(!\X D)-(!\X E)-(!\X F)-)");
    assert!(m.atoms.iter().any(|a| a.label == r"{\scriptstyle A}"), "{:?}", m.atoms.iter().map(|a| &a.label).collect::<Vec<_>>());
    // any other # is drawn as it is, and #( is a bond's spacing
    let m = molecule_after(r"\definesubmol\X2{#1-#2-#3-#(3pt,3pt)#4}", "A-!\\X{M}{N}-B");
    assert_eq!(m.atoms.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["A", "M", "N", r"\#3", r"\#4", "B"]);
    assert_eq!(m.bonds[3].start_gap, Some(0.3));
    // an alias from a single character, as the manual's adrenaline uses for invisible bonds
    assert!(strict(r"\definesubmol{&}{-[,,,,draw=none]} \chemfig{*6((-HO)-=*6(!&!&HN---)-=-(-HO)=)}").warnings.is_empty());
    // an unknown name warns and is left out; a submolecule that calls itself stops, with a warning
    let out = warns(r"\chemfig{A-!{nope}B}", "E-REC-1");
    assert!(out.tree.to_json().contains("!{nope}"), "the label keeps what was written");
    warns(r"\definesubmol{a}{-!a} \chemfig{A!a}", "E-REC-1");
}

/// The structure of `\chemfig{body}` after `defs` in the same input.
fn molecule_after(defs: &str, body: &str) -> Molecule {
    let out = strict(&format!("{defs} \\chemfig{{{body}}}"));
    match &scheme_of(out.tree.single()).items[0] {
        Item::Compound(c) => match &c.body {
            Body::Structure(m) => m.clone(),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_arrow_can_move_the_compound_it_brings() {
    // chemfig manual, section 6.6: \arrow(--name[yshift=5mm]) moves the compound after the arrow
    let arrow = |tex: &str| scheme_of(strict(tex).tree.single()).items.iter().find_map(|i| if let Item::Arrow(a) = i { Some(a.clone()) } else { None }).unwrap();
    assert_eq!(arrow(r"\schemestart A\arrow(--foo[yshift=5mm])B\schemestop").shift, Some((0.0, 5.0 * 0.2845)));
    assert_eq!(arrow(r"\schemestart A\arrow(--[xshift=1em,yshift=-10pt])B\schemestop").shift, Some((1.0, -1.0)));
    assert_eq!(arrow(r"\schemestart A\arrow B\schemestop").shift, None);
    // B is higher, and the arrow goes up to it
    let height = |tex: &str| context_equation_core::typeset(tex, &Options::default()).unwrap().0.height;
    assert!(height(r"\schemestart A\arrow(--foo[yshift=5mm])B\schemestop") > height(r"\schemestart A\arrow B\schemestop"));
    // what contextEquation does not draw (colors, fill) still warns, alone
    let out = warns(r"\schemestart A\arrow([red]--[fill=blue,yshift=2pt])B\schemestop", "E-REC-9");
    assert!(out.warnings.iter().any(|w| w.description.contains("compound style [fill=blue]")), "{:?}", out.warnings);
    assert!(!out.warnings.iter().any(|w| w.description.contains("yshift")), "{:?}", out.warnings);
}

#[test]
fn a_compound_name_breaks_into_lines() {
    // chemfig manual, section 6.3: \\ in a \chemname breaks it, each line centered under the compound
    let out = strict(r"\schemestart \chemname{\chemfig{R-OH}}{Acide\\carboxylique} \schemestop");
    assert!(out.speech.starts_with("Acide carboxylique, structure of"), "{}", out.speech);
    let one = context_equation_core::typeset(r"\schemestart \chemname{\chemfig{R-OH}}{Acide carboxylique} \schemestop", &Options::default()).unwrap().0;
    let two = context_equation_core::typeset(r"\schemestart \chemname{\chemfig{R-OH}}{Acide\\carboxylique} \schemestop", &Options::default()).unwrap().0;
    assert!(two.depth > one.depth && two.width < one.width, "two lines go lower and narrower");
}

#[test]
fn chemleft_takes_its_delimiter_between_braces() {
    // the manual writes \chemleft{[}…\chemright{]}, as \chemleft[…\chemright]
    assert_eq!(strict(r"\chemleft{[}\chemfig{A-B}\chemright{]}").tree, strict(r"\chemleft[\chemfig{A-B}\chemright]").tree);
}

#[test]
fn an_arrow_can_leave_a_subscheme_box() {
    // chemfig manual, section 6.8: a \subscheme's box has its name, before its first compound's; an
    // arrow from it leaves the box around its compounds
    let tex = r"\schemestart \subscheme{\chemfig{A-B} \arrow{0}[,0]\+\chemfig{C-D}} \arrow(@c1--olefin){<=>}[-90] \chemfig{E-F} \schemestop";
    let arrows: Vec<_> = scheme_of(strict(tex).tree.single()).items.iter().filter_map(|i| if let Item::Arrow(a) = i { Some(a.clone()) } else { None }).collect();
    assert_eq!(arrows.last().unwrap().from_box, Some((0, 2)));
    // a box holding only a sign has no compound to leave from: it warns, as before
    warns(r"\schemestart A\subscheme{\+}BCDEF \arrow G \arrow(@c2--)[-90]H \schemestop", "E-REC-9");
}

#[test]
fn a_plus_right_after_an_arrow_is_drawn() {
    // the manual's "Alignement du signe +": \arrow{0}[,0]\+ puts the sign with the next compound
    let with = context_equation_core::typeset(r"\schemestart \chemfig{A-B} \arrow{0}[,0]\+\chemfig{C-D} \schemestop", &Options::default()).unwrap().0;
    let without = context_equation_core::typeset(r"\schemestart \chemfig{A-B} \arrow{0}[,0]\chemfig{C-D} \schemestop", &Options::default()).unwrap().0;
    assert!(with.width > without.width, "the + takes its place");
}

#[test]
fn a_mechanism_arrow_point_may_move_and_name_an_anchor() {
    // the manual's ([yshift=7pt]b1.180): the hook is b1; the point's own move and anchor are left out
    let out = strict(r"\schemestart \chemfig{@{a}A-B} \arrow \chemfig{@{b}C-D} \chemmove{\draw(a)..([yshift=7pt]b.180);} \schemestop");
    let s = scheme_of(out.tree.single());
    assert_eq!((s.moves.len(), s.moves[0].to.as_str()), (1, "b"));
}
