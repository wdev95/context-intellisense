//! L-CHE-2, the corners of structural chemistry the other suites leave: the scheme reader's rarer
//! commands and recoveries (`$…$` and loose commands in a scheme, `\chemname*`, `\+{…}`, `\merge`
//! and `\arrow` with names that do not resolve, `\chemmove` paths with keys and units), the label
//! commands and `\charge` options of chemfig, what the layout draws for arrows with nothing after
//! them, and the drawing primitives and formulas the chemistry modules expose.
#![cfg(feature = "chem")]

use context_equation_core::chem::draw::{Drawing, Rect, Shape};
use context_equation_core::chem::formula::Formula;
use context_equation_core::chem::model::{ArrowKind, Body, BondKind, ChargeContent, Item, Label, Molecule, Scheme};
use context_equation_core::tree::Node;
use context_equation_core::{convert, typeset, Options};

fn strict(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let tolerant = convert(tex, &Options::default()).unwrap();
    assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    out
}

/// The tolerant output of an input strict mode refuses, with the warning that names `problem`;
/// strict mode's error says the same.
fn recovered(tex: &str, problem: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"));
    assert!(out.warnings.iter().any(|w| w.description.contains(problem)), "{tex}: {:?}", out.warnings);
    let error = convert(tex, &Options { strict: true, ..Default::default() }).expect_err(tex);
    assert!(error.to_string().contains(problem), "{tex}: {error}");
    out
}

fn scheme_of(out: &context_equation_core::Output) -> Scheme {
    match out.tree.single() {
        Node::Chemistry { scheme } => scheme.clone(),
        other => panic!("{other:?}"),
    }
}

fn scheme(tex: &str) -> Scheme {
    scheme_of(&strict(tex))
}

