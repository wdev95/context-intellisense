//! Math fonts: STIX Two Math embedded in the binary (the default), Latin Modern Math with the
//! `latin-modern` feature, and any OpenType math font the caller loads ([`MathFont::load`]). Nothing
//! is read from the user's system. The embedded files are subsets of the originals in tools/fonts,
//! written by tools/fonts/subset.sh.
//!
//! A character the math font lacks is looked up in the fallback fonts ([`FallbackFont`],
//! `Options::fallback_fonts`), in order: any OpenType or TrueType font with outlines, such as Noto
//! Emoji (the monochrome one), embedded with the `emoji` feature.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::num::NonZeroU16;
use std::sync::{Mutex, OnceLock, PoisonError, RwLock};

use ttf_parser::math::{Constants, MathValue};
pub use ttf_parser::GlyphId;
use ttf_parser::{Face, OutlineBuilder};

static STIX_TWO: &[u8] = include_bytes!("../fonts/STIXTwoMath-Regular-subset.otf");
#[cfg(feature = "latin-modern")]
static LATIN_MODERN: &[u8] = include_bytes!("../fonts/latinmodern-math-subset.otf");
#[cfg(feature = "emoji")]
static NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular-subset.ttf");
/// The name of the default font.
pub const NAME: &str = "STIX Two Math";

/// The math fonts contextEquation can typeset in: STIX Two Math (the default), with the `latin-modern`
/// feature Latin Modern Math, the OpenType Computer Modern that TeX's own documents are set in, and
/// any OpenType font with a MATH table, loaded from its bytes with [`MathFont::load`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MathFont {
    #[default]
    StixTwo,
    #[cfg(feature = "latin-modern")]
    LatinModern,
    /// A font the caller loaded ([`MathFont::load`]). It lives as long as the program.
    Custom(&'static Font),
}

/// Why a font could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontError {
    /// The bytes are not an OpenType or TrueType font.
    NotAFont(String),
    /// The font has no MATH table: it can write letters, not fractions, radicals or tall
    /// delimiters. A math font has one (STIX Two Math, Latin Modern Math, Libertinus Math, …).
    NoMathTable { name: String },
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::NotAFont(why) => write!(f, "not a font: {why}"),
            FontError::NoMathTable { name } => write!(f, "{name} is not a math font: it has no OpenType MATH table"),
        }
    }
}

impl std::error::Error for FontError {}

