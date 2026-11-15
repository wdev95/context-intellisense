//! Glyphs that grow, in math fonts that draw them otherwise than the bundled ones: a third-party font
//! loaded with `MathFont::load` may lack the assemblies these have, give a glyph sizes that are
//! empty or not the shape asked for, or an assembly of no parts. Each font is the bundled STIX Two
//! Math with its MATH table patched here, byte by byte (OpenType's MathVariants, MathGlyphConstruction
//! and GlyphAssembly tables), so nothing new is checked in.

use context_equation_core::font::MathFont;
use context_equation_core::layout::{Item, MathBox};
use context_equation_core::{typeset_in_font, Options};
use ttf_parser::Face;

fn stix() -> Vec<u8> {
    std::fs::read(format!("{}/fonts/STIXTwoMath-Regular-subset.otf", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([bytes[at], bytes[at + 1]]))
}

fn set_u16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}

fn glyph(bytes: &[u8], c: char) -> u16 {
    Face::parse(bytes, 0).unwrap().glyph_index(c).unwrap_or_else(|| panic!("no {c}")).0
}

/// Where the MathGlyphConstruction of `c` starts, vertical or horizontal.
fn construction(bytes: &[u8], c: char, vertical: bool) -> usize {
    let id = usize::from(glyph(bytes, c));
    let tables = u16_at(bytes, 4);
    let math = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|r| &bytes[*r..*r + 4] == b"MATH")
        .map(|r| u32::from_be_bytes(bytes[r + 8..r + 12].try_into().unwrap()) as usize)
        .expect("a MATH table");
    // MATH header: version, then the offsets of MathConstants, MathGlyphInfo and MathVariants
    let variants = math + u16_at(bytes, math + 8);
    let coverage = variants + u16_at(bytes, variants + if vertical { 2 } else { 4 });
    let before = if vertical { 0 } else { u16_at(bytes, variants + 6) };
    let index = match u16_at(bytes, coverage) {
        1 => (0..u16_at(bytes, coverage + 2)).position(|i| u16_at(bytes, coverage + 4 + 2 * i) == id),
        _ => (0..u16_at(bytes, coverage + 2)).find_map(|i| {
            let range = coverage + 4 + 6 * i;
            let (start, end) = (u16_at(bytes, range), u16_at(bytes, range + 2));
            (start..=end).contains(&id).then(|| u16_at(bytes, range + 4) + id - start)
        }),
    }
    .unwrap_or_else(|| panic!("{c} has no construction"));
    variants + u16_at(bytes, variants + 10 + 2 * (before + index))
}

/// The font without the assembly of `c`.
fn without_assembly(mut bytes: Vec<u8>, c: char, vertical: bool) -> Vec<u8> {
    let at = construction(&bytes, c, vertical);
    assert_ne!(u16_at(&bytes, at), 0, "{c} has an assembly");
    set_u16(&mut bytes, at, 0);
    bytes
}

/// The font with the largest horizontal size of `c` drawn by the glyph of `other`.
fn largest_size_drawn_as(mut bytes: Vec<u8>, c: char, other: char) -> Vec<u8> {
    let at = construction(&bytes, c, false);
    let count = u16_at(&bytes, at + 2);
    assert!(count > 0, "{c} has sizes");
    let id = glyph(&bytes, other);
    set_u16(&mut bytes, at + 4 + 4 * (count - 1), id);
    bytes
}

/// The font with the horizontal assembly of `c` made of no parts.
fn empty_assembly(mut bytes: Vec<u8>, c: char) -> Vec<u8> {
    let at = construction(&bytes, c, false);
    let assembly = at + u16_at(&bytes, at);
    // GlyphAssembly: the italic correction (a MathValueRecord, 4 bytes), then the part count
    set_u16(&mut bytes, assembly + 4, 0);
    bytes
}

fn set(tex: &str, font: MathFont) -> MathBox {
    typeset_in_font(tex, &Options { display: true, strict: true, ..Default::default() }, font).unwrap_or_else(|e| panic!("{tex}: {e}")).0
}

fn count(b: &MathBox, what: fn(&Item) -> bool) -> usize {
    b.items.iter().filter(|i| what(i)).count()
}

fn glyphs(b: &MathBox) -> usize {
    count(b, |i| matches!(i, Item::Glyph { .. }))
}

fn lines(b: &MathBox) -> usize {
    count(b, |i| matches!(i, Item::Line { .. }))
}

#[test]
fn a_tall_lgroup_stays_one_glyph_when_the_braces_have_no_halves_to_build_it_from() {
    let tex = r"\left\lgroup \rule{1pt}{8em} \right\rgroup";
    let font = MathFont::load(without_assembly(without_assembly(stix(), '{', true), '}', true)).unwrap();
    let patched = set(tex, font);
    let bundled = set(tex, MathFont::StixTwo);
    // each a glyph of its own, in place of the halves and extenders it is built of in STIX Two
    assert_eq!(glyphs(&patched), 2);
    assert!(glyphs(&bundled) > 2, "{}", glyphs(&bundled));
}

#[test]
fn an_overgroup_whose_largest_size_is_empty_is_not_drawn_again_from_it() {
    let tex = r"\overgroup{abcdefghijklmnopqrstuvwxyz}";
    let font = MathFont::load(largest_size_drawn_as(stix(), '⏠', ' ')).unwrap();
    assert_eq!(lines(&set(tex, font)), 0);
    // STIX Two's own is drawn again: its two corners
    assert_eq!(lines(&set(tex, MathFont::StixTwo)), 2);
}

#[test]
fn an_overgroup_whose_largest_size_has_no_flat_stroke_is_not_drawn_again_from_it() {
    // a slash: the line through its middle crosses it, but no point of its outline is there
    let tex = r"\overgroup{abcdefghijklmnopqrstuvwxyz}";
    let font = MathFont::load(largest_size_drawn_as(stix(), '⏠', '/')).unwrap();
    assert_eq!(lines(&set(tex, font)), 0);
}

#[test]
fn harpoons_that_cannot_stretch_leave_the_pair_at_its_largest_size() {
    let tex = r"\xrightleftharpoons{\text{a long label over the arrows}}";
    let font = MathFont::load(without_assembly(stix(), '⇀', false)).unwrap();
    let patched = set(tex, font);
    let bundled = set(tex, MathFont::StixTwo);
    // the ⇌ glyph, narrower than the label, in place of two harpoons each built of parts
    assert!(patched.width <= bundled.width);
    assert!(glyphs(&patched) < glyphs(&bundled), "{} {}", glyphs(&patched), glyphs(&bundled));
}

#[test]
fn a_brace_assembled_from_no_parts_is_placed_by_its_box() {
    let tex = r"\overbrace{abcdefghijklmnopqrstuvwxyz}^{n}";
    let font = MathFont::load(empty_assembly(stix(), '⏞')).unwrap();
    let b = set(tex, font);
    assert!(b.width.is_finite() && b.height.is_finite() && b.depth.is_finite());
    // the base and the label, and no brace
    assert_eq!(glyphs(&b), 27);
}
