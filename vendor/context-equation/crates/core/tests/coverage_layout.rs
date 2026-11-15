//! Layout corners the other suites leave alone: graphicx's rotation origins and its `!` sizes,
//! `\mathchoice` without the branch of its style, a word with a letter the font lacks, small
//! capitals of a letter whose capital is missing, marks under the base drawn only with rules,
//! dashed table rules shorter than a dash, the arrows built from two stretched ones, the moustaches
//! and other delimiters of a font that lacks them (Latin Modern Math, loaded as a font of the
//! caller's), and the box operations (`\color`, `\scalebox`, `\reflectbox`) over turned, grouped,
//! crossed and circled content. Each case checks a measure or the drawing, not just that it runs.

use context_equation_core::font::MathFont;
use context_equation_core::layout::{compose, Context, Item, MathBox, Style};
use context_equation_core::rules::Registry;
use context_equation_core::tree::{BoxLength, Node, TableKind, TableRule, Variant};
use context_equation_core::{Class, Error, Locale, Options};

fn options() -> Options {
    Options { locale: Locale::En, fallback_fonts: Vec::new(), ..Options::default() }
}

fn math_box(tex: &str) -> MathBox {
    context_equation_core::typeset(tex, &options()).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn warnings(tex: &str) -> Vec<String> {
    context_equation_core::typeset(tex, &options()).unwrap().1.into_iter().map(|a| a.rule.to_string()).collect()
}

/// Composes a hand-built tree, with the warnings it gave.
fn compose_node(node: &Node, e: Style, strict: bool) -> (Result<MathBox, Error>, Vec<String>) {
    let mut registry = Registry::default();
    let mut ctx = Context { locale: Locale::En, registry: &mut registry, strict, kerning: Default::default(), fallbacks: &[] };
    let r = compose(node, e, &mut ctx);
    let warnings = registry.warnings.iter().map(|a| a.rule.to_string()).collect();
    (r, warnings)
}

fn ident(c: &str) -> Node {
    Node::Identifier { name: c.into(), variant: Variant::Italic }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// Every item, the ones inside groups and transforms too.
fn flatten(items: &[Item]) -> Vec<&Item> {
    let mut all = Vec::new();
    for item in items {
        all.push(item);
        if let Item::Transformed { items, .. } | Item::Group { items, .. } = item {
            all.extend(flatten(items));
        }
    }
    all
}

/// A lowercase letter STIX Two Math does not have, nor its capital (Armenian ayb, Ա).
const ABSENT: char = 'ա';

#[test]
fn the_absent_letter_and_its_capital_are_not_in_the_font() {
    let f = MathFont::StixTwo.get();
    assert!(ABSENT.is_lowercase());
    assert!(f.glyph(ABSENT).is_none() && f.glyph('Ա').is_none());
}

// ---------- graphicx ----------

/// `\rotatebox{180}` about the right end of the bottom (`origin=rb`) puts the whole box below the
/// bottom it turned on; about the baseline's middle (`origin=B`) it swaps height and depth.
#[test]
fn rotation_origins_right_bottom_and_baseline() {
    let plain = math_box(r"\text{y}");
    let (h, d, w) = (plain.height, plain.depth, plain.width);
    assert!(d > 0.0, "y has a depth");

    let rb = math_box(r"\rotatebox[origin=rb]{180}{y}");
    assert!(near(rb.width, w), "{} {w}", rb.width);
    assert!(near(rb.height, 0.0), "turned below its bottom: {}", rb.height);
    assert!(near(rb.depth, h + 2.0 * d), "{} {}", rb.depth, h + 2.0 * d);

    let baseline = math_box(r"\rotatebox[origin=B]{180}{y}");
    assert!(near(baseline.height, d) && near(baseline.depth, h), "{baseline:?}");
    assert!(near(baseline.width, w));
    assert!(matches!(baseline.items[0], Item::Transformed { .. }));
}

/// `\resizebox{!}{!}{…}` asks for no size: the box keeps its measures.
#[test]
fn resizebox_with_no_size_leaves_the_box_alone() {
    let plain = math_box(r"\text{x}");
    let same = math_box(r"\resizebox{!}{!}{x}");
    assert!(near(same.width, plain.width) && near(same.height, plain.height) && near(same.depth, plain.depth), "{same:?} {plain:?}");
    // a size that cannot be reached (an empty box) is not a size either
    let empty = Node::Resized { width: Some(BoxLength { em: 2.0, ..Default::default() }), height: None, total: false, base: Box::new(Node::row(vec![])) };
    let (b, _) = compose_node(&empty, Style::display(), true);
    let b = b.unwrap();
    assert_eq!((b.width, b.height, b.depth), (0.0, 0.0, 0.0));
}

// ---------- \mathchoice ----------

/// A `\mathchoice` with fewer branches than its style's index sets nothing.
#[test]
fn mathchoice_without_the_branch_of_its_style_is_empty() {
    let choice = Node::Choice { branches: vec![ident("a")] };
    let (display, _) = compose_node(&choice, Style::display(), true);
    assert!(display.unwrap().width > 0.0, "the display branch is there");
    let (text, _) = compose_node(&choice, Style::text(), true);
    let text = text.unwrap();
    assert_eq!(text.width, 0.0);
    assert!(text.items.is_empty());
}

// ---------- missing glyphs ----------

/// A word with a letter the font lacks is set letter by letter: the letters it has as themselves,
/// the missing one as a box with a warning (E-REC-8), or refused in strict mode.
#[test]
fn word_with_a_missing_letter_draws_a_box_for_that_letter_only() {
    let word = Node::Identifier { name: format!("a{ABSENT}b"), variant: Variant::Italic };
    let (b, warned) = compose_node(&word, Style::display(), false);
    let b = b.unwrap();
    assert_eq!(warned, ["E-REC-8"]);
    let glyphs = b.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    assert_eq!(glyphs, 3, "a, the box and b");
    let width = |n: &Node| compose_node(n, Style::display(), true).0.unwrap().width;
    let expected = width(&ident("a")) + width(&Node::Operator { symbol: '□', class: Class::Ord }) + width(&ident("b"));
    assert!(near(b.width, expected), "{} {expected}", b.width);

    let (strict, _) = compose_node(&word, Style::display(), true);
    assert!(matches!(strict, Err(Error::MissingGlyph { character }) if character == ABSENT), "{strict:?}");
}

/// In `\textsc`, a lowercase letter whose capital the font lacks is not shrunk: it goes on as the
/// letter itself, here a box with a warning.
#[test]
fn small_capital_of_a_letter_without_its_capital_falls_back_to_the_letter() {
    let tex = format!(r"\textsc{{{ABSENT}}}");
    assert_eq!(warnings(&tex), ["E-REC-8"]);
    let sc = math_box(&tex);
    let plain = math_box(&format!(r"\text{{{ABSENT}}}"));
    assert!(near(sc.width, plain.width) && near(sc.height, plain.height), "{sc:?} {plain:?}");
}

/// A digit the font has in no form (Devanagari one, in bold: neither 𝟏-like form nor the plain
/// one) is a box with a warning, or refused in strict mode.
#[test]
fn digit_the_font_lacks_in_any_form_is_a_box() {
    let digit = '१';
    assert!(MathFont::StixTwo.get().glyph(digit).is_none());
    let number = Node::Number { value: digit.to_string(), group: false, thousands: None, variant: Variant::Bold };
    let (b, warned) = compose_node(&number, Style::display(), false);
    assert_eq!(warned, ["E-REC-8"]);
    let square = compose_node(&Node::Operator { symbol: '□', class: Class::Ord }, Style::display(), true).0.unwrap();
    assert!(near(b.unwrap().width, square.width));
    let (strict, _) = compose_node(&number, Style::display(), true);
    assert!(matches!(strict, Err(Error::MissingGlyph { character }) if character == digit), "{strict:?}");
}

/// A delimiter the font lacks, not one of the doubled ones nor a moustache, is a box.
#[test]
fn fixed_delimiter_the_font_lacks_is_a_box() {
    let delimiter = Node::FixedDelimiter { symbol: ABSENT, size: 2, class: Class::Open };
    let (b, warned) = compose_node(&delimiter, Style::display(), false);
    assert_eq!(warned, ["E-REC-8"]);
    assert!(b.unwrap().width > 0.0);
    let (strict, _) = compose_node(&delimiter, Style::display(), true);
    assert!(matches!(strict, Err(Error::MissingGlyph { character }) if character == ABSENT));
}

// ---------- marks under the base ----------

/// An accent under the base whose mark has no glyph (mhchem's dashed bond, drawn with rules) is
/// placed by its box: centered under the base, below it, and the box deepens to hold it.
#[test]
fn mark_drawn_with_rules_goes_centered_under_the_base() {
    let base = compose_node(&ident("x"), Style::display(), true).0.unwrap();
    let node = Node::Accent { base: Box::new(ident("x")), accent: context_equation_core::composite::BOND_DASHED, stretchy: false, below: true };
    let b = compose_node(&node, Style::display(), true).0.unwrap();
    let rules: Vec<(f32, f32, f32)> = b.items.iter().filter_map(|i| if let Item::Rule { x, y, width, .. } = i { Some((*x, *y, *width)) } else { None }).collect();
    assert_eq!(rules.len(), 3, "the three dashes");
    assert!(rules.iter().all(|r| r.1 < -base.depth), "{rules:?} under the base ({})", base.depth);
    let left = rules.iter().map(|r| r.0).fold(f32::INFINITY, f32::min);
    let right = rules.iter().map(|r| r.0 + r.2).fold(f32::NEG_INFINITY, f32::max);
    // the mark's box (the font's minus) is centered on the base, so its ink nearly is
    assert!((left + right - base.width).abs() < 0.2 * base.width, "{left} {right} {}", base.width);
    assert!(b.depth > base.depth && near(b.height, base.height) && near(b.width, base.width));
}

// ---------- dashed rules ----------

/// An array of one column with no `\arraycolsep` and a dashed rule over it, as wide as its cell.
fn dashed_table(cell: Node) -> Node {
    Node::Table {
        shape: TableKind::Array,
        rows: vec![vec![cell]],
        columns: vec!['c'],
        verticals: vec![],
        dashed: vec![],
        horizontals: vec![TableRule { row: 0, from: 0, to: usize::MAX, dashed: true }],
        between: vec![],
        intertexts: vec![],
        stretch: 1.0,
        colsep: 0,
    }
}

fn horizontal_rules(b: &MathBox) -> Vec<(f32, f32)> {
    b.items.iter().filter_map(|i| if let Item::Rule { x, width, height, .. } = i { (width > height).then_some((*x, *width)) } else { None }).collect()
}

/// A dashed rule shorter than one dash (4pt) is one dash, as long as the rule; one too short for a
/// dash, a gap and a dash is one dash too. A longer one is dashes and gaps that start and end with
/// a dash.
#[test]
fn dashed_rules_too_short_for_two_dashes_are_one() {
    let em = MathFont::StixTwo.get().units_per_em();
    let dash = 0.4 * em;
    for (cell, shortest, longest) in [(Node::Operator { symbol: '.', class: Class::Ord }, 0.0, dash), (ident("x"), dash, 2.0 * dash)] {
        let b = compose_node(&dashed_table(cell), Style::display(), true).0.unwrap();
        assert!(b.width > shortest && b.width < longest, "{} not in ({shortest}, {longest})", b.width);
        let rules = horizontal_rules(&b);
        assert_eq!(rules.len(), 1, "{rules:?}");
        assert!(near(rules[0].0, 0.0) && near(rules[0].1, b.width), "{rules:?} {}", b.width);
    }
    let long = compose_node(&dashed_table(ident("xxxxxxxx")), Style::display(), true).0.unwrap();
    let rules = horizontal_rules(&long);
    assert!(rules.len() > 2, "{rules:?}");
    assert!(near(rules[0].0, 0.0) && near(rules.last().map(|r| r.0 + r.1).unwrap(), long.width));
}

// ---------- the operations of a box over its kinds of items ----------

/// `\color` reaches inside a turned box and a group of `\class`.
#[test]
fn color_reaches_inside_transforms_and_groups() {
    for tex in [r"\color{red}{\rotatebox{90}{x}}", r"\color{red}{\class{a}{x}}"] {
        let b = math_box(tex);
        assert!(matches!(b.items[0], Item::Transformed { .. } | Item::Group { .. }), "{tex}: {:?}", b.items);
        let glyphs: Vec<&Item> = flatten(&b.items).into_iter().filter(|i| matches!(i, Item::Glyph { .. })).collect();
        assert!(!glyphs.is_empty());
        for g in glyphs {
            assert!(matches!(g, Item::Glyph { color: Some(_), .. }), "{tex}: {g:?}");
        }
    }
}

/// `\scalebox{2}` doubles a turned box's map and the glyphs inside a group.
#[test]
fn scalebox_scales_transforms_and_groups() {
    let turned = math_box(r"\rotatebox{90}{x}");
    let twice = math_box(r"\scalebox{2}{$\rotatebox{90}{x}$}");
    let matrix = |b: &MathBox| flatten(&b.items).into_iter().find_map(|i| if let Item::Transformed { matrix, .. } = i { Some(*matrix) } else { None }).unwrap();
    for (a, b) in matrix(&turned).iter().zip(matrix(&twice)) {
        assert!((2.0 * a - b).abs() < 1e-3, "{:?} {:?}", matrix(&turned), matrix(&twice));
    }
    assert!(near(twice.width, 2.0 * turned.width) && near(twice.height, 2.0 * turned.height));

    let scale = |b: &MathBox| flatten(&b.items).into_iter().find_map(|i| if let Item::Glyph { scale, .. } = i { Some(*scale) } else { None }).unwrap();
    let grouped = math_box(r"\class{a}{x}");
    let grouped_twice = math_box(r"\scalebox{2}{$\class{a}{x}$}");
    assert!(matches!(grouped_twice.items[0], Item::Group { .. }));
    assert!((scale(&grouped_twice) - 2.0 * scale(&grouped)).abs() < 1e-4);
}

/// `\reflectbox` mirrors a cancel's line, a circle, a turned box and a group inside the box's width.
#[test]
fn reflectbox_mirrors_lines_circles_transforms_and_groups() {
    let line = |b: &MathBox| b.items.iter().find_map(|i| if let Item::Line { x1, x2, .. } = i { Some((*x1, *x2)) } else { None }).unwrap();
    let cancel = math_box(r"\cancel{x}");
    let mirrored = math_box(r"\reflectbox{$\cancel{x}$}");
    let ((a1, a2), (b1, b2)) = (line(&cancel), line(&mirrored));
    assert!(near(b1, cancel.width - a1) && near(b2, cancel.width - a2), "{a1} {a2} / {b1} {b2}");

    let center = |b: &MathBox| b.items.iter().find_map(|i| if let Item::Ellipse { cx, .. } = i { Some(*cx) } else { None }).unwrap();
    let circled = math_box(r"\textcircled{x}");
    let mirrored = math_box(r"\reflectbox{$\textcircled{x}$}");
    assert!(near(center(&mirrored), circled.width - center(&circled)));

    let turned = math_box(r"\rotatebox{90}{x}");
    let mirrored = math_box(r"\reflectbox{$\rotatebox{90}{x}$}");
    let matrix = |b: &MathBox| b.items.iter().find_map(|i| if let Item::Transformed { matrix, .. } = i { Some(*matrix) } else { None }).unwrap();
    let (m, r) = (matrix(&turned), matrix(&mirrored));
    assert!(near(r[0], -m[0]) && near(r[2], -m[2]) && near(r[4], turned.width - m[4]) && near(r[1], m[1]) && near(r[3], m[3]), "{m:?} {r:?}");

    let x_of = |b: &MathBox| flatten(&b.items).into_iter().find_map(|i| if let Item::Glyph { x, flip, .. } = i { Some((*x, *flip)) } else { None }).unwrap();
    let grouped = math_box(r"\class{a}{x}");
    let mirrored = math_box(r"\reflectbox{$\class{a}{x}$}");
    let ((x, flip), (mx, mflip)) = (x_of(&grouped), x_of(&mirrored));
    assert!(near(mx, grouped.width - x) && flip != mflip);
}

/// Mirroring a glyph's box moves its accent point to the same distance from the other side.
#[test]
fn mirroring_moves_the_accent_point_across() {
    let mut b = compose_node(&ident("f"), Style::display(), true).0.unwrap();
    let point = b.accent_point.expect("a letter has an accent point");
    b.mirror();
    assert_eq!(b.accent_point, Some(b.width - point));
    assert_eq!(b.italic, 0.0);
    assert!(!b.single_glyph);
}

/// A turned or grouped box with nothing drawn has no ink; with something drawn, its ink is the
/// turned one.
#[test]
fn ink_of_a_turned_box_with_nothing_drawn_is_none() {
    let f = MathFont::StixTwo.get();
    assert_eq!(math_box(r"\rotatebox{90}{$\phantom{x}$}").ink(f), None);
    assert_eq!(math_box(r"\class{a}{\phantom{x}}").ink(f), None);
    let (x0, y0, x1, y1) = math_box(r"\rotatebox{90}{$x$}").ink(f).unwrap();
    let (a0, b0, a1, b1) = math_box("x").ink(f).unwrap();
    // a quarter turn: the width of the ink becomes its height
    assert!(((x1 - x0) - (b1 - b0)).abs() < 1.0 && ((y1 - y0) - (a1 - a0)).abs() < 1.0);
}

// ---------- text kerning ----------

/// Bold text in T1 is kerned with the bold T1 table: `AV` closer than unkerned.
#[test]
fn bold_text_kerned_with_t1() {
    use context_equation_core::layout::kerning::{kern, TextKerning};
    let t1 = Options { text_kerning: TextKerning::T1, ..options() };
    let plain = math_box(r"\textbf{AV}").width;
    let kerned = context_equation_core::typeset(r"\textbf{AV}", &t1).unwrap().0.width;
    let k = kern(TextKerning::T1, true, 'A', 'V');
    assert!(k < 0.0);
    let em = MathFont::StixTwo.get().units_per_em();
    assert!(near(kerned, plain + k * em), "{kerned} {plain} {k}");
}

// ---------- arrows built from others ----------

/// An arrow over a label, the label a row of `n` letters.
fn arrow(symbol: char, n: usize) -> MathBox {
    let label = Node::row(vec![ident("x"); n]);
    let node = Node::Arrow { symbol, above: Some(Box::new(label)), below: None };
    compose_node(&node, Style::display(), true).0.unwrap_or_else(|e| panic!("{symbol}: {e}"))
}

fn glyph_ys(b: &MathBox) -> Vec<f32> {
    b.items.iter().filter_map(|i| if let Item::Glyph { y, .. } = i { Some(*y) } else { None }).collect()
}

/// The long arrows STIX Two Math draws at one size are their short arrow stretched: under a long
/// label, `⟶` is the same drawing as `→`.
#[test]
fn long_arrows_stretch_as_their_short_ones() {
    for (long, short) in [('⟶', '→'), ('⟵', '←'), ('⟷', '↔'), ('⟹', '⇒'), ('⟸', '⇐'), ('⟺', '⇔'), ('⟼', '↦')] {
        let (a, b) = (arrow(long, 12), arrow(short, 12));
        assert_eq!(a, b, "{long} as {short}");
        assert!(a.width > arrow(long, 1).width, "{long} grows");
    }
}

/// The pairs of arrows (⇆ ⇉ ⇇) stretch as two arrows stacked, each as long as the whole.
#[test]
fn paired_arrows_stretch_as_two_stacked_arrows() {
    for symbol in ['⇆', '⇉', '⇇'] {
        let long = arrow(symbol, 12);
        let short = arrow(symbol, 1);
        assert!(long.width > 2.0 * short.width, "{symbol}: {} {}", long.width, short.width);
        // the label's glyphs on one line, the two arrows' on two others
        let mut levels = glyph_ys(&long);
        levels.sort_by(f32::total_cmp);
        levels.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        assert!(levels.len() >= 3, "{symbol}: {levels:?}");
    }
}

// ---------- a font with fewer stretchy glyphs ----------

/// Latin Modern Math, loaded from its file as a font of the caller's (no `latin-modern` feature
/// needed): it has no moustaches, and fewer assemblies than STIX Two Math.
fn latin_modern() -> MathFont {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/latinmodern-math-subset.otf");
    MathFont::load(std::fs::read(path).unwrap()).unwrap()
}

/// `\lmoustache` and `\rmoustache` in a font without them are built as cmex builds them: the
/// halves of the braces joined by their extender, at least the two halves tall, and as tall as
/// the content asks.
#[test]
fn moustaches_are_built_from_brace_halves_in_a_font_without_them() {
    let lm = latin_modern();
    assert!(lm.get().glyph('⎰').is_none() && lm.get().glyph('⎱').is_none());
    let options = options();
    let set = |tex: &str| context_equation_core::typeset_in_font(tex, &options, lm).unwrap_or_else(|e| panic!("{tex}: {e}"));
    let (small, warned) = set(r"\left\lmoustache x \right\rmoustache");
    assert!(warned.is_empty(), "{warned:?}");
    let rows = ["x"; 8].join(r"\\");
    let (tall, _) = set(&format!(r"\left\lmoustache \begin{{matrix}} {rows} \end{{matrix}} \right\rmoustache"));
    let glyphs = |b: &MathBox| b.items.iter().filter(|i| matches!(i, Item::Glyph { .. })).count();
    // short, each moustache is its two halves; tall, extenders join them
    assert_eq!(glyphs(&small), 1 + 2 * 2);
    assert!(glyphs(&tall) > glyphs(&small) + 8, "more extenders: {} {}", glyphs(&tall), glyphs(&small));
    assert!(tall.height + tall.depth > 2.0 * (small.height + small.depth));
}

/// An arrow the font cannot stretch at all (`\leadsto`, ↝: no larger size, no parts, not one built
/// from others) stays the glyph it is, centered under a label wider than it.
#[test]
fn arrow_that_cannot_stretch_stays_its_glyph_under_a_long_label() {
    let f = MathFont::StixTwo.get();
    let id = f.glyph('↝').unwrap();
    assert!(f.assembly(id, false).is_none() && f.variants(id, false).is_empty());
    let long = arrow('↝', 12);
    let drawn: Vec<(f32, f32)> = long.items.iter().filter_map(|i| if let Item::Glyph { id: g, x, scale, .. } = i { (*g == id).then_some((*x, *scale)) } else { None }).collect();
    assert_eq!(drawn.len(), 1, "the glyph itself, once");
    let natural = f.advance(id) * drawn[0].1;
    assert!(long.width > 4.0 * natural, "the label is the wider: {} {natural}", long.width);
    assert!(near(drawn[0].0, (long.width - natural) / 2.0), "centered: {:?} {}", drawn, long.width);
}

/// The measures Latin Modern Math takes from Computer Modern rather than from its MATH table
/// (`MathFont::LatinModern`, only with the `latin-modern` feature): a lone subscript 0.15 em down,
/// a display superscript 0.413 em up, and 0.1 em of room past a display operator's limits.
#[cfg(feature = "latin-modern")]
#[test]
fn latin_modern_takes_computer_modern_script_shifts_and_limit_room() {
    let lm = MathFont::LatinModern;
    let em = lm.get().units_per_em();
    let set = |tex: &str| context_equation_core::typeset_in_font(tex, &options(), lm).unwrap().0;
    let lowest = |b: &MathBox| b.items.iter().filter_map(|i| if let Item::Glyph { y, .. } = i { Some(*y) } else { None }).fold(f32::INFINITY, f32::min);
    let highest = |b: &MathBox| b.items.iter().filter_map(|i| if let Item::Glyph { y, .. } = i { Some(*y) } else { None }).fold(f32::NEG_INFINITY, f32::max);
    assert!(near(lowest(&set("x_1")), -0.15 * em), "{}", lowest(&set("x_1")));
    assert!(near(highest(&set("x^2")), 0.413 * em), "{}", highest(&set("x^2")));
    let f = lm.get();
    let both = set(r"\sum_{i}^{n}");
    let (_, bottom, _, top) = both.ink(f).unwrap();
    assert!(near(both.height, top + 0.1 * em) && near(both.depth, -bottom + 0.1 * em), "{both:?} {bottom} {top}");
}