/// The fonts loaded so far, by a hash of their bytes: loading the same font twice gives the same
/// font, and keeps one copy.
static LOADED: Mutex<Vec<(u64, &'static Font)>> = Mutex::new(Vec::new());

impl MathFont {
    /// A math font from the bytes of an OpenType (`.otf`) or TrueType (`.ttf`) file. It must have a
    /// MATH table: contextEquation sets fractions, radicals, scripts and tall delimiters from it. The font
    /// is kept for as long as the program runs; the same bytes again give the same font.
    pub fn load(bytes: Vec<u8>) -> Result<MathFont, FontError> {
        let hash = fnv(&bytes);
        let mut loaded = LOADED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, f)) = loaded.iter().find(|(h, _)| *h == hash) {
            return Ok(MathFont::Custom(f));
        }
        let (name, has_math) = {
            let face = Face::parse(&bytes, 0).map_err(|e| FontError::NotAFont(e.to_string()))?;
            (family_name(&face), face.tables().math.and_then(|m| m.constants).is_some())
        };
        if !has_math {
            return Err(FontError::NoMathTable { name });
        }
        let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let font: &'static Font = Box::leak(Box::new(Font { hash: Some(hash), ..Font::parse(data, name) }));
        loaded.push((hash, font));
        Ok(MathFont::Custom(font))
    }

    /// What sets a loaded font's SVG ids apart from another's: none for the embedded fonts.
    pub(crate) fn id_salt(self) -> Option<u64> {
        self.get().hash
    }

    /// The fonts loaded with [`MathFont::load`], in the order they were loaded.
    pub fn loaded() -> Vec<MathFont> {
        LOADED.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|(_, f)| MathFont::Custom(f)).collect()
    }

    pub fn name(self) -> &'static str {
        match self {
            MathFont::StixTwo => NAME,
            #[cfg(feature = "latin-modern")]
            MathFont::LatinModern => "Latin Modern Math",
            MathFont::Custom(f) => &f.name,
        }
    }

    /// The space between words in text, in em: a quarter em in STIX Two Math, and in Latin Modern
    /// Math its space, as wide as Computer Modern's (a third of an em).
    /// A loaded font's is its own space, or a quarter em if it has none.
    pub fn word_space(self) -> f32 {
        match self {
            MathFont::StixTwo => 0.25,
            _ => {
                let f = self.get();
                f.glyph(' ').map(|g| f.advance(g) / f.units_per_em()).filter(|w| *w > 0.0).unwrap_or(0.25)
            }
        }
    }

    /// How far a subscript without a superscript goes down, in em, where the font says otherwise
    /// than its MATH table: Latin Modern Math, which stands for Computer Modern, takes cmsy10's `sub1`
    /// (0.15 em) as pdfTeX does; its MATH table has only `sub2` (0.247 em), meant for a subscript
    /// under a superscript.
    pub fn lone_subscript_shift(self) -> Option<f32> {
        match self {
            MathFont::StixTwo => None,
            #[cfg(feature = "latin-modern")]
            MathFont::LatinModern => Some(0.15),
            MathFont::Custom(_) => None,
        }
    }

    /// How far a superscript goes up in display style, in em, where the font says otherwise than its
    /// MATH table: Latin Modern Math takes cmsy10's `sup1` (0.413 em), as pdfTeX does in display.
    pub fn display_superscript_shift(self) -> Option<f32> {
        match self {
            MathFont::StixTwo => None,
            #[cfg(feature = "latin-modern")]
            MathFont::LatinModern => Some(0.413),
            MathFont::Custom(_) => None,
        }
    }

    /// The room TeX leaves above an upper limit and below a lower one (`big_op_spacing5`), in em,
    /// for the font that stands for Computer Modern: cmex10's 0.1 em. The MATH table has no such
    /// constant.
    pub fn limit_padding(self) -> Option<f32> {
        match self {
            MathFont::StixTwo => None,
            #[cfg(feature = "latin-modern")]
            MathFont::LatinModern => Some(0.1),
            MathFont::Custom(_) => None,
        }
    }

    /// The font, parsed once.
    pub fn get(self) -> &'static Font {
        match self {
            MathFont::StixTwo => {
                static FONT: OnceLock<Font> = OnceLock::new();
                FONT.get_or_init(|| Font::parse(STIX_TWO, NAME.into()))
            }
            #[cfg(feature = "latin-modern")]
            MathFont::LatinModern => {
                static FONT: OnceLock<Font> = OnceLock::new();
                FONT.get_or_init(|| Font::parse(LATIN_MODERN, "Latin Modern Math".into()))
            }
            MathFont::Custom(f) => f,
        }
    }
}

/// A font for the characters the math font lacks (emoji, CJK in `\text{…}`): any OpenType (`.otf`)
/// or TrueType (`.ttf`) font with outlines (`glyf`, `CFF` or `CFF2`), MATH table or not. Its glyphs
/// are scaled to the math font's em and sit on the baseline, as ordinary symbols; their outlines go in
/// the SVG as the others do (S-SVG-5), in one color, `currentColor` (color layers are not read).
///
/// It is a number into the fonts used as fallbacks so far, so that a glyph of the layout carries it in
/// two bytes ([`crate::layout::Item::Glyph`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FallbackFont(NonZeroU16);

/// Every font used as a fallback so far: [`FallbackFont`] `n` is the `n`th.
static FALLBACK_FONTS: RwLock<Vec<&'static Font>> = RwLock::new(Vec::new());

/// The fallback fonts loaded so far, by a hash of their bytes (as [`LOADED`] for math fonts).
static LOADED_FALLBACKS: Mutex<BTreeMap<u64, FallbackFont>> = Mutex::new(BTreeMap::new());

impl std::fmt::Debug for FallbackFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FallbackFont").field(&self.name()).finish()
    }
}

