//! The stmaryrd and mathabx symbols Unicode has no character for (#176), drawn by composition from
//! the math font's glyphs as the packages' METAFONT sources build them (`layout/composite.rs`), and
//! the \varo… forms the font already draws.

use context_equation_core::composite::{self, Composite};
use context_equation_core::layout::{Item, MathBox};
use context_equation_core::tree::{Class, Node};
use context_equation_core::{convert, font, typeset, Options};

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

fn inline() -> Options {
    Options { strict: true, display: false, ..Options::default() }
}

fn only(node: &Node) -> &Node {
    match node {
        Node::Row { children } if children.len() == 1 => only(&children[0]),
        other => other,
    }
}

fn boxed(tex: &str) -> MathBox {
    typeset(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

/// Every item, the ones inside transforms and groups too.
fn items(b: &MathBox) -> Vec<Item> {
    fn walk(items: &[Item], out: &mut Vec<Item>) {
        for i in items {
            match i {
                Item::Transformed { items, .. } | Item::Group { items, .. } => walk(items, out),
                other => out.push(other.clone()),
            }
        }
    }
    let mut out = Vec::new();
    walk(&b.items, &mut out);
    out
}

fn lines(b: &MathBox) -> Vec<(f32, f32, f32, f32, f32)> {
    items(b)
        .into_iter()
        .filter_map(|i| match i {
            Item::Line { x1, y1, x2, y2, thickness, .. } => Some((x1, y1, x2, y2, thickness)),
            _ => None,
        })
        .collect()
}

fn glyphs(b: &MathBox) -> Vec<(u16, f32, f32)> {
    items(b)
        .into_iter()
        .filter_map(|i| match i {
            Item::Glyph { id, x, y, .. } => Some((id.0, x, y)),
            _ => None,
        })
        .collect()
}

fn id(c: char) -> u16 {
    font::font().glyph(c).unwrap_or_else(|| panic!("STIX Two Math has {c}")).0
}

fn ink(b: &MathBox) -> (f32, f32, f32, f32) {
    b.ink(font::font()).expect("ink")
}

/// The font's rule thickness and x-height, in font units (STIX Two Math: 68 and 473).
const RULE: f32 = 68.0;
const X_HEIGHT: f32 = 473.0;

fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn every_symbol_reads_in_its_class_in_strict_mode() {
    for k in composite::ALL {
        let tex = composite::source(k);
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        let expected = match k.class {
            Class::Op => Node::LargeOperator { symbol: k.symbol, limits: None },
            class => Node::Operator { symbol: k.symbol, class },
        };
        assert_eq!(only(&out.tree), &expected, "{tex}");
        let tolerant = convert(&tex, &Options::default()).unwrap();
        assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
        // drawn: ink, and no box for a missing character
        let b = boxed(&tex);
        assert!(b.ink(font::font()).is_some(), "{tex}: nothing drawn");
        assert!(!glyphs(&b).iter().any(|g| g.0 == id('□')), "{tex}: drawn as a box");
        // the marker stays in the tree (and the tree JSON the SVG carries), not in what is read
        for text in [&out.text, &out.speech, &out.braille, &out.mathml] {
            assert!(!text.contains(k.symbol), "{tex}: the marker leaked into {text}");
        }
    }
}

#[test]
fn the_classes_are_the_packages() {
    let class = |name: &str| composite::command(name).map(|k| k.class);
    // stmaryrd.dtx: \llfloor \mathopen, \rrceil \mathclose, \varovee \mathbin, \Yleft \mathbin,
    // \shortuparrow \mathrel, \inplus \mathrel, \bigbox \mathop; mathabx.dcl: \bigoslash {1}, \mathop
    for (name, expected) in [
        ("llfloor", Class::Open),
        ("rrceil", Class::Close),
        ("varovee", Class::Bin),
        ("Yleft", Class::Bin),
        ("shortuparrow", Class::Rel),
        ("inplus", Class::Rel),
        ("arrownot", Class::Rel),
        ("bigbox", Class::Op),
        ("bigoslash", Class::Op),
    ] {
        assert_eq!(class(name), Some(expected), "\\{name}");
    }
}

#[test]
fn the_var_circles_the_font_already_draws_are_its_characters() {
    // stmaryrd's \varo… are its circle operators drawn circular and with the rule's pen; the font's are
    for (name, symbol) in [
        ("varotimes", '⊗'),
        ("varoplus", '⊕'),
        ("varominus", '⊖'),
        ("varodot", '⊙'),
        ("varoslash", '⊘'),
        ("varobslash", '⦸'),
        ("varobar", '⦶'),
        ("varoast", '⊛'),
        ("varbigcirc", '◯'),
    ] {
        let tex = format!("\\{name}");
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert_eq!(only(&out.tree), &Node::Operator { symbol, class: Class::Bin }, "{tex}");
        // circular, with the ring as thick as the rule
        let b = boxed(&tex);
        let (x0, y0, x1, y1) = ink(&b);
        assert!(close(x1 - x0, y1 - y0, 2.0), "{tex}: not circular");
    }
    let out = convert(r"f \curvearrowbotright g", &strict()).unwrap();
    assert!(format!("{:?}", out.tree).contains("symbol: '⤻', class: Rel"), "{:?}", out.tree);
}

#[test]
fn a_doubled_ceiling_is_two_ceilings_a_kern_apart() {
    for (name, single) in [("llceil", '⌈'), ("rrceil", '⌉'), ("llfloor", '⌊'), ("rrfloor", '⌋')] {
        let tex = format!("\\{name}");
        let b = boxed(&tex);
        let g = glyphs(&b);
        assert_eq!(g.len(), 2, "{tex}: {g:?}");
        assert!(g.iter().all(|x| x.0 == id(single)), "{tex}");
        // stmaryaj.mf: the stems `spread` apart, 0.45 of the x-height
        assert!(close(g[1].1 - g[0].1, 0.45 * X_HEIGHT, 0.5), "{tex}: {g:?}");
        assert_eq!(g[0].2, g[1].2, "{tex}");
        let one = boxed(&single.to_string());
        assert!(close(b.width, one.width + 0.45 * X_HEIGHT, 0.5), "{tex}");
    }
}

#[test]
fn the_doubled_ceilings_grow_after_left_and_right() {
    let tall = boxed(r"\left\llceil \frac{\frac{a}{b}}{c} \right\rrceil");
    let small = boxed(r"\llceil \frac{\frac{a}{b}}{c} \rrceil");
    assert!(tall.height + tall.depth > small.height + small.depth, "{} vs {}", tall.height + tall.depth, small.height + small.depth);
    let out = convert(r"\left\llfloor x \right\rrfloor", &strict()).unwrap();
    assert!(matches!(only(&out.tree), Node::Delimited { open: Some(composite::LLFLOOR), close: Some(composite::RRFLOOR), .. }), "{:?}", out.tree);
    // \big too
    assert!(convert(r"\bigl\llceil x \bigr\rrceil", &strict()).is_ok());
}

#[test]
fn the_circular_forms_draw_stmaryrds_strokes_inside_the_circle() {
    let ring = boxed(r"\varovee");
    let (x0, y0, x1, y1) = ink(&ring);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let r = (x1 - x0 - RULE) / 2.0;
    // z1--z4--z7: from 45° down to the bottom of the circle and up to 135°, at the rule's thickness
    let l = lines(&ring);
    assert_eq!(l.len(), 2);
    assert!(l.iter().all(|x| x.4 == RULE));
    assert!(close(l[0].0, cx + r * 0.5f32.sqrt(), 1.0) && close(l[0].1, cy + r * 0.5f32.sqrt(), 1.0), "{l:?}");
    assert!(close(l[0].2, cx, 1.0) && close(l[0].3, cy - r, 1.0), "{l:?}");
    assert_eq!(glyphs(&ring)[0].0, id('○'));
    // var_o_circle: the inner circle at half the radius
    let inner = items(&boxed(r"\varocircle")).into_iter().find_map(|i| match i {
        Item::Ellipse { rx, ry, thickness, .. } => Some((rx, ry, thickness)),
        _ => None,
    });
    let (rx, ry, t) = inner.expect("an inner circle");
    assert!(close(rx, r / 2.0, 1.0) && rx == ry && t == RULE);
    // z1--z6--z3: the less-than touches the circle on the left
    let less = lines(&boxed(r"\varolessthan"));
    assert!(close(less[0].2, cx - r, 1.0), "{less:?}");
}

/// The angle of a turned box's transform, in degrees.
fn turn(b: &MathBox) -> f32 {
    b.items
        .iter()
        .find_map(|i| match i {
            Item::Transformed { matrix, .. } => Some(matrix[1].atan2(matrix[0]).to_degrees()),
            _ => None,
        })
        .expect("a turned box")
}

#[test]
fn the_y_forms_are_yup_turned() {
    for (name, degrees) in [("Ydown", 180.0f32), ("Yleft", 90.0), ("Yright", -90.0)] {
        let b = boxed(&format!("\\{name}"));
        assert!(close(turn(&b).rem_euclid(360.0), degrees.rem_euclid(360.0), 0.01), "\\{name}");
        assert_eq!(glyphs(&b)[0].0, id('⅄'));
    }
    // \Yleft and \Yright centered on the axis (stmaryjg.mf: 5u+math_axis over, 5u-math_axis under)
    let (_, y0, _, y1) = ink(&boxed(r"\Yleft"));
    assert!(close((y0 + y1) / 2.0, 258.0, 1.0));
}

#[test]
fn the_short_arrows_are_three_quarters_of_the_arrow() {
    let long = ink(&boxed(r"\rightarrow"));
    let short = ink(&boxed(r"\shortrightarrow"));
    assert!(close(short.2 - short.0, 0.75 * (long.2 - long.0), 1.0), "{short:?} vs {long:?}");
    // the same head: as tall as →'s
    assert!(close(short.3 - short.1, long.3 - long.1, 1.0));
    let left = ink(&boxed(r"\shortleftarrow"));
    assert!(close(left.2 - left.0, short.2 - short.0, 1.0));
    // the vertical ones are it turned, centered on the axis
    for (name, degrees) in [("shortuparrow", 90.0f32), ("shortdownarrow", -90.0)] {
        let b = boxed(&format!("\\{name}"));
        assert!(close(turn(&b), degrees, 0.01), "\\{name}");
        let (_, y0, _, y1) = ink(&b);
        assert!(close(y1 - y0, short.2 - short.0, 1.0), "\\{name}");
        assert!(close((y0 + y1) / 2.0, 258.0, 1.0), "\\{name}");
    }
    // stmaryaj.mf's north-north-east arrow: 7u across, 16u up
    assert!(close(turn(&boxed(r"\nnearrow")), 16f32.atan2(7.0).to_degrees(), 0.01));
}

#[test]
fn arrownot_and_mapsfromchar_take_no_width_and_build_on_the_arrow() {
    let not = boxed(r"\arrownot");
    assert_eq!(not.width, 0.0);
    let arrow = boxed(r"\rightarrow");
    let l = lines(&not);
    assert_eq!(l.len(), 1);
    // over the middle of the arrow that follows
    assert!(close((l[0].0 + l[0].2) / 2.0, arrow.width / 2.0, 0.5));
    let with = boxed(r"\arrownot\rightarrow");
    assert!(close(with.width, arrow.width, 0.5), "no space between the two relations");
    let from = boxed(r"\mapsfromchar");
    assert_eq!(from.width, 0.0);
    // \leftarrow\mapsfromchar is ↤: the stem at the tail of the arrow
    let stem = lines(&from)[0];
    assert!(stem.0 < 0.0 && stem.0 == stem.2, "{stem:?}");
    let pair = boxed(r"\leftarrow\mapsfromchar");
    let (_, _, x1, _) = ink(&pair);
    assert!(close(x1, boxed(r"\leftarrow").width - (boxed(r"\leftarrow").width - ink(&boxed(r"\leftarrow")).2), 0.5 + RULE / 2.0 + 1.0));
}

#[test]
fn trianglelefteqslant_has_the_lower_side_drawn_again_below() {
    let b = boxed(r"\trianglelefteqslant");
    let l = lines(&b);
    assert_eq!(l.len(), 1);
    let (x1, y1, x2, y2, _) = l[0];
    // from under the right corner to under the point, going up to the left: parallel to the lower side
    assert!(x1 > x2 && y2 > y1, "{l:?}");
    // the whole centered on the axis (v_center in stmaryaj.mf)
    let (_, b0, _, b1) = ink(&b);
    assert!(close((b0 + b1) / 2.0, 258.0, 1.0));
    // spread' below: 0.45 of the x-height
    let g = glyphs(&b)[0];
    let tri = ink(&boxed("⊲"));
    assert!(close((tri.1 + g.2 + RULE / 2.0) - y1, 0.45 * X_HEIGHT, 1.0), "{l:?} {tri:?} {g:?}");
}

#[test]
fn inplus_is_in_with_a_bar_across_its_middle() {
    let b = boxed(r"\inplus");
    let (x0, y0, x1, y1) = ink(&boxed(r"\in"));
    let l = lines(&b);
    assert_eq!(l.len(), 1);
    let (x, top, x2, bottom, _) = l[0];
    assert_eq!(x, x2);
    assert!(x > x0 && x < x1);
    assert!(close((top + bottom) / 2.0, (y0 + y1) / 2.0, 1.0));
    assert!(top - bottom < y1 - y0);
}

#[test]
fn the_big_operators_grow_in_display_and_take_limits_under() {
    for k in composite::ALL.iter().filter(|k| k.class == Class::Op) {
        let name = k.command;
        let display = boxed(&format!("\\{name}"));
        let text = typeset(&format!("\\{name}"), &inline()).unwrap().0;
        assert!(display.height + display.depth > text.height + text.depth, "\\{name}");
        let tex = format!("\\{name}_{{i=1}}^{{n}} A_i");
        let side = format!("\\{name}\\nolimits_{{i=1}}^{{n}} A_i");
        assert!(boxed(&tex).depth > boxed(&side).depth + 0.1, "\\{name}");
        // centered on the axis as the font's big operators are (whose display sizes in STIX Two Math
        // start a little above their baseline, so their ink sits up to 33 units above the axis)
        let (_, y0, _, y1) = ink(&display);
        assert!(close((y0 + y1) / 2.0, 258.0, 40.0), "\\{name}: {}", (y0 + y1) / 2.0);
    }
}

#[test]
fn the_big_operators_are_built_from_the_fonts() {
    // \bigbox: ⨅ and ⨆ over each other
    let g: Vec<u16> = glyphs(&boxed(r"\bigbox")).iter().map(|g| g.0).collect();
    assert_eq!(g.len(), 2);
    let cap = ink(&boxed(r"\bigsqcap"));
    let b = ink(&boxed(r"\bigbox"));
    assert!(close(b.3 - b.1, cap.3 - cap.1, 1.0) && close(b.2 - b.0, cap.2 - cap.0, 1.0));
    // \bigboxplus: and a plus across it
    assert_eq!(lines(&boxed(r"\bigboxplus")).len(), 2);
    // \bigparallel: two legs as tall as ⨅'s, 8u apart where those are 18u apart (display)
    let rules: Vec<(f32, f32)> = items(&boxed(r"\bigparallel"))
        .into_iter()
        .filter_map(|i| match i {
            Item::Rule { x, height, .. } => Some((x, height)),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 2);
    assert!(close(rules[0].1, cap.3 - cap.1, 1.0));
    // \bignplus: ⨄ upside down
    let n = boxed(r"\bignplus");
    assert!(n.items.iter().any(|i| matches!(i, Item::Transformed { matrix, .. } if matrix[3] == -1.0)));
    // \bigcurlyvee: in the box of ⋁
    let vee = ink(&boxed(r"\bigvee"));
    let curly = ink(&boxed(r"\bigcurlyvee"));
    assert!(close(curly.3 - curly.1, vee.3 - vee.1, 2.0) && close(curly.2 - curly.0, vee.2 - vee.0, 2.0), "{curly:?} {vee:?}");
    // \bigominus and \bigoslash: the circle of ⨁ and one stroke
    let plus = ink(&boxed(r"\bigoplus"));
    for name in ["bigominus", "bigoslash"] {
        let b = boxed(&format!("\\{name}"));
        assert!(close(ink(&b).2 - ink(&b).0, plus.2 - plus.0, 1.0), "\\{name}");
        assert_eq!(lines(&b).len(), 1, "\\{name}");
    }
}

fn spoken(tex: &str) -> String {
    convert(tex, &strict()).unwrap().speech
}

#[test]
fn every_output_writes_the_closest_text() {
    let out = convert(r"\llceil x \rrceil + \llfloor y \rrfloor", &strict()).unwrap();
    assert_eq!(out.text, "⌈⌈x⌉⌉ + ⌊⌊y⌋⌋");
    // MathML: the S-MML-1 fallback, with the closest drawing and the source
    assert!(out.mathml.contains("<semantics>") && out.mathml.contains("<mo>⌈⌈</mo>") && out.mathml.contains(r"\llceil x"), "{}", out.mathml);
    assert!(out.applied.iter().any(|a| a.rule == "S-MML-1"));
    // braille: the full cell, with a note naming the command
    assert!(out.braille.contains('⠿'));
    assert!(out.applied.iter().any(|a| a.rule == "S-BRL-1" && a.description.contains(r"\llceil")), "{:?}", out.applied);
    // no marker in any of them
    for text in [&out.text, &out.speech, &out.mathml, &out.braille] {
        assert!(!text.chars().any(composite::is), "{text}");
    }
    // a short arrow is read as the arrow, and has its braille sign
    assert_eq!(spoken(r"a \shortrightarrow b"), spoken(r"a \rightarrow b"));
    let short = convert(r"a \shortrightarrow b", &strict()).unwrap();
    assert_eq!(short.braille, convert(r"a \rightarrow b", &strict()).unwrap().braille);
    assert_eq!(short.text, "a → b");
    // a big operator as one
    assert_eq!(spoken(r"\bigbox_i P_i"), spoken(r"\bigsqcap_i P_i"));
}

#[test]
fn the_issue_examples_read_without_warnings() {
    for tex in [
        r"\llceil x \rrceil + \llfloor y \rrfloor",
        r"a \varotimes b",
        r"\bigbox_{i} P_i",
        r"x \shortrightarrow y",
        r"\left\llbracket \bigbox_{i\inplus I}^{a \varoplus b} P_i \right\rrbracket",
    ] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
    }
}

#[test]
fn the_table_has_no_marker_twice() {
    let all: Vec<&Composite> = composite::ALL.iter().collect();
    for (i, a) in all.iter().enumerate() {
        assert!(all[i + 1..].iter().all(|b| b.command != a.command), "\\{}", a.command);
        assert!(font::font().glyph(a.symbol).is_none(), "the font draws the marker of \\{}", a.command);
    }
}

/// #207: the tree's JSON (`--tree`, the SVG's metadata) names a symbol drawn by composition by its command,
/// not by its private marker; the MathML fallback keeps the command in its annotation, and `Node` keeps
/// its size.
#[test]
fn the_tree_names_the_symbol_by_its_command() {
    let private = |s: &str| s.chars().any(composite::is);
    for options in [strict(), Options::default()] {
        let out = convert(r"\llceil x \rrceil", &options).unwrap();
        let json = out.tree.to_json();
        assert!(json.contains(r#""symbol":"\\llceil""#) && json.contains(r#""symbol":"\\rrceil""#), "{json}");
        assert!(!private(&json) && !json.contains(r"\u{f0b") && !json.contains(r"\uDB82"), "{json}");
        assert!(!private(&out.svg), "the SVG's metadata");
        assert!(out.svg.contains(r"&quot;symbol&quot;:&quot;\\llceil&quot;"), "{}", out.svg);
        assert!(out.mathml.contains(r"\llceil x \rrceil"), "{}", out.mathml);
    }
    // every place a marker can be: an operator, a large operator, \left, \middle, \right and \big
    let json = convert(r"\left\llfloor x \middle\llceil y \right\rrfloor + \bigbox_a b \big\llceil \varovee", &strict()).unwrap().tree.to_json();
    for k in [
        r#""open":"\\llfloor""#,
        r#""close":"\\rrfloor""#,
        r#""middles":["\\llceil"]"#,
        r#""symbol":"\\bigbox""#,
        r#""type":"fixed-delimiter","symbol":"\\llceil""#,
        r#""symbol":"\\varovee""#,
    ] {
        assert!(json.contains(k), "{k} in {json}");
    }
    assert!(!private(&json), "{json}");
    // every composite symbol, and the others still their character
    for k in composite::ALL {
        let json = convert(&composite::source(k), &strict()).unwrap().tree.to_json();
        assert!(json.contains(&format!(r#""symbol":"\\{}""#, k.command)), "{json}");
    }
    assert!(convert(r"\lceil x \rceil", &strict()).unwrap().tree.to_json().contains(r#""symbol":"⌈""#));
    assert_eq!(std::mem::size_of::<Node>(), 176);
}