fn math_tex(item: &Item) -> &str {
    match item {
        Item::Compound(c) => match &c.body {
            Body::Math { tex } => tex,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

fn structure(item: &Item) -> &Molecule {
    match item {
        Item::Compound(c) => match &c.body {
            Body::Structure(m) => m,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

fn molecule(tex: &str) -> Molecule {
    structure(&scheme(tex).items[0]).clone()
}

fn width(tex: &str) -> f32 {
    typeset(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}")).0.width
}

#[test]
fn math_and_loose_commands_inside_a_scheme_are_compounds() {
    // $…$ is a compound of math, as written
    let s = scheme(r"\schemestart $x^2$ \arrow B \schemestop");
    assert_eq!(math_tex(&s.items[0]), "x^2");
    assert!(matches!(s.items[1], Item::Arrow(_)));
    assert_eq!(math_tex(&s.items[2]), r"\ce{B}");
    // an empty $ $ is an empty compound, drawn as nothing
    let s = scheme(r"\schemestart $ $ \schemestop");
    assert_eq!(math_tex(&s.items[0]), "");
    // any other command is math up to the next scheme command: \Delta alone, then \Delta x before \+
    let s = scheme(r"\schemestart A \arrow{->} \Delta \arrow B \schemestop");
    assert_eq!(math_tex(&s.items[2]), r"\Delta");
    assert_eq!(s.items.len(), 5);
    let s = scheme(r"\schemestart A \arrow{->} \Delta x \+ B \schemestop");
    assert_eq!(math_tex(&s.items[2]), r"\Delta x");
    assert_eq!(s.items[3], Item::Plus);
    // or up to a $ that starts math of its own
    let s = scheme(r"\schemestart A \arrow \Delta$x$ \schemestop");
    assert_eq!((math_tex(&s.items[2]), math_tex(&s.items[3])), (r"\Delta", "x"));
}

#[test]
fn a_ring_fused_at_a_vertex_names_the_shared_atom_with_the_next_label() {
    // the bond after the inner ring reaches the vertex both rings share: its label goes there
    let m = molecule(r"\chemfig{*6(---*6(------)-N--)}");
    assert_eq!(m.rings.len(), 2);
    let n = m.atoms.iter().position(|a| a.label == "N").unwrap();
    assert!(m.rings.iter().all(|r| r.atoms.contains(&n)), "{:?}", m.rings);
    assert_eq!(m.atoms.len(), 10);
    // an @ with no braced name after it hooks nothing, in a label or after a branch
    for tex in [r"\chemfig{A@-B}", r"\chemfig{A(-B)@-C}"] {
        let m = molecule(tex);
        assert!(m.atoms.iter().all(|a| a.hook.is_none()), "{tex}");
        assert_eq!(m.bonds.last().map(|b| (b.from, b.kind)), Some((0, BondKind::Single)), "{tex}");
    }
}

#[test]
fn chemname_starred_empty_and_the_plus_with_its_spacing() {
    // \chemname* names the compound as \chemname does (the star only changes chemfig's alignment)
    let s = scheme(r"\schemestart \chemname*{\chemfig{A-B}}{name} \schemestop");
    let Item::Compound(c) = &s.items[0] else { panic!() };
    assert_eq!(c.name.as_deref(), Some("name"));
    assert!(matches!(c.body, Body::Structure(_)));
    // the same outside a scheme
    let s = scheme(r"\chemname*{\chemfig{A-B}}{name}");
    let Item::Compound(c) = &s.items[0] else { panic!() };
    assert_eq!(c.name.as_deref(), Some("name"));
    // a name over nothing is a name under an empty compound
    let s = scheme(r"\schemestart \chemname{}{label} \schemestop");
    let Item::Compound(c) = &s.items[0] else { panic!() };
    assert_eq!(c.name.as_deref(), Some("label"));
    assert_eq!(c.body, Body::Math { tex: String::new() });
    // \+{before,after,shift}: the spacing is contextEquation's, the sign is kept
    let s = scheme(r"\schemestart A \+{1em,1em,0} B \schemestop");
    assert_eq!(s.items[1], Item::Plus);
    assert_eq!(s.items.len(), 3);
    // \chemfig[options]{…}: the options are read and left out
    let m = molecule(r"\chemfig[atom sep=2em]{A-B}");
    assert_eq!(m.atoms.len(), 2);
}

#[test]
fn a_chemfig_without_its_braces_is_an_empty_structure() {
    let s = scheme_of(&recovered(r"\schemestart \chemfig A \schemestop", "missing braced argument"));
    // the structure is one bare vertex; the letter after it is a formula of its own
    let m = structure(&s.items[0]);
    assert_eq!((m.atoms.len(), m.atoms[0].label.as_str(), m.bonds.len()), (1, "", 0));
    assert_eq!(math_tex(&s.items[1]), r"\ce{A}");
}

#[test]
fn arrow_labels_keep_nested_braces_of_their_commands() {
    let s = scheme(r"\schemestart A \arrow{->[\ce{a{b}c} text]} B \schemestop");
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!(a.above, Some(Label::Text { tex: r"\ce{a{b}c} text".into() }));
    // drawn: the label is read as \ce{a{b}c} and the word in \text
    assert!(width(r"\schemestart A \arrow{->[\ce{a{b}c} text]} B \schemestop") > width(r"\schemestart A \arrow B \schemestop"));
}

#[test]
fn arrows_from_and_to_names_that_do_not_resolve() {
    // @c9: there is no ninth compound
    let s = scheme_of(&recovered(r"\schemestart A \arrow(@c9--) B \schemestop", "no compound is named c9"));
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!((a.from, a.from_box), (None, None));
    // after a \subscheme box and arrows, c7 is neither a compound nor a box
    recovered(r"\schemestart \subscheme{A \arrow B} \arrow C \arrow(@c7--) D \schemestop", "no compound is named c7");
    // an arrow to a name no compound has
    let s = scheme_of(&recovered(r"\schemestart A \arrow(--@zz) B \schemestop", "an arrow to @zz"));
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!(a.to, None);
    // a move of the compound before the arrow is not chemfig's to give
    recovered(r"\schemestart A \arrow(a[xshift=1em]--b) B \schemestop", "a move of the compound before an arrow");
    // a style key that is not a shift, even with a length, is left out
    let s = scheme_of(&recovered(r"\schemestart A \arrow(--b[foo=1cm]) B \schemestop", "compound style [foo=1cm]"));
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!(a.shift, None);
}

#[test]
fn merge_directions_moves_and_missing_parts() {
    // a braced direction
    let s = scheme(r"\schemestart A \arrow(a--b) B \merge{>}(a)(b)--(c) C \schemestop");
    let Item::Merge(m) = &s.items[3] else { panic!("{:?}", s.items) };
    assert_eq!(m.from, vec![(0, None), (1, None)]);
    assert_eq!(m.angle, 0.0);
    // an unknown braced direction goes to the right
    let s = scheme_of(&recovered(r"\schemestart A \arrow(a--b) B \merge{x}(a)(b)--(c) C \schemestop", r"\merge direction {x}"));
    let Item::Merge(m) = &s.items[3] else { panic!() };
    assert_eq!(m.angle, 0.0);
    // moves of the compounds joined, or of the next one, are left out
    let out = recovered(r"\schemestart A \arrow(a--b) B \merge>(a[xshift=1em])(b)--(c[yshift=2pt]) C \schemestop", r"\merge: a move of a");
    assert!(out.warnings.iter().any(|w| w.description.contains(r"\merge: a move of c")), "{:?}", out.warnings);
    // without --(name), or without -- at all, the next compound follows the arrow
    for tex in [r"\schemestart A \arrow(a--b) B \merge>(a)(b)-- C \schemestop", r"\schemestart A \arrow(a--b) B \merge>(a)(b) C \schemestop"] {
        let s = scheme_of(&recovered(tex, r"\merge without --(name)"));
        assert!(matches!(s.items[3], Item::Merge(_)), "{tex}");
        assert_eq!(math_tex(&s.items[4]), r"\ce{C}", "{tex}");
    }
    // a merge that joins no compound is left out
    let out = recovered(r"\schemestart A \arrow(a--b) B \merge>(zz)--(c) C \schemestop", r"\merge: no compound is named zz");
    assert!(out.warnings.iter().any(|w| w.description.contains(r"\merge joins no compound")), "{:?}", out.warnings);
    assert!(!scheme_of(&out).items.iter().any(|i| matches!(i, Item::Merge(_))));
}

#[test]
fn merge_outside_a_scheme_is_read_to_its_end_and_left_out() {
    for tex in [r"\merge > [a] (c1) -- (c2)", r"\merge {v} [a][b] ( c1 ) -- ( c2 ) [0.5,0.5,0.5]", r"\merge (c1)--(c2)", r"\merge [[a]] (c1)--(c2)"] {
        let out = recovered(tex, r"chemfig defines it only inside \schemestart");
        // the whole construction is taken: nothing of it is left in the equation
        let written = &out.warnings[0].description;
        assert!(written.contains(tex), "{written}");
        assert_eq!(*out.tree.single(), Node::row(vec![]), "{tex}");
    }
}

#[test]
fn chemleft_and_chemright_nest_and_take_a_named_delimiter() {
    // the outer \chemleft[ ends at its own \chemright ], past the inner pair
    let s = scheme(r"\schemestart \chemleft[ \chemleft( A \chemright) \chemright ] \schemestop");
    assert_eq!(s.items.len(), 1);
    assert_eq!(math_tex(&s.items[0]), r"\chemleft[ \chemleft( A \chemright) \chemright ]");
    // the closing delimiter as a control word, after a space
    let s = scheme(r"\schemestart \chemleft[ A \chemright \rbrack \schemestop");
    assert_eq!(math_tex(&s.items[0]), r"\chemleft[ A \chemright \rbrack");
}

#[test]
fn chemmove_paths_with_keys_bends_and_units() {
    let moves = |body: &str| scheme(&format!(r"\chemfig{{@{{a}}A-@{{b}}B}} \chemmove{{{body}}}")).moves;
    // out= and in= set the angles
    let m = &moves(r"\draw(a) to[out=90,in=45] (b);")[0];
    assert_eq!((m.out_angle, m.in_angle), (Some(90.0), Some(45.0)));
    // bend right turns the other way, by 45° unless it says how much
    assert_eq!(moves(r"\draw(a) to[bend right=30] (b);")[0].bend, -30.0);
    assert_eq!(moves(r"\draw(a) to[bend right] (b);")[0].bend, -45.0);
    assert_eq!(moves(r"\draw(a) to[bend left=20] (b);")[0].bend, 20.0);
    // control points in ex and em; a length without a unit is in em, an unknown unit is no control
    let m = &moves(r"\draw(a)..controls +(90:1ex) and +(45:1em)..(b);")[0];
    assert_eq!(m.reach, Some((0.43, 1.0)));
    let m = &moves(r"\draw(a)..controls +(90:1) and +(45:1furlong)..(b);")[0];
    assert_eq!(m.reach, None);
    assert_eq!(m.out_angle, None);
    // one hook is not an arrow
    let out = recovered(r"\chemfig{@{a}A-@{b}B} \chemmove{\draw(a);}", "an electron arrow needs two hooks");
    assert!(scheme_of(&out).moves.is_empty());
    // a hook name in nested parentheses is the name with them, which no atom has
    recovered(r"\chemfig{@{a}A-@{b}B} \chemmove{\draw((a))..controls +(90:1pt) and +(45:1in)..(b);}", "no atom is hooked as @{(a)}");
}

#[test]
fn chemmove_to_a_hook_on_a_bond_too_short_to_draw() {
    // the bond's node is where the arrow would end; a bond hidden under its labels has none, and the
    // arrow is not drawn
    let short = r"\chemfig{@{a}A-[@{h}0,0.1]B} \chemmove{\draw(a) to (h);}";
    let long = r"\chemfig{@{a}A-[@{h}0,2]B} \chemmove{\draw(a) to (h);}";
    assert_eq!(scheme(short).moves.len(), 1);
    let drawn = |tex: &str| typeset(tex, &Options::default()).unwrap().0.items.len();
    let bare = |tex: &str| tex.split(r" \chemmove").next().unwrap().to_string();
    assert_eq!(drawn(short), drawn(&bare(short)));
    assert!(drawn(long) > drawn(&bare(long)));
}

#[test]
fn hooks_after_a_branch_and_closures_that_strengthen_a_bond() {
    // @{x} after a branch hooks the atom the branch hangs from
    let m = molecule(r"\chemfig{A(-B)@{x}-C}");
    assert_eq!(m.atoms[0].hook.as_deref(), Some("x"));
    assert_eq!(m.bonds.len(), 2);
    // a closure over an existing bond makes it double; written from the far end, the bond turns round
    let m = molecule(r"\chemfig{A?-B?[a,{=}]}");
    assert_eq!(m.bonds.len(), 1);
    assert_eq!((m.bonds[0].from, m.bonds[0].to, m.bonds[0].kind), (0, 1, BondKind::Double));
    let m = molecule(r"\chemfig{X(-Y?)?[a,{=}]}");
    assert_eq!(m.bonds.len(), 1);
    assert_eq!((m.bonds[0].from, m.bonds[0].to, m.bonds[0].kind), (1, 0, BondKind::Double));
}

#[test]
fn submolecules_at_the_end_of_the_input() {
    // `!` with nothing after it is an unknown submolecule, written as it is
    let out = recovered(r"\chemfig{A-!}", "unknown submolecule !{}");
    assert_eq!(structure(&scheme_of(&out).items[0]).atoms[1].label, "!");
    // an argument the input does not give is empty: the bond ends at a bare vertex
    let m = molecule(r"\definesubmol{x}1{-#1} \chemfig{A!x}");
    assert_eq!(m.atoms.len(), 2);
    assert_eq!(m.atoms[1].label, "");
    let m = molecule(r"\definesubmol{x}1{-#1} \chemfig{A!xB}");
    assert_eq!(m.atoms[1].label, "B");
}

#[test]
fn label_commands_chemabove_chembelow_and_unknown_ones() {
    let m = molecule(r"\chemfig{\chemabove{N}{+}-\chembelow{C}{-}}");
    assert_eq!(m.atoms[0].label, r"\overset{+}{N}");
    assert_eq!(m.atoms[1].label, r"\underset{-}{C}");
    // without braces, the arguments are empty
    let m = molecule(r"\chemfig{\chemabove N}");
    assert_eq!(m.atoms[0].label, r"\overset{}{}N");
    // an unknown command keeps its braced arguments, for the LaTeX reader to judge
    let out = recovered(r"\chemfig{\foo{X}{Y}-C}", r"unsupported command: \foo");
    assert_eq!(structure(&scheme_of(&out).items[0]).atoms[0].label, r"\foo{X}{Y}");
    // a direction digit of 8 or more, or a letter, is no Lewis mark
    let m = molecule(r"\chemfig{\lewis{8x9,O}}");
    assert!(m.atoms[0].lewis.is_empty());
}

#[test]
fn charge_options_and_their_recoveries() {
    // a general parameter other than extra sep is left out; an empty item too
    for tex in [r"\chemfig{\charge{[foo=1]90=+}{N}}", r"\chemfig{\charge{90=+,}{N}}"] {
        let m = molecule(tex);
        assert_eq!(m.atoms[0].charges.len(), 1, "{tex}");
        assert_eq!(m.atoms[0].charges[0].offset, 0.0, "{tex}");
        assert_eq!(m.atoms[0].charges[0].content, ChargeContent::Text { tex: "+".into() });
        assert_eq!(strict(tex).text, "N⁺");
    }
    // anchor= an angle
    let m = molecule(r"\chemfig{\charge{90[anchor=45]=+}{N}}");
    assert_eq!(m.atoms[0].charges[0].anchor, Some(45.0));
    // anchor= something else: the charge centered on its position
    let out = recovered(r"\chemfig{\charge{90[anchor=xyz]=+}{N}}", r#"\charge anchor "xyz""#);
    assert_eq!(structure(&scheme_of(&out).items[0]).atoms[0].charges[0].anchor, None);
    // a position without its charge
    let out = recovered(r"\chemfig{\charge{90}{N}}", r"\charge{90} without '='");
    assert!(structure(&scheme_of(&out).items[0]).atoms[0].charges.is_empty());
    // text that is not a sign is no formal charge
    assert_eq!(strict(r"\chemfig{\charge{90=x}{N}}").text, "N");
}

#[test]
fn a_vertical_bond_leaves_a_methyl_from_its_carbon() {
    let m = molecule(r"\chemfig{H_3C-[2]X}");
    assert_eq!(m.atoms[0].pieces, vec!["H_3", "C"]);
    assert_eq!(m.bonds[0].departure, 1);
    assert_eq!(m.bonds[0].angle, 90.0);
}

#[test]
fn arrows_with_nothing_after_them_still_draw() {
    // an arrow at the end of a scheme
    let s = scheme(r"\schemestart A \arrow \schemestop");
    assert!(matches!(s.items.last(), Some(Item::Arrow(_))));
    assert!(width(r"\schemestart A \arrow \schemestop") > width(r"\schemestart A \schemestop") + 1.0);
    // two arrows in a row: the first ends in empty space, the second leaves from there
    assert!(width(r"\schemestart A \arrow \arrow B \schemestop") > width(r"\schemestart A \arrow B \schemestop") + 1.0);
    // an arrow just before a \merge ends in empty space too
    let tex = r"\schemestart A \arrow(a--b) B \arrow \merge>(a)(b)--(c) C \schemestop";
    let s = scheme(tex);
    assert!(matches!(s.items[3], Item::Arrow(_)) && matches!(s.items[4], Item::Merge(_)));
    let strokes = |tex: &str| typeset(tex, &Options::default()).unwrap().0.items.len();
    assert!(strokes(tex) > strokes(r"\schemestart A \arrow(a--b) B \merge>(a)(b)--(c) C \schemestop"));
    // an invisible arrow is still a gap between its compounds, read as nothing
    let s = scheme(r"\schemestart A \arrow{0} B \schemestop");
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    assert_eq!(a.kind, ArrowKind::Invisible);
    assert_eq!(strict(r"\schemestart A \arrow{0} B \schemestop").text, "A B");
}

#[test]
fn compound_names_with_commands_and_groups_split_into_lines() {
    let tex = r"\schemestart \chemname{A}{\textbf{x} {y} \\ z} \schemestop";
    let s = scheme(tex);
    let Item::Compound(c) = &s.items[0] else { panic!() };
    assert_eq!(c.name.as_deref(), Some(r"\textbf{x} {y} \\ z"));
    // two lines under the compound: taller than one
    let one = width(r"\schemestart \chemname{A}{\textbf{x} {y} z} \schemestop");
    assert!(width(tex) < one, "the second line makes the name narrower");
    let (two_lines, _) = typeset(tex, &Options::default()).unwrap();
    let (one_line, _) = typeset(r"\schemestart \chemname{A}{\textbf{x} {y} z} \schemestop", &Options::default()).unwrap();
    assert!(two_lines.height + two_lines.depth > one_line.height + one_line.depth);
}

#[test]
fn a_structure_over_an_arrow_is_drawn_smaller_with_its_ring_circle() {
    let tex = r"\schemestart A \arrow{->[\chemfig{**6(------)}]} B \schemestop";
    let s = scheme(tex);
    let Item::Arrow(a) = &s.items[1] else { panic!() };
    let Some(Label::Structure(m)) = &a.above else { panic!("{a:?}") };
    assert!(m.rings[0].aromatic);
    assert!(strict(tex).text.contains("C₆H₆"));
    let (b, _) = typeset(tex, &Options::default()).unwrap();
    assert!(b.items.iter().any(|i| matches!(i, context_equation_core::layout::Item::Ellipse { .. })));
}

#[test]
fn chemup_with_a_delimiter_it_cannot_draw() {
    // `<` and `>` have no turned form: left out, the bracket that has one kept
    let out = recovered(r"\chemup[ A \chemdown <", r#"\chemup or \chemdown with the delimiter "<""#);
    assert!(matches!(out.tree.single(), Node::Brace { above: true, bracket: true, .. }), "{:?}", out.tree);
    let out = recovered(r"\chemup< A \chemdown >", r#"with the delimiter ">""#);
    assert_eq!(out.warnings.len(), 2);
    assert!(matches!(out.tree.single(), Node::Identifier { .. }));
}

#[test]
fn formulas_of_a_tree_with_and_without_chemistry() {
    // a row with a structure among other things
    let out = strict(r"x + \chemfig{H-O-H}");
    assert_eq!(context_equation_core::chem::formulas(&out.tree), vec!["H₂O"]);
    // a node that is neither a row nor chemistry has none
    let out = strict(r"\frac{1}{2}");
    assert!(context_equation_core::chem::formulas(out.tree.single()).is_empty());
}

#[test]
fn charges_written_as_superscript_digits() {
    let f = Formula { counts: [("Fe".to_string(), 1)].into(), unknown: vec![], charge: 13 };
    assert_eq!(f.written(), "Fe¹³⁺");
    assert_eq!(f.tex(), r"\mathrm{Fe}^{13+}");
    let f = Formula { charge: -1, ..f };
    assert_eq!(f.written(), "Fe⁻");
    assert_eq!(strict(r"\chemfig{Fe^{3+}}").text, "Fe³⁺");
}

#[test]
fn rectangles_grow_and_a_ray_without_direction_stays_put() {
    let r = Rect { x0: 0.0, y0: 0.0, x1: 2.0, y1: 1.0 };
    assert_eq!(r.grow(0.5), Rect { x0: -0.5, y0: -0.5, x1: 2.5, y1: 1.5 });
    // a ray with no direction never leaves: it stays at its start
    assert_eq!(r.exit((1.0, 0.5), (0.0, 0.0)), (1.0, 0.5));
    // a ray to the left leaves through the left side
    assert_eq!(r.exit((1.0, 0.5), (-1.0, 0.0)), (0.0, 0.5));
}

#[test]
fn drawings_with_circles_and_curves_scale_and_move() {
    let mut d = Drawing::default();
    d.circle((0.0, 0.0), 10.0, 1.0, false);
    d.curve((0.0, 0.0), (10.0, 20.0), (20.0, 20.0), (30.0, 0.0), 1.0);
    let big = d.scaled(2.0);
    assert_eq!(big.shapes[0], Shape::Circle { center: (0.0, 0.0), radius: 20.0, width: 2.0, fill: false });
    assert_eq!(big.shapes[1], Shape::Curve { a: (0.0, 0.0), c1: (20.0, 40.0), c2: (40.0, 40.0), b: (60.0, 0.0), width: 2.0 });
    assert_eq!(big.bounds().width(), 2.0 * d.bounds().width());
    let mut moved = Drawing::default();
    moved.place(&d, (5.0, -5.0));
    assert_eq!(moved.shapes[0], Shape::Circle { center: (5.0, -5.0), radius: 10.0, width: 1.0, fill: false });
    assert_eq!(moved.shapes[1], Shape::Curve { a: (5.0, -5.0), c1: (15.0, 15.0), c2: (25.0, 15.0), b: (35.0, -5.0), width: 1.0 });
    assert_eq!(moved.bounds().x0, d.bounds().x0 + 5.0);
}

#[test]
fn drawings_with_dashes_and_polygons_scale() {
    let mut d = Drawing::default();
    d.dashed((0.0, 0.0), (10.0, 0.0), 1.0, (2.0, 1.0));
    d.polygon(vec![(0.0, 0.0), (4.0, 0.0), (0.0, 3.0)], true, 0.5);
    let half = d.scaled(0.5);
    assert_eq!(half.shapes[0], Shape::Line { a: (0.0, 0.0), b: (5.0, 0.0), width: 0.5, dash: Some((1.0, 0.5)) });
    assert_eq!(half.shapes[1], Shape::Polygon { points: vec![(0.0, 0.0), (2.0, 0.0), (0.0, 1.5)], fill: true, width: 0.25 });
}

#[test]
fn bonds_with_explicit_departure_and_arrival_pieces() {
    // [angle,length,departure,arrival]: counted from 1 in the labels' pieces
    let m = molecule(r"\chemfig{H_3C-[0,1,1,2]XY}");
    assert_eq!((m.bonds[0].departure, m.bonds[0].arrival), (0, 1));
    // 0 is no piece: the default (the last going right, the first arriving from the left)
    let m = molecule(r"\chemfig{H_3C-[0,1,0,0]XY}");
    assert_eq!((m.bonds[0].departure, m.bonds[0].arrival), (1, 0));
}

#[test]
fn names_and_boxes_given_inside_subschemes_are_the_schemes() {
    // a name given inside a \subscheme counts from the scheme's compounds: a \merge after it joins them
    let s = scheme(r"\schemestart X \arrow \subscheme{A \arrow(a--b) B} \merge>(a)(b)--(c) C \schemestop");
    let Some(Item::Merge(m)) = s.items.iter().find(|i| matches!(i, Item::Merge(_))) else { panic!("{:?}", s.items) };
    assert_eq!(m.from, vec![(1, None), (2, None)]);
    // a box inside a box: an arrow from the outer box (c1) leaves from both compounds
    let s = scheme(r"\schemestart \subscheme{\subscheme{A} \arrow B} \arrow(@c1--) C \schemestop");
    let Some(Item::Arrow(a)) = s.items.iter().rev().find(|i| matches!(i, Item::Arrow(_))) else { panic!() };
    assert_eq!(a.from_box, Some((0, 2)));
}

#[test]
fn a_merge_from_compounds_that_are_not_there_draws_nothing() {
    use context_equation_core::chem::model::{Compound, Merge};
    // the reader only joins compounds it has placed; a scheme built otherwise may name others
    let a = Item::Compound(Compound { body: Body::Math { tex: "A".into() }, name: None });
    let merge = Item::Merge(Merge { from: vec![(5, None)], angle: 0.0, above: None, below: None, coefficients: (0.5, 0.5, 0.5) });
    let draw = |items: Vec<Item>| {
        let mut registry = context_equation_core::rules::Registry::default();
        let mut ctx = context_equation_core::layout::Context { locale: Default::default(), registry: &mut registry, strict: true, kerning: Default::default(), fallbacks: &[] };
        context_equation_core::chem::layout::compose(&Scheme { items, moves: vec![] }, context_equation_core::layout::Style::display(), &mut ctx).unwrap()
    };
    let (with, without) = (draw(vec![a.clone(), merge]), draw(vec![a]));
    assert_eq!((with.width, with.items.len()), (without.width, without.items.len()));
}

#[test]
fn a_scheme_whose_math_cannot_be_read_is_read_as_written() {
    use context_equation_core::chem::model::Compound;
    use context_equation_core::chem::speech::{as_nodes, linear, speak};
    // \skew without a number is an error even in tolerant mode: the speech and the text keep the TeX,
    // braille an empty row
    let s = Scheme { items: vec![Item::Compound(Compound { body: Body::Math { tex: r"\skew{x}\hat{a}".into() }, name: None })], moves: vec![] };
    assert_eq!(speak(&s, context_equation_core::Locale::En), r"\skew{x}\hat{a}");
    assert_eq!(linear(&s, context_equation_core::Locale::En), r"\skew{x}\hat{a}");
    assert_eq!(as_nodes(&s), Node::row(vec![Node::row(vec![])]));
}

#[test]
fn a_label_counts_a_group_in_parentheses_as_many_times_as_its_number() {
    use context_equation_core::chem::model::Atom;
    let count = |label: &str| {
        let atom = Atom { label: label.to_string(), ..Atom::default() };
        context_equation_core::chem::formula::of(&Molecule { atoms: vec![atom], ..Molecule::default() }).written()
    };
    assert_eq!(count("(CH_2)_3CH_3"), "C₄H₉");
    assert_eq!(count("C(CH_3)_3"), "C₄H₉");
    // a ')' with no '(' before it counts nothing (#210)
    assert_eq!(count("CH_3)_2"), "CH₃");
}