impl FallbackFont {
    /// A fallback font from the bytes of an OpenType or TrueType file. It needs outlines, not a
    /// MATH table; color-only fonts (bitmap emoji) are refused. Kept for as long as the program
    /// runs; the same bytes again give the same font.
    pub fn load(bytes: Vec<u8>) -> Result<FallbackFont, FontError> {
        let hash = fnv(&bytes);
        let mut loaded = LOADED_FALLBACKS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(f) = loaded.get(&hash) {
            return Ok(*f);
        }
        if FALLBACK_FONTS.read().unwrap_or_else(PoisonError::into_inner).len() >= usize::from(u16::MAX) {
            return Err(FontError::NotAFont("too many fallback fonts loaded".into()));
        }
        let (name, has_outlines) = {
            let face = Face::parse(&bytes, 0).map_err(|e| FontError::NotAFont(e.to_string()))?;
            let t = face.tables();
            (family_name(&face), t.glyf.is_some() || t.cff.is_some() || t.cff2.is_some())
        };
        if !has_outlines {
            return Err(FontError::NotAFont(format!("{name} has no outlines (glyf, CFF or CFF2)")));
        }
        let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let font: &'static Font = Box::leak(Box::new(Font { hash: Some(hash), ..Font::parse(data, name) }));
        // a font just made, so not among the fallbacks yet
        let fallback = FallbackFont::push(&mut FALLBACK_FONTS.write().unwrap_or_else(PoisonError::into_inner), font);
        loaded.insert(hash, fallback);
        Ok(fallback)
    }

    /// The number of a font among the fallbacks, given it the first time.
    fn register(font: &'static Font) -> FallbackFont {
        let mut fonts = FALLBACK_FONTS.write().unwrap_or_else(PoisonError::into_inner);
        match fonts.iter().position(|f| std::ptr::eq(*f, font)) {
            Some(i) => FallbackFont::nth(i),
            None => FallbackFont::push(&mut fonts, font),
        }
    }

    /// A font added to the fallbacks, and its number.
    fn push(fonts: &mut Vec<&'static Font>, font: &'static Font) -> FallbackFont {
        fonts.push(font);
        FallbackFont::nth(fonts.len() - 1)
    }

    /// The number of the fallback at `index`.
    fn nth(index: usize) -> FallbackFont {
        // at most u16::MAX fonts: `load` refuses more, and the embedded ones are a handful
        FallbackFont(NonZeroU16::new(u16::try_from(index + 1).expect("at most 65535 fallback fonts")).expect("index + 1 is not zero"))
    }

    /// The fallback fonts loaded with [`FallbackFont::load`], in the order they were loaded.
    pub fn loaded() -> Vec<FallbackFont> {
        let mut fonts: Vec<FallbackFont> = LOADED_FALLBACKS.lock().unwrap_or_else(PoisonError::into_inner).values().copied().collect();
        // numbered as they were loaded
        fonts.sort_by_key(|f| f.0);
        fonts
    }

    /// Noto Emoji, the monochrome one (SIL Open Font License), embedded with the `emoji` feature: a
    /// subset of version 3.002 at its regular weight (crates/core/fonts/README.md).
    #[cfg(feature = "emoji")]
    pub fn noto_emoji() -> FallbackFont {
        static FONT: OnceLock<Font> = OnceLock::new();
        static FALLBACK: OnceLock<FallbackFont> = OnceLock::new();
        *FALLBACK.get_or_init(|| FallbackFont::register(FONT.get_or_init(|| Font::parse(NOTO_EMOJI, "Noto Emoji".into()))))
    }

    /// The fallback fonts of `Options::default()`: Noto Emoji with the `emoji` feature, else none.
    pub fn defaults() -> Vec<FallbackFont> {
        #[cfg(feature = "emoji")]
        return vec![FallbackFont::noto_emoji()];
        #[cfg(not(feature = "emoji"))]
        Vec::new()
    }

    /// Its family name.
    pub fn name(self) -> &'static str {
        &self.get().name
    }

    /// The font, parsed.
    pub fn get(self) -> &'static Font {
        FALLBACK_FONTS.read().unwrap_or_else(PoisonError::into_inner)[usize::from(self.0.get()) - 1]
    }

    /// What names its glyphs in the SVG apart from the math font's and another fallback's
    /// (`…-f1a2b3c4g42`): a hash of its bytes, or of its name for an embedded font.
    pub(crate) fn tag(self) -> u32 {
        let f = self.get();
        f.hash.unwrap_or_else(|| fnv(f.name.as_bytes())) as u32
    }

    /// The first font of `fallbacks` with a glyph for `c`, and that glyph.
    pub fn find(fallbacks: &[FallbackFont], c: char) -> Option<(FallbackFont, GlyphId)> {
        fallbacks.iter().find_map(|f| f.get().glyph(c).map(|id| (*f, id)))
    }
}

/// A math font can stand in for another: Latin Modern Math behind a loaded font, say.
impl From<MathFont> for FallbackFont {
    fn from(font: MathFont) -> FallbackFont {
        FallbackFont::register(font.get())
    }
}

/// The font a glyph of a box is drawn from: its fallback font, or the math font.
pub fn glyph_font(fallback: Option<FallbackFont>, math: &Font) -> &Font {
    match fallback {
        Some(f) => f.get(),
        None => math,
    }
}

pub struct Font {
    face: Face<'static>,
    /// Its family name, from its `name` table.
    name: String,
    /// A hash of its bytes, for a loaded font: the SVG's ids carry it, so two fonts on a page do
    /// not share glyphs. The embedded fonts have none, and their ids are as they always were.
    hash: Option<u64>,
    /// The `ssty` alternates of a glyph: its forms for scripts and for scripts of scripts, drawn for
    /// those sizes as TeX draws its 7pt and 5pt designs (wider and bolder than the 10pt one scaled)
    script_forms: std::collections::HashMap<GlyphId, Vec<GlyphId>>,
}

/// The default font (STIX Two Math).
pub fn font() -> &'static Font {
    MathFont::StixTwo.get()
}

/// One part of an assembled glyph (tall parenthesis, long brace).
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub id: GlyphId,
    pub start: f32,
    pub end: f32,
    pub advance: f32,
    pub extender: bool,
}

impl Font {
    fn parse(data: &'static [u8], name: String) -> Font {
        let face = Face::parse(data, 0).expect("a font checked before");
        let script_forms = script_forms(&face);
        Font { face, name, hash: None, script_forms }
    }

    /// Its family name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The glyph to draw at a script level (1: script, 2: script of script): its `ssty` form for
    /// that level, the last one there is for deeper levels, or the glyph itself.
    pub fn script_form(&self, id: GlyphId, level: usize) -> GlyphId {
        if level == 0 {
            return id;
        }
        self.script_forms.get(&id).and_then(|forms| forms.get(level - 1).or(forms.last())).copied().unwrap_or(id)
    }

    pub fn units_per_em(&self) -> f32 {
        f32::from(self.face.units_per_em())
    }

    pub fn glyph(&self, c: char) -> Option<GlyphId> {
        self.face.glyph_index(c)
    }

    pub fn advance(&self, id: GlyphId) -> f32 {
        f32::from(self.face.glyph_hor_advance(id).unwrap_or(0))
    }

    /// (x_min, y_min, x_max, y_max) of the outline, in font units.
    pub fn limits(&self, id: GlyphId) -> (f32, f32, f32, f32) {
        match self.face.glyph_bounding_box(id) {
            Some(r) => (f32::from(r.x_min), f32::from(r.y_min), f32::from(r.x_max), f32::from(r.y_max)),
            None => (0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Glyph outline as the `d` of an SVG `<path>`, with y pointing up (as in the font).
    pub fn outline(&self, id: GlyphId) -> String {
        let mut path = PathWriter(String::new());
        self.face.outline_glyph(id, &mut path);
        path.0.trim_end().to_string()
    }

    /// The outline of a glyph as closed polylines in font units, one per contour: its curves as
    /// short lines of at most `step` units. What a pen follows to draw it.
    pub fn contours(&self, id: GlyphId, step: f32) -> Vec<Vec<(f32, f32)>> {
        let mut c = Contours { step: step.max(1.0), done: Vec::new(), line: Vec::new(), first: (0.0, 0.0) };
        self.face.outline_glyph(id, &mut c);
        c.finish();
        c.done
    }

    fn constants(&self) -> Constants<'static> {
        self.face.tables().math.and_then(|m| m.constants).expect("the embedded font has a MATH table")
    }

    /// A constant from the MATH table, in font units.
    pub fn c(&self, f: impl Fn(&Constants<'static>) -> MathValue<'static>) -> f32 {
        f32::from(f(&self.constants()).value)
    }

    pub fn script_scale(&self) -> f32 {
        f32::from(self.constants().script_percent_scale_down()) / 100.0
    }

    pub fn script_script_scale(&self) -> f32 {
        f32::from(self.constants().script_script_percent_scale_down()) / 100.0
    }

    pub fn display_operator_min_height(&self) -> f32 {
        f32::from(self.constants().display_operator_min_height())
    }

    /// The height of the lowercase letters (OS/2's `sxHeight`); the MATH table's accent base height
    /// when the font does not give it.
    pub fn x_height(&self) -> f32 {
        match self.face.tables().os2.and_then(|t| t.x_height()) {
            Some(h) if h > 0 => f32::from(h),
            _ => f32::from(self.constants().accent_base_height().value),
        }
    }

    pub fn radical_degree_raise(&self) -> f32 {
        f32::from(self.constants().radical_degree_bottom_raise_percent()) / 100.0
    }

    pub fn italic_correction(&self, id: GlyphId) -> f32 {
        self.face.tables().math.and_then(|m| m.glyph_info).and_then(|g| g.italic_corrections).and_then(|c| c.get(id)).map_or(0.0, |v| f32::from(v.value))
    }

    pub fn accent_point(&self, id: GlyphId) -> Option<f32> {
        self.face.tables().math.and_then(|m| m.glyph_info).and_then(|g| g.top_accent_attachments).and_then(|c| c.get(id)).map(|v| f32::from(v.value))
    }

    pub fn min_overlap(&self) -> f32 {
        self.face.tables().math.and_then(|m| m.variants).map_or(0.0, |v| f32::from(v.min_connector_overlap))
    }

    /// Ready-made size variants, smallest to largest: (glyph, measure).
    pub fn variants(&self, id: GlyphId, vertical: bool) -> Vec<(GlyphId, f32)> {
        self.face
            .tables()
            .math
            .and_then(|m| m.variants)
            .and_then(|v| if vertical { v.vertical_constructions } else { v.horizontal_constructions }.get(id))
            .map(|c| c.variants.into_iter().map(|g| (g.variant_glyph, f32::from(g.advance_measurement))).collect())
            .unwrap_or_default()
    }

    /// Parts to assemble a glyph larger than every variant, bottom to top
    /// (vertical) or left to right (horizontal).
    pub fn assembly(&self, id: GlyphId, vertical: bool) -> Option<Vec<Part>> {
        let v = self.face.tables().math.and_then(|m| m.variants)?;
        let constructions = if vertical { v.vertical_constructions } else { v.horizontal_constructions };
        let assembly = constructions.get(id)?.assembly?;
        Some(
            assembly
                .parts
                .into_iter()
                .map(|p| Part {
                    id: p.glyph_id,
                    start: f32::from(p.start_connector_length),
                    end: f32::from(p.end_connector_length),
                    advance: f32::from(p.full_advance),
                    extender: p.part_flags.extender(),
                })
                .collect(),
        )
    }
}

/// The alternates of the GSUB `ssty` feature, by glyph.
fn script_forms(face: &Face<'static>) -> std::collections::HashMap<GlyphId, Vec<GlyphId>> {
    use ttf_parser::gsub::SubstitutionSubtable;
    let mut forms = std::collections::HashMap::new();
    let Some(gsub) = face.tables().gsub else { return forms };
    for feature in gsub.features.into_iter().filter(|f| f.tag == ttf_parser::Tag::from_bytes(b"ssty")) {
        for index in feature.lookup_indices {
            let Some(lookup) = gsub.lookups.get(index) else { continue };
            for subtable in lookup.subtables.into_iter::<SubstitutionSubtable>() {
                let SubstitutionSubtable::Alternate(alternate) = subtable else { continue };
                for (i, set) in alternate.alternate_sets.into_iter().enumerate() {
                    if let Some(glyph) = coverage_glyph(&alternate.coverage, i as u16) {
                        forms.entry(glyph).or_insert_with(|| set.alternates.into_iter().collect());
                    }
                }
            }
        }
    }
    forms
}

/// The glyph a coverage table lists at `index`.
fn coverage_glyph(coverage: &ttf_parser::opentype_layout::Coverage, index: u16) -> Option<GlyphId> {
    use ttf_parser::opentype_layout::Coverage;
    match coverage {
        Coverage::Format1 { glyphs } => glyphs.get(index),
        Coverage::Format2 { records } => records.into_iter().find_map(|r| {
            let first = r.value;
            (index >= first && index - first <= r.end.0 - r.start.0).then(|| GlyphId(r.start.0 + index - first))
        }),
    }
}

struct PathWriter(String);

fn num(v: f32) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

impl OutlineBuilder for PathWriter {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "M{} {} ", num(x), num(y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "L{} {} ", num(x), num(y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let _ = write!(self.0, "Q{} {} {} {} ", num(x1), num(y1), num(x), num(y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let _ = write!(self.0, "C{} {} {} {} {} {} ", num(x1), num(y1), num(x2), num(y2), num(x), num(y));
    }
    fn close(&mut self) {
        self.0.push_str("Z ");
    }
}

/// A font is itself: two are the same only if they are one (each is loaded once).
impl PartialEq for Font {
    fn eq(&self, other: &Font) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Font {}

impl std::hash::Hash for Font {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state);
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").field("name", &self.name).finish()
    }
}

/// FNV-1a of the bytes: which loaded font they are.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A font's family name (name id 16, typographic, or 1), else its full name, else "custom font".
fn family_name(face: &Face) -> String {
    let names = face.names();
    [16u16, 1, 4].iter().find_map(|id| names.into_iter().filter(|n| n.name_id == *id && n.is_unicode()).find_map(|n| n.to_string())).unwrap_or_else(|| "custom font".to_string())
}

/// Collects a glyph's contours as polylines, its curves cut into short lines.
struct Contours {
    step: f32,
    done: Vec<Vec<(f32, f32)>>,
    line: Vec<(f32, f32)>,
    first: (f32, f32),
}

impl Contours {
    fn finish(&mut self) {
        if self.line.len() > 1 {
            self.done.push(std::mem::take(&mut self.line));
        }
        self.line.clear();
    }

    fn last(&self) -> (f32, f32) {
        self.line.last().copied().unwrap_or(self.first)
    }

    /// A Bézier from the last point, with its control points and end, as short lines.
    fn curve(&mut self, curve: Bezier) {
        let p = self.last();
        let dist = |u: (f32, f32), v: (f32, f32)| (u.0 - v.0).hypot(u.1 - v.1);
        let points: &[(f32, f32)] = match &curve {
            Bezier::Quadratic(points) => points,
            Bezier::Cubic(points) => points,
        };
        let mut length = 0.0;
        let mut prev = p;
        for &q in points {
            length += dist(prev, q);
            prev = q;
        }
        let n = ((length / self.step).ceil() as usize).clamp(1, 64);
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let u = 1.0 - t;
            let at = match curve {
                Bezier::Quadratic([a, q]) => (u * u * p.0 + 2.0 * u * t * a.0 + t * t * q.0, u * u * p.1 + 2.0 * u * t * a.1 + t * t * q.1),
                Bezier::Cubic([a, b, q]) => (
                    u * u * u * p.0 + 3.0 * u * u * t * a.0 + 3.0 * u * t * t * b.0 + t * t * t * q.0,
                    u * u * u * p.1 + 3.0 * u * u * t * a.1 + 3.0 * u * t * t * b.1 + t * t * t * q.1,
                ),
            };
            self.line.push(at);
        }
    }
}

/// A curve's control points and end, after the point it starts from.
enum Bezier {
    Quadratic([(f32, f32); 2]),
    Cubic([(f32, f32); 3]),
}

impl OutlineBuilder for Contours {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.first = (x, y);
        self.line.push((x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.line.push((x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.curve(Bezier::Quadratic([(x1, y1), (x, y)]));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.curve(Bezier::Cubic([(x1, y1), (x2, y2), (x, y)]));
    }
    fn close(&mut self) {
        if self.last() != self.first {
            self.line.push(self.first);
        }
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_without_outline_has_zero_bounds() {
        let f = font();
        assert_eq!(f.limits(f.glyph(' ').unwrap()), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn quadratic_curve_becomes_q() {
        // STIX Two Math is CFF (cubics only), but the outline writer also accepts TrueType fonts
        let mut c = PathWriter(String::new());
        c.quad_to(1.0, 2.25, 3.0, 4.0);
        assert_eq!(c.0, "Q1 2.3 3 4 ");
    }
}
