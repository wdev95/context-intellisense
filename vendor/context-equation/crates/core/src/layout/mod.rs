//! From the tree to positioned boxes. Follows Appendix G of the TeXbook, with parameters
//! taken from the font's MATH table (as MathML Core describes).

pub mod alphabet;
mod arithmetic;
mod breaking;
mod composite;
mod diagram;
pub mod kerning;
pub mod math_box;
mod stretch;
mod table;

pub use alphabet::math_italic;
pub use breaking::{break_lines, Formula};
pub use math_box::{Item, MathBox};

use ttf_parser::math::Constants;

use crate::font::{FallbackFont, Font, MathFont};
use crate::locale::Locale;
use crate::rules::{self, Registry};
use crate::tree::*;
use crate::Error;

type R<T> = Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Display,
    Text,
    Script,
    ScriptScript,
}

/// TeX style: size (display, text, script, scriptscript) and whether it is "cramped"
/// (superscripts rise less inside denominators and subscripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub level: Level,
    pub cramped: bool,
    /// The font the equation is set in.
    pub font: MathFont,
}

impl Style {
    pub fn display() -> Style {
        Style { level: Level::Display, cramped: false, font: MathFont::default() }
    }

    pub fn text() -> Style {
        Style { level: Level::Text, cramped: false, font: MathFont::default() }
    }

    /// The same style in another font.
    pub fn in_font(self, font: MathFont) -> Style {
        Style { font, ..self }
    }

    /// The style's font.
    pub fn font(self) -> &'static Font {
        self.font.get()
    }

    pub fn scale(self) -> f32 {
        match self.level {
            Level::Display | Level::Text => 1.0,
            Level::Script => self.font().script_scale(),
            Level::ScriptScript => self.font().script_script_scale(),
        }
    }

    /// 0 in text and display, 1 in scripts, 2 in scripts of scripts: the `ssty` form to draw.
    pub fn script_level(self) -> usize {
        match self.level {
            Level::Display | Level::Text => 0,
            Level::Script => 1,
            Level::ScriptScript => 2,
        }
    }

    pub fn is_script(self) -> bool {
        matches!(self.level, Level::Script | Level::ScriptScript)
    }

    fn sup(self) -> Style {
        let level = match self.level {
            Level::Display | Level::Text => Level::Script,
            _ => Level::ScriptScript,
        };
        Style { level, cramped: self.cramped, font: self.font }
    }

    fn sub(self) -> Style {
        Style { cramped: true, ..self.sup() }
    }

    fn num(self) -> Style {
        let level = match self.level {
            Level::Display => Level::Text,
            Level::Text => Level::Script,
            _ => Level::ScriptScript,
        };
        Style { level, cramped: self.cramped, font: self.font }
    }

    fn den(self) -> Style {
        Style { cramped: true, ..self.num() }
    }

    fn cramp(self) -> Style {
        Style { cramped: true, ..self }
    }

    fn not_display(self) -> Style {
        if self.level == Level::Display {
            Style { level: Level::Text, ..self }
        } else {
            self
        }
    }
}

pub struct Context<'a> {
    pub locale: Locale,
    pub registry: &'a mut Registry,
    /// A missing glyph becomes an error (strict) or a box with a warning (tolerant).
    pub strict: bool,
    /// How text is kerned.
    pub kerning: kerning::TextKerning,
    /// Where a character the math font lacks is looked up, in order, before it becomes a box.
    pub fallbacks: &'a [FallbackFont],
}

/// MATH table constant already at the style's scale.
fn k(e: Style, f: impl Fn(&Constants<'static>) -> ttf_parser::math::MathValue<'static>) -> f32 {
    e.font().c(f) * e.scale()
}

/// A graphicx length (`2\height`, `1cm`) for a box of width `w`, height `h` and depth `d`.
fn length(l: &BoxLength, w: f32, h: f32, d: f32, e: Style) -> f32 {
    em(e, l.em) + l.width * w + l.height * h + l.depth * d
}

/// `\scalebox{x}[y]`: a negative factor mirrors the box inside its own width (x) or turns its height
/// into depth (y), as graphicx does.
fn scale_xy(b: &mut MathBox, x: f32, y: f32) {
    if y > 0.0 && (x.abs() - y).abs() < 1e-6 {
        // the same factor both ways: scaled glyphs, no transform
        if y != 1.0 {
            b.scale_box(y);
        }
        if x < 0.0 {
            b.mirror();
        }
        return;
    }
    let (w, h, d) = (b.width, b.height, b.depth);
    b.transform([x, 0.0, 0.0, y, if x < 0.0 { -x * w } else { 0.0 }, 0.0]);
    b.width = x.abs() * w;
    (b.height, b.depth) = if y >= 0.0 { (y * h, y * d) } else { (-y * d, -y * h) };
}

/// `\rotatebox[origin=…]{degrees}`: turns the box counterclockwise around the origin point, and takes
/// the box that holds the turned one, as graphicx does. Without `origin`, the point is the left end of
/// the baseline; with it, each letter (l, r, t, b, B) sets one coordinate and the other is the center.
fn rotate(b: &mut MathBox, degrees: f32, origin: &str) {
    let (w, h, d) = (b.width, b.height, b.depth);
    let (mut px, mut py) = if origin.is_empty() { (0.0, 0.0) } else { (w / 2.0, (h - d) / 2.0) };
    for letter in origin.chars() {
        match letter {
            'l' => px = 0.0,
            'r' => px = w,
            't' => py = h,
            'b' => py = -d,
            'B' => py = 0.0,
            _ => {}
        }
    }
    let snap = |v: f32| if v.abs() < 1e-6 { 0.0 } else { v };
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (sin, cos) = (snap(sin), snap(cos));
    let turn = |x: f32, y: f32| (px + cos * (x - px) - sin * (y - py), py + sin * (x - px) + cos * (y - py));
    let corners = [turn(0.0, -d), turn(w, -d), turn(0.0, h), turn(w, h)];
    let left = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
    let right = corners.iter().map(|c| c.0).fold(f32::NEG_INFINITY, f32::max);
    let low = corners.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
    let high = corners.iter().map(|c| c.1).fold(f32::NEG_INFINITY, f32::max);
    let (e, f) = turn(0.0, 0.0);
    b.transform([cos, sin, -sin, cos, snap(e - left), snap(f)]);
    b.width = right - left;
    b.height = high.max(0.0);
    b.depth = (-low).max(0.0);
}

fn em(e: Style, amount: f32) -> f32 {
    e.font().units_per_em() * amount * e.scale()
}

pub(crate) fn glyph(c: char, e: Style) -> R<MathBox> {
    let f = e.font();
    // (a stmaryrd or mathabx symbol Unicode has no character for is drawn by composition, #176)
    let Some(id) = f.glyph(c).or_else(|| equivalent(c).and_then(|x| f.glyph(x))) else { return composite::symbol(c, e) };
    // in a script, the font's form for that size (`ssty`), as TeX draws cmr7 rather than cmr10
    let mut math_box = glyph_id(f.script_form(id, e.script_level()), e.scale(), f);
    // (with the letter's italic correction when that form has none of its own, as cmmi7's has)
    if math_box.italic == 0.0 {
        math_box.italic = f.italic_correction(id) * e.scale();
    }
    Ok(math_box)
}

/// `\ldots` and `\cdots` as TeX sets them: `\mathinner{\ldotp\ldotp\ldotp}`, three dots with a
/// thin space between them (punctuation's, so none in scripts), each in the script's own form.
fn dots(dot: char, e: Style) -> R<MathBox> {
    let g = glyph(dot, e)?;
    let mut math_box = g.clone();
    for _ in 0..2 {
        if !e.is_script() {
            math_box.append(&MathBox::space(em(e, 3.0 / 18.0)));
        }
        math_box.append(&g);
    }
    math_box.single_glyph = false;
    Ok(math_box)
}

/// Characters the font lacks, drawn with an equivalent it has.
fn equivalent(c: char) -> Option<char> {
    Some(match c {
        '✔' | '✅' => '✓',
        // the font has no ✗: the cross-product × is the closest drawing
        '✗' | '✘' | '❌' => '⨯',
        _ => return None,
    })
}

pub(crate) fn glyph_id(id: ttf_parser::GlyphId, s: f32, f: &Font) -> MathBox {
    let (_, y0, _, y1) = f.limits(id);
    MathBox {
        width: f.advance(id) * s,
        height: y1.max(0.0) * s,
        depth: (-y0).max(0.0) * s,
        items: vec![Item::Glyph { id, x: 0.0, y: 0.0, scale: s, color: None, flip: false, font: None }],
        italic: f.italic_correction(id) * s,
        accent_point: f.accent_point(id).map(|v| v * s),
        single_glyph: true,
    }
}

/// Size of the capitals that stand for lowercase letters in `\textsc`.
const SMALL_CAPS_SCALE: f32 = 0.76;

/// Upright text (function names, `\text`, units), and the italic inside it.
fn upright_text(t: &str, e: Style, ctx: &mut Context) -> R<MathBox> {
    use crate::input::latex::{BOLD_END, BOLD_START, ITALIC_CORRECTED_END, ITALIC_END, ITALIC_START, MARKERS, MONO_END, MONO_START, SMALL_CAPS_END, SMALL_CAPS_START};
    let mut math_box = MathBox::empty();
    let mut bold = 0;
    let mut small_caps = 0;
    let mut mono = 0;
    let mut italic = 0;
    // L-TXT-5: the italic correction of the last letter, for the end of a `\textit{…}`
    let mut last_italic = 0.0;
    // the previous character and its weight, for TeX's kern between the two (none across a change)
    let mut previous: Option<(char, bool)> = None;
    let chars: Vec<char> = t.chars().collect();
    for (k, &c) in chars.iter().enumerate() {
        match c {
            BOLD_START => bold += 1,
            BOLD_END => bold -= 1,
            SMALL_CAPS_START => small_caps += 1,
            SMALL_CAPS_END => small_caps -= 1,
            MONO_START => mono += 1,
            MONO_END => mono -= 1,
            ITALIC_START => italic += 1,
            ITALIC_END => italic -= 1,
            ITALIC_CORRECTED_END => {
                italic -= 1;
                // LaTeX's \/ after the group, unless a period or a comma follows it
                let next = chars[k + 1..].iter().find(|x| !MARKERS.contains(x));
                if !matches!(next, Some('.' | ',')) {
                    math_box.append(&MathBox::space(last_italic));
                }
            }
            _ => {}
        }
        if MARKERS.contains(&c) {
            previous = None;
            continue;
        }
        // TeX's kerns are the upright fonts'; italic letters keep none between them
        if small_caps == 0 && mono == 0 && italic == 0 {
            if let Some((p, weight)) = previous {
                let k = kerning::kern(ctx.kerning, weight, p, c);
                if k != 0.0 {
                    math_box.append(&MathBox::space(em(e, k)));
                }
            }
            previous = Some((c, bold > 0));
        } else {
            previous = None;
        }
        // small capitals: a lowercase letter is its capital at the height of the x
        if small_caps > 0 && c.is_lowercase() {
            if let Some(id) = c.to_uppercase().next().and_then(|u| e.font().glyph(u)) {
                math_box.append(&glyph_id(id, e.scale() * SMALL_CAPS_SCALE, e.font()));
                continue;
            }
        }
        // bold through the math block letters; accented ones stay regular
        // typewriter likewise: letters and digits; the font has no typewriter punctuation
        let c = if mono > 0 {
            alphabet::character(c, Variant::Monospace)
        } else if italic > 0 {
            alphabet::character(c, if bold > 0 { Variant::BoldItalic } else { Variant::Italic })
        } else if bold > 0 {
            alphabet::character(c, Variant::Bold)
        } else {
            c
        };
        if c == ' ' || c == '\u{00A0}' {
            last_italic = 0.0;
            math_box.append(&MathBox::space(em(e, e.font.word_space())));
            continue;
        }
        if c == '\u{2003}' {
            math_box.append(&MathBox::space(em(e, 1.0)));
            continue;
        }
        if c == '\u{200B}' {
            continue;
        }
        // (`glyph` fails only for a character the font lacks)
        let g = match glyph(c, e) {
            Err(Error::MissingGlyph { character }) => {
                math_box.append(&missing(character, e, ctx)?);
                continue;
            }
            other => other?,
        };
        last_italic = if italic > 0 { g.italic } else { 0.0 };
        math_box.append(&g);
    }
    math_box.single_glyph = false;
    Ok(math_box)
}

pub fn compose(node: &Node, e: Style, ctx: &mut Context) -> R<MathBox> {
    let leaf = matches!(node, Node::Identifier { .. } | Node::Operator { .. } | Node::Number { .. } | Node::FixedDelimiter { .. } | Node::LargeOperator { .. });
    match compose_node(node, e, ctx) {
        Err(Error::MissingGlyph { character }) if leaf => missing_leaf(node, character, e, ctx),
        other => other,
    }
}

/// A symbol with a character the math font lacks. A name of several letters is set letter by letter,
/// each from the math font or a fallback; anything else is the one character. Out of line, so the
/// recursion's frames do not grow.
#[cold]
#[inline(never)]
fn missing_leaf(node: &Node, character: char, e: Style, ctx: &mut Context) -> R<MathBox> {
    if let Node::Identifier { name, variant } = node {
        if name.chars().count() > 1 {
            let mut math_box = MathBox::empty();
            for c in name.chars() {
                let g = match glyph(alphabet::character(c, *variant), e).or_else(|_| glyph(c, e)) {
                    Err(Error::MissingGlyph { character }) => missing(character, e, ctx)?,
                    other => other?,
                };
                math_box.append(&g);
            }
            return Ok(math_box);
        }
    }
    missing(character, e, ctx)
}

/// A character the math font lacks: from the first fallback font that has it, as an ordinary symbol;
/// else refused (strict) or drawn as a box (E-REC-8).
#[cold]
#[inline(never)]
fn missing(c: char, e: Style, ctx: &mut Context) -> R<MathBox> {
    if let Some(math_box) = fallback_glyph(c, e, ctx.fallbacks) {
        return Ok(math_box);
    }
    if ctx.strict {
        return Err(Error::MissingGlyph { character: c });
    }
    ctx.registry.warn(rules::E_REC_8, format!("the font has no '{c}' (U+{:04X}): drew a box", c as u32));
    glyph('□', e)
}

/// `c` from the first of `fallbacks` that has it: its glyph scaled from that font's em to the math
/// font's, on the baseline, with no italic correction and its accent over the middle.
pub fn fallback_glyph(c: char, e: Style, fallbacks: &[FallbackFont]) -> Option<MathBox> {
    let (font, id) = FallbackFont::find(fallbacks, c)?;
    let f = font.get();
    let s = e.scale() * e.font().units_per_em() / f.units_per_em();
    let (_, y0, _, y1) = f.limits(id);
    Some(MathBox {
        width: f.advance(id) * s,
        height: y1.max(0.0) * s,
        depth: (-y0).max(0.0) * s,
        items: vec![Item::Glyph { id, x: 0.0, y: 0.0, scale: s, color: None, flip: false, font: Some(font) }],
        italic: 0.0,
        accent_point: None,
        single_glyph: true,
    })
}

fn compose_node(node: &Node, e: Style, ctx: &mut Context) -> R<MathBox> {
    match node {
        Node::Number { value, group, thousands, variant } => compose_number(value, (*group, *thousands, *variant), e, ctx),
        Node::Identifier { name, variant } => {
            let mut math_box = MathBox::empty();
            for c in name.chars() {
                let mapped = alphabet::character(c, *variant);
                let g = glyph(mapped, e).or_else(|_| glyph(c, e))?;
                if name.chars().count() == 1 {
                    return Ok(g);
                }
                math_box.append(&g);
            }
            Ok(math_box)
        }
        Node::Function { name, .. } => {
            let local = ctx.locale.function_name(name);
            if local != *name {
                ctx.registry.note(rules::L_FUN_1, format!("function {name} written as '{local}' ({})", ctx.locale));
            }
            upright_text(&local, e, ctx)
        }
        Node::Operator { symbol: '…', .. } => dots('.', e),
        Node::Operator { symbol: '⋯', .. } => dots('⋅', e),
        Node::Operator { symbol, .. } => glyph(*symbol, e),
        Node::FixedDelimiter { symbol, size, .. } => {
            let total = [1.2, 1.8, 2.4, 3.0][(*size as usize).clamp(1, 4) - 1];
            stretch::delimiter(*symbol, em(e, total), e)
        }
        Node::LargeOperator { symbol, .. } => large_operator(*symbol, e),
        Node::Row { children } => compose_row(children, e, ctx, false),
        Node::Fraction { .. } => compose_fraction(node, e, ctx),
        Node::Root { radicand, index } => compose_root(radicand, index.as_deref(), e, ctx),
        Node::Scripts { base, sup, sub } => compose_scripts(base, sup.as_deref(), sub.as_deref(), e, ctx),
        Node::Delimited { open, close, parts, middles } => compose_delimited(*open, *close, parts, middles, e, ctx),
        Node::Table { shape: kind, rows, columns, verticals, dashed, horizontals, between, intertexts, stretch, colsep } => {
            let lines = table::Lines { verticals, dashed, horizontals, stretch: *stretch, colsep: *colsep };
            table::compose(*kind, rows, columns, &lines, between, intertexts, e, ctx)
        }
        Node::VdotsWithin { symbol, .. } => compose_vdots_within(symbol, e, ctx),
        // a cell, a lap and a phantom are math lists of their own: a slanted letter that ends them
        // keeps its italic correction
        Node::Multicolumn { base, .. } => compose_list(base, e, ctx),
        Node::Unit { base, .. } => compose(base, e, ctx),
        Node::Raised { em: amount, height, depth, base } => {
            let mut math_box = compose(base, e, ctx)?;
            math_box.shift_vertical(em(e, *amount));
            math_box.single_glyph = false;
            // \height and \depth in the optional arguments are those of the raised box, as in LaTeX
            let (w, h, d) = (math_box.width, math_box.height, math_box.depth);
            if let Some(height) = height {
                math_box.height = length(height, w, h, d, e);
            }
            if let Some(depth) = depth {
                math_box.depth = length(depth, w, h, d, e);
            }
            Ok(math_box)
        }
        Node::Scaled { factor, vertical, base } => {
            let mut math_box = compose(base, e, ctx)?;
            scale_xy(&mut math_box, *factor, vertical.unwrap_or(*factor));
            Ok(math_box)
        }
        Node::Resized { width, height, total, base } => {
            let mut math_box = compose(base, e, ctx)?;
            let (w, h, d) = (math_box.width, math_box.height, math_box.depth);
            let tall = if *total { h + d } else { h };
            let factor = |target: &Option<BoxLength>, natural: f32| target.as_ref().map(|t| length(t, w, h, d, e) / natural).filter(|f| f.is_finite() && *f != 0.0);
            let (x, y) = (factor(width, w), factor(height, tall));
            // `!` keeps the ratio; a size that cannot be reached (an empty box) leaves it as it is
            let (x, y) = match (x, y) {
                (Some(x), Some(y)) => (x, y),
                (Some(f), None) | (None, Some(f)) => (f, f),
                (None, None) => (1.0, 1.0),
            };
            let limit = |f: f32| f.signum() * f.abs().clamp(0.05, 20.0);
            scale_xy(&mut math_box, limit(x), limit(y));
            Ok(math_box)
        }
        Node::Rotated { degrees, origin, base } => {
            let mut math_box = compose(base, e, ctx)?;
            rotate(&mut math_box, *degrees, origin);
            Ok(math_box)
        }
        Node::Rule { width, height, raise } => Ok(MathBox::bar(em(e, *width), em(e, *height), em(e, *raise))),
        // \mathchoice: TeX sets the branch of the style it lands in
        Node::Choice { branches } => {
            let i = match e.level {
                Level::Display => 0,
                Level::Text => 1,
                Level::Script => 2,
                Level::ScriptScript => 3,
            };
            match branches.get(i) {
                Some(branch) => compose(branch, e, ctx),
                None => Ok(MathBox::empty()),
            }
        }
        // \vcenter: the box's middle on the math axis
        Node::VCenter { base } => {
            let mut b = compose(base, e, ctx)?;
            let axis = k(e, |c| c.axis_height());
            b.shift_vertical(axis - (b.height - b.depth) / 2.0);
            b.single_glyph = false;
            Ok(b)
        }
        // \reflectbox: \scalebox{-1}[1], mirrored inside its own width
        Node::Reflected { base } => {
            let mut b = compose(base, e, ctx)?;
            b.mirror();
            Ok(b)
        }
        Node::Smashed { base, height, depth } => {
            let mut math_box = compose(base, e, ctx)?;
            if *height {
                math_box.height = 0.0;
            }
            if *depth {
                math_box.depth = 0.0;
            }
            math_box.single_glyph = false;
            Ok(math_box)
        }
        Node::Overlapped { base, side } => {
            let b = compose_list(base, e, ctx)?;
            let dx = match side {
                'l' => -b.width,
                'c' => -b.width / 2.0,
                _ => 0.0,
            };
            let mut math_box = MathBox::empty();
            math_box.place(&b, dx, 0.0);
            Ok(math_box)
        }
        Node::VerticalArrow { symbol, left, right } => compose_vertical_arrow(*symbol, left.as_deref(), right.as_deref(), e, ctx),
        Node::Accent { base, accent, stretchy, below } => compose_accent(base, *accent, *stretchy, *below, e, ctx),
        Node::Bar { base, above } => compose_bar(base, *above, e, ctx),
        Node::Brace { base, above, label, bracket } => compose_brace(base, *above, label.as_deref(), *bracket, e, ctx),
        Node::Text { parts } => {
            let mut math_box = MathBox::empty();
            for p in parts {
                match p {
                    TextPart::Text(t) => math_box.append(&upright_text(t, e, ctx)?),
                    TextPart::Math(m) => math_box.append(&compose_list(m, e.not_display(), ctx)?),
                }
            }
            math_box.single_glyph = false;
            Ok(math_box)
        }
        Node::Space { em: amount } => Ok(MathBox::space(em(e, *amount))),
        Node::Canceled { base, shape: kind } => compose_canceled(base, *kind, e, ctx),
        Node::Framed { base, shape, background, border, padding, thickness } => {
            compose_frame(base, *shape, background.as_deref(), border.as_deref(), (*padding, *thickness), e, ctx)
        }
        Node::Colored { color, base } => {
            let mut math_box = compose(base, e, ctx)?;
            math_box.colorize(color);
            Ok(math_box)
        }
        Node::Attributed { attributes, base } => {
            let mut math_box = compose(base, e, ctx)?;
            math_box.group(attributes.clone());
            Ok(math_box)
        }
        Node::Quantity { unit, .. } => compose(unit, e, ctx),
        Node::Arrow { symbol, above, below } => compose_arrow(*symbol, above.as_deref(), below.as_deref(), e, ctx),
        Node::Stacked { base, above, below } => {
            let b = compose(base, e, ctx)?;
            limits(b, above.as_deref(), below.as_deref(), e, ctx)
        }
        Node::Phantom { base, width, height } => {
            let mut math_box = compose_list(base, e, ctx)?;
            math_box.items.clear();
            if !width {
                math_box.width = 0.0;
            }
            if !height {
                math_box.height = 0.0;
                math_box.depth = 0.0;
            }
            Ok(math_box)
        }
        Node::ForcedClass { base, .. } => compose(base, e, ctx),
        Node::Style { level, base } => {
            let level = match level {
                0 => Level::Display,
                1 => Level::Text,
                2 => Level::Script,
                _ => Level::ScriptScript,
            };
            compose(base, Style { level, cramped: e.cramped, font: e.font }, ctx)
        }
        Node::ArrowDiagram { cells, arrows } => diagram::compose(cells, arrows, e, ctx),
        #[cfg(feature = "chem")]
        Node::Chemistry { scheme } => crate::chem::layout::compose(scheme, e, ctx),
        Node::LongDivision { dividend, divisor } => arithmetic::long_division(dividend, divisor, e),
        Node::ColumnArithmetic { operation, terms } => arithmetic::column_arithmetic(*operation, terms, e),
    }
}

fn compose_number(value: &str, (group, thousands, variant): (bool, Option<char>, Variant), e: Style, ctx: &mut Context) -> R<MathBox> {
    let sep = ctx.locale.decimal_separator();
    if value.contains('.') && sep != '.' {
        ctx.registry.note(rules::L_NUM_2, format!("{value} written with '{sep}' as decimal separator ({})", ctx.locale));
    }
    let (integer_part, fraction) = value.split_once('.').map_or((value, None), |(a, b)| (a, Some(b)));
    let mut math_box = MathBox::empty();
    let n = integer_part.chars().count();
    for (i, c) in integer_part.chars().enumerate() {
        // siunitx: groups of three from five digits on, with a thin space
        if group && n >= 5 && i > 0 && (n - i) % 3 == 0 {
            math_box.append(&MathBox::space(em(e, 3.0 / 18.0)));
        }
        if let Some(m) = thousands {
            if i > 0 && (n - i) % 3 == 0 {
                math_box.append(&glyph(m, e)?);
            }
        }
        math_box.append(&digit(c, variant, e)?);
    }
    if let Some(fr) = fraction {
        math_box.append(&glyph(sep, e)?);
        for c in fr.chars() {
            math_box.append(&digit(c, variant, e)?);
        }
    }
    math_box.single_glyph = value.chars().count() == 1;
    Ok(math_box)
}

/// #203: a digit in its alphabet (𝟏 in `\mathbf`), or the regular one if the font lacks it.
fn digit(c: char, variant: Variant, e: Style) -> R<MathBox> {
    glyph(alphabet::character(c, variant), e).or_else(|_| glyph(c, e))
}

// ---------- row: classes and spacing ----------

/// Atom class of a node. `None` for explicit spaces, which do not count as neighbors.
pub fn class_of(node: &Node) -> Option<Class> {
    Some(match node {
        Node::Space { .. } => return None,
        Node::Function { .. } | Node::LargeOperator { .. } => Class::Op,
        Node::Operator { class, .. } | Node::FixedDelimiter { class, .. } | Node::ForcedClass { class, .. } => *class,
        Node::Fraction { .. } | Node::Delimited { .. } => Class::Inner,
        Node::Table { shape: kind, .. } if !matches!(kind, TableKind::Array | TableKind::Aligned | TableKind::Centered | TableKind::Substack | TableKind::Diagram) => Class::Inner,
        Node::Multicolumn { base, .. } | Node::Raised { base, .. } | Node::Scaled { base, .. } | Node::Smashed { base, .. } | Node::Unit { base, .. } => return class_of(base),
        // the text style's branch: a class does not know the style
        Node::Choice { branches } => return branches.get(1).and_then(class_of),
        Node::Scripts { base, .. } => return class_of(base),
        Node::Arrow { .. } => Class::Rel,
        // #205: a long arrow after \not or \centernot stays a relation, spaced as one
        Node::Canceled { base, shape: Cancel::Bar | Cancel::CenterNot } if is_long_arrow(base) => Class::Rel,
        Node::Colored { base, .. } | Node::Attributed { base, .. } | Node::Style { base, .. } => match base.as_ref() {
            Node::Row { children } if children.len() == 1 => return class_of(&children[0]),
            Node::Row { .. } => Class::Ord,
            other => return class_of(other),
        },
        _ => Class::Ord,
    })
}

/// Classes after the context rules (L-INT-1 and L-ESP-2).
pub fn classes(children: &[Node], locale: Locale, mut registry: Option<&mut Registry>, initial_ord: bool) -> Vec<Option<Class>> {
    let mut cl: Vec<Option<Class>> = children.iter().map(class_of).collect();
    let neighbor_before =
        |cl: &[Option<Class>], i: usize| -> Option<Class> { cl[..i].iter().rev().flatten().next().copied().or(if initial_ord { Some(Class::Ord) } else { None }) };
    let neighbor_after = |cl: &[Option<Class>], i: usize| -> Option<Class> { cl[i + 1..].iter().flatten().next().copied() };

    // L-INT-1 (pt-BR, fr…): ]a, b[ is an open interval, so ] opens and [ closes
    if locale.open_interval() {
        for i in 0..children.len() {
            let Node::Operator { symbol, .. } = &children[i] else {
                continue;
            };
            let before = neighbor_before(&cl, i);
            let after = neighbor_after(&cl, i);
            let no_operand = matches!(before, None | Some(Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct));
            if *symbol == ']' && no_operand && after.is_some() {
                cl[i] = Some(Class::Open);
                if let Some(r) = registry.as_deref_mut() {
                    r.note(rules::L_INT_1, "']' at the start read as an interval opening");
                }
            } else if *symbol == '['
                && matches!(before, Some(Class::Ord | Class::Close | Class::Inner))
                && matches!(after, None | Some(Class::Rel | Class::Punct | Class::Close | Class::Bin))
                && children[..i].iter().any(|n| matches!(n, Node::Operator { symbol: ']', .. }))
            {
                cl[i] = Some(Class::Close);
                if let Some(r) = registry.as_deref_mut() {
                    r.note(rules::L_INT_1, "'[' at the end read as an interval closing");
                }
            }
        }
    }

    // L-ESP-2 (TeXbook, Appendix G, rules 5 and 6): a Bin without operands on both sides becomes Ord
    for i in 0..cl.len() {
        if cl[i] != Some(Class::Bin) {
            continue;
        }
        let before = neighbor_before(&cl, i);
        let after = neighbor_after(&cl, i);
        let no_left = matches!(before, None | Some(Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct));
        let no_right = matches!(after, None | Some(Class::Rel | Class::Close | Class::Punct));
        if no_left || no_right {
            cl[i] = Some(Class::Ord);
            if let (Some(r), Node::Operator { symbol, .. }) = (registry.as_deref_mut(), &children[i]) {
                r.note(rules::L_ESP_2, format!("'{symbol}' is unary: no operator spacing"));
            }
        }
    }
    cl
}

/// L-ESP-1: space between atoms in mu (1/18 em). TeXbook table, ch. 18.
/// Negative values are the ones TeX puts in parentheses: they vanish in script style.
fn mu_space(left: Class, right: Class, index: bool) -> f32 {
    use Class::*;
    let idx = |c: Class| match c {
        Ord => 0,
        Op => 1,
        Bin => 2,
        Rel => 3,
        Open => 4,
        Close => 5,
        Punct => 6,
        Inner => 7,
    };
    const T: [[i8; 8]; 8] = [
        // Ord Op Bin Rel Open Close Punct Inner
        [0, 1, -2, -3, 0, 0, 0, -1],     // Ord
        [1, 1, 0, -3, 0, 0, 0, -1],      // Op
        [-2, -2, 0, 0, -2, 0, 0, -2],    // Bin
        [-3, -3, 0, 0, -3, 0, 0, -3],    // Rel
        [0, 0, 0, 0, 0, 0, 0, 0],        // Open
        [0, 1, -2, -3, 0, 0, 0, -1],     // Close
        [-1, -1, 0, -1, -1, -1, -1, -1], // Punct
        [-1, 1, -2, -3, -1, 0, -1, -1],  // Inner
    ];
    let v = T[idx(left)][idx(right)];
    if v < 0 && index {
        return 0.0;
    }
    match v.abs() {
        1 => 3.0,
        2 => 4.0,
        3 => 5.0,
        _ => 0.0,
    }
}

pub(crate) fn compose_row(children: &[Node], e: Style, ctx: &mut Context, initial_ord: bool) -> R<MathBox> {
    if let ([single], false) = (children, initial_ord) {
        // the row of a single node is the node itself: keeps italic and accent point
        return compose(single, e, ctx);
    }
    let cl = classes(children, ctx.locale, Some(ctx.registry), initial_ord);
    let mu = em(e, 1.0 / 18.0);
    let mut math_box = MathBox::empty();
    let mut previous = if initial_ord { Some(Class::Ord) } else { None };
    let mut pending_italic = 0.0;
    for (node, class) in children.iter().zip(&cl) {
        let b = compose(node, e, ctx)?;
        if let (Some(a), Some(c)) = (previous, class) {
            math_box.append(&MathBox::space(mu_space(a, *c, e.is_script()) * mu));
        }
        // italic correction after a slanted letter, before whatever follows (Appendix G, rule 17)
        if pending_italic > 0.0 {
            math_box.append(&MathBox::space(pending_italic));
        }
        pending_italic = if slanted_letter(node) { b.italic } else { 0.0 };
        math_box.append(&b);
        if class.is_some() {
            previous = *class;
        }
    }
    // and after the row's last letter: TeX ends the list with its kern
    if pending_italic > 0.0 {
        math_box.append(&MathBox::space(pending_italic));
    }
    math_box.single_glyph = false;
    Ok(math_box)
}

/// A math list on its own (the formula, a script, a numerator, a radicand): a slanted letter that
/// ends it keeps its italic correction (Appendix G, rule 17), as a row does after its last letter.
pub fn compose_list(node: &Node, e: Style, ctx: &mut Context) -> R<MathBox> {
    let mut math_box = compose(node, e, ctx)?;
    if ends_slanted(node) && math_box.italic > 0.0 {
        let italic = math_box.italic;
        math_box.append(&MathBox::space(italic));
        math_box.single_glyph = false;
    }
    Ok(math_box)
}

/// A slanted letter or word (`\mathit{diff}`), alone or in a row of one: the row adds no kern of its own.
fn ends_slanted(node: &Node) -> bool {
    match node {
        Node::Identifier { variant: Variant::Italic | Variant::BoldItalic | Variant::SansSerifItalic | Variant::SansSerifBoldItalic, .. } => true,
        Node::Row { children } if children.len() == 1 => ends_slanted(&children[0]),
        Node::Attributed { base, .. } => ends_slanted(base),
        _ => false,
    }
}

/// A slanted letter, also inside the group of `\class` and its kin, which changes nothing in the drawing.
fn slanted_letter(node: &Node) -> bool {
    match node {
        Node::Identifier { variant: Variant::Italic | Variant::BoldItalic | Variant::SansSerifItalic | Variant::SansSerifBoldItalic, .. } => true,
        Node::Attributed { base, .. } => slanted_letter(base.single()),
        _ => false,
    }
}

/// A cell: if it is a row, compose it with an invisible Ord in front (so `&=` gets relation spacing).
fn compose_cell(node: &Node, e: Style, ctx: &mut Context, initial_ord: bool) -> R<MathBox> {
    match node {
        Node::Row { children } => compose_row(children, e, ctx, initial_ord),
        other => compose_row(std::slice::from_ref(other), e, ctx, initial_ord),
    }
}

// ---------- fractions and roots ----------

#[allow(clippy::too_many_arguments)]
fn compose_fraction(fraction: &Node, e: Style, ctx: &mut Context) -> R<MathBox> {
    let Node::Fraction { num, den, style, rule, thickness: given, numerator_align } = fraction else { unreachable!("a fraction") };
    let (style, rule, given) = (*style, *rule, *given);
    let e = match style {
        FractionStyle::Display => Style { level: Level::Display, ..e },
        FractionStyle::Row => e.not_display(),
        FractionStyle::Auto => e,
    };
    let n = compose_list(num, e.num(), ctx)?;
    let d = compose_list(den, e.den(), ctx)?;
    let display = e.level == Level::Display;
    let axis = k(e, |c| c.axis_height());
    let (mut u, mut v);
    let mut thickness = 0.0;
    // a thickness of 0 (\above0pt) is a fraction without a rule, as in TeX
    let rule = rule && given.is_none_or(|t| t > 0.0);
    if rule {
        thickness = k(e, |c| c.fraction_rule_thickness());
        u = if display { k(e, |c| c.fraction_numerator_display_style_shift_up()) } else { k(e, |c| c.fraction_numerator_shift_up()) };
        v = if display { k(e, |c| c.fraction_denominator_display_style_shift_down()) } else { k(e, |c| c.fraction_denominator_shift_down()) };
        let mut gap_n = if display { k(e, |c| c.fraction_num_display_style_gap_min()) } else { k(e, |c| c.fraction_numerator_gap_min()) };
        let mut gap_d = if display { k(e, |c| c.fraction_denom_display_style_gap_min()) } else { k(e, |c| c.fraction_denominator_gap_min()) };
        // a thickness the input gives (\above, \genfrac) is absolute, as TeX's dimensions are, and the
        // clearance follows it (TeX's rule 15d: 3θ in display style, θ otherwise)
        if let Some(t) = given {
            thickness = e.font().units_per_em() * t;
            gap_n = if display { 3.0 * thickness } else { thickness };
            gap_d = gap_n;
        }
        u = u.max(gap_n + thickness / 2.0 + axis + n.depth);
        v = v.max(gap_d + thickness / 2.0 - axis + d.height);
    } else {
        u = if display { k(e, |c| c.stack_top_display_style_shift_up()) } else { k(e, |c| c.stack_top_shift_up()) };
        v = if display { k(e, |c| c.stack_bottom_display_style_shift_down()) } else { k(e, |c| c.stack_bottom_shift_down()) };
        let gap_min = if display { k(e, |c| c.stack_display_style_gap_min()) } else { k(e, |c| c.stack_gap_min()) };
        let gap = (u - n.depth) - (d.height - v);
        if gap < gap_min {
            let delta = (gap_min - gap) / 2.0;
            u += delta;
            v += delta;
        }
    }
    // \nulldelimiterspace on each side, as in TeX
    let margin = em(e, 0.12);
    let width = n.width.max(d.width);
    let mut math_box = MathBox::space(width + 2.0 * margin);
    // \cfrac[l], \cfrac[r]: the numerator at one side
    let room = match numerator_align {
        Some('l') => 0.0,
        Some('r') => width - n.width,
        _ => (width - n.width) / 2.0,
    };
    math_box.place(&n, margin + room, u);
    math_box.place(&d, margin + (width - d.width) / 2.0, -v);
    if rule {
        math_box.place(&MathBox::bar(width, thickness, axis - thickness / 2.0), margin, 0.0);
    }
    Ok(math_box)
}

fn compose_root(radicand: &Node, index: Option<&Node>, e: Style, ctx: &mut Context) -> R<MathBox> {
    let f = e.font();
    let base = compose_list(radicand, e.cramp(), ctx)?;
    let thickness = k(e, |c| c.radical_rule_thickness());
    let mut gap = if e.level == Level::Display { k(e, |c| c.radical_display_style_vertical_gap()) } else { k(e, |c| c.radical_vertical_gap()) };
    let extra = k(e, |c| c.radical_extra_ascender());
    let target = base.height + base.depth + gap + thickness;
    let mut radical = stretch::vertical('√', target, e)?;
    let total = radical.total();
    if total > target {
        gap += (total - target) / 2.0;
    }
    // the top of the radical touches the rule above
    let top = base.height + gap + thickness;
    let dy = top - radical.height;
    radical.shift_vertical(dy);

    let mut math_box = MathBox::empty();
    if let Some(ind) = index {
        let ei = Style { level: Level::ScriptScript, cramped: true, font: e.font };
        let i = compose(ind, ei, ctx)?;
        let before = k(e, |c| c.radical_kern_before_degree());
        let after = k(e, |c| c.radical_kern_after_degree());
        let raise = f.radical_degree_raise() * radical.total() - radical.depth;
        math_box.place(&i, before, raise + i.depth);
        math_box.width = (before + i.width + after).max(0.0);
    }
    let x_radical = math_box.width;
    math_box.place(&radical, x_radical, 0.0);
    let x_base = x_radical + radical.width;
    math_box.place(&base, x_base, 0.0);
    math_box.place(&MathBox::bar(base.width, thickness, top - thickness), x_base, 0.0);
    math_box.width = x_base + base.width;
    math_box.height = math_box.height.max(top + extra);
    Ok(math_box)
}

// ---------- scripts and limits ----------

fn uses_limits(base: &Node, e: Style) -> bool {
    let display = e.level == Level::Display;
    match base {
        Node::LargeOperator { symbol, limits } => {
            let by_default = !is_integral(*symbol);
            limits.unwrap_or(by_default && display)
        }
        Node::Function { limits, .. } => *limits && display,
        Node::ForcedClass { base, class: Class::Op } => uses_limits(base, e) || display,
        _ => false,
    }
}

/// Integrals take their limits on the side, even in display style.
pub(crate) fn is_integral(c: char) -> bool {
    matches!(c, '∫' | '∬' | '∭' | '⨌' | '∮' | '∯' | '∰' | '∱' | '∲' | '∳' | '⨏' | '⨖')
}

fn compose_scripts(base: &Node, sup: Option<&Node>, sub: Option<&Node>, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, e, ctx)?;
    if uses_limits(base, e) {
        return limits(b, sup, sub, e, ctx);
    }
    let integral = matches!(base, Node::LargeOperator { .. });
    // the prime of f' is already drawn as a superscript: it goes at normal size, attached to the base
    let (primes, sup) = split_primes(sup);
    let mut b = b;
    if let Some(p) = primes {
        let mut with_primes = b.clone();
        with_primes.append(&MathBox::space(b.italic));
        with_primes.append(&compose(&p, e, ctx)?);
        with_primes.single_glyph = false;
        with_primes.italic = 0.0;
        b = with_primes;
        if sup.is_none() && sub.is_none() {
            return Ok(b);
        }
    }
    let is_simple = b.single_glyph && !integral;
    let s = sup.as_ref().map(|n| compose_list(n, e.sup(), ctx)).transpose()?;
    let d = sub.map(|n| compose_list(n, e.sub(), ctx)).transpose()?;

    let mut u = 0.0;
    let mut v = 0.0;
    if let Some(s) = &s {
        let rise = match e.font.display_superscript_shift() {
            Some(shift) if e.level == Level::Display && !e.cramped => em(e, shift),
            _ if e.cramped => k(e, |c| c.superscript_shift_up_cramped()),
            _ => k(e, |c| c.superscript_shift_up()),
        };
        let drop = if is_simple { 0.0 } else { b.height - k(e, |c| c.superscript_baseline_drop_max()) };
        u = rise.max(drop).max(s.depth + k(e, |c| c.superscript_bottom_min()));
    }
    if let Some(d) = &d {
        let descent = match e.font.lone_subscript_shift() {
            Some(shift) if s.is_none() => em(e, shift),
            _ => k(e, |c| c.subscript_shift_down()),
        };
        let drop = if is_simple { 0.0 } else { b.depth + k(e, |c| c.subscript_baseline_drop_min()) };
        v = descent.max(drop).max(d.height - k(e, |c| c.subscript_top_max()));
    }
    if let (Some(s), Some(d)) = (&s, &d) {
        let gap = (u - s.depth) - (d.height - v);
        let gap_min = k(e, |c| c.sub_superscript_gap_min());
        if gap < gap_min {
            v += gap_min - gap;
        }
        let raise_by = k(e, |c| c.superscript_bottom_max_with_subscript()) - (u - s.depth);
        if raise_by > 0.0 {
            u += raise_by;
            v -= raise_by;
        }
    }
    let delta = b.italic;
    let (x_sup, x_sub) = if integral { (b.width, b.width - delta) } else { (b.width + delta, b.width) };
    let mut math_box = b.clone();
    math_box.single_glyph = false;
    let mut width = b.width;
    if let Some(s) = &s {
        math_box.place(s, x_sup, u);
        width = width.max(x_sup + s.width);
    }
    if let Some(d) = &d {
        math_box.place(d, x_sub, -v);
        width = width.max(x_sub + d.width);
    }
    math_box.width = width + k(e, |c| c.space_after_script());
    math_box.italic = 0.0;
    Ok(math_box)
}

/// Takes the primes (′ ″ ‴) off the start of the superscript: f'^2 becomes f′ with exponent 2.
fn split_primes(sup: Option<&Node>) -> (Option<Node>, Option<Node>) {
    let prime = |n: &Node| matches!(n, Node::Operator { symbol: '′' | '″' | '‴', .. });
    match sup {
        Some(n) if prime(n) => (Some(n.clone()), None),
        Some(Node::Row { children }) if children.first().is_some_and(prime) => {
            let qty = children.iter().take_while(|f| prime(f)).count();
            let remainder = &children[qty..];
            let remainder = if remainder.is_empty() { None } else { Some(Node::row(remainder.to_vec()).simplify()) };
            (Some(Node::row(children[..qty].to_vec())), remainder)
        }
        other => (None, other.cloned()),
    }
}

/// Limits above and below (display sum, `lim`, `\overset`).
fn limits(b: MathBox, above: Option<&Node>, below: Option<&Node>, e: Style, ctx: &mut Context) -> R<MathBox> {
    let s = above.map(|n| compose(n, e.sup(), ctx)).transpose()?;
    let d = below.map(|n| compose(n, e.sub(), ctx)).transpose()?;
    let width = b.width.max(s.as_ref().map_or(0.0, |s| s.width)).max(d.as_ref().map_or(0.0, |d| d.width));
    let delta = b.italic;
    let mut math_box = MathBox::space(width);
    math_box.place(&b, (width - b.width) / 2.0, 0.0);
    if let Some(s) = &s {
        let gap = k(e, |c| c.upper_limit_gap_min()).max(k(e, |c| c.upper_limit_baseline_rise_min()) - s.depth);
        math_box.place(s, (width - s.width + delta) / 2.0, b.height + gap + s.depth);
    }
    if let Some(d) = &d {
        let gap = k(e, |c| c.lower_limit_gap_min()).max(k(e, |c| c.lower_limit_baseline_drop_min()) - d.height);
        math_box.place(d, (width - d.width - delta) / 2.0, -(b.depth + gap + d.height));
    }
    if let Some(padding) = e.font.limit_padding() {
        if s.is_some() {
            math_box.height += em(e, padding);
        }
        if d.is_some() {
            math_box.depth += em(e, padding);
        }
    }
    Ok(math_box)
}

fn large_operator(c: char, e: Style) -> R<MathBox> {
    let f = e.font();
    let Some(id) = f.glyph(c) else { return composite::large(c, e) };
    let mut chosen = id;
    if e.level == Level::Display {
        let minimum = f.display_operator_min_height();
        for (v, measure) in f.variants(id, true) {
            chosen = v;
            if measure >= minimum {
                break;
            }
        }
    }
    let mut math_box = glyph_id(f.script_form(chosen, e.script_level()), e.scale(), f);
    // centered on the math axis
    let axis = k(e, |c| c.axis_height());
    let offset = axis - (math_box.height - math_box.depth) / 2.0;
    math_box.shift_vertical(offset);
    math_box.single_glyph = true;
    Ok(math_box)
}

// ---------- delimiters ----------

fn compose_delimited(open: Option<char>, close: Option<char>, parts: &[Node], middles: &[char], e: Style, ctx: &mut Context) -> R<MathBox> {
    let contents = parts.iter().map(|p| compose(p, e, ctx)).collect::<R<Vec<_>>>()?;
    let axis = k(e, |c| c.axis_height());
    let half = contents.iter().map(|c| (c.height - axis).max(c.depth + axis)).fold(0.0f32, f32::max);
    // \delimiterfactor = 901 and \delimitershortfall = 5pt
    let target = (2.0 * half * 0.901).max(2.0 * half - em(e, 0.5));
    let delim = |c: Option<char>| -> R<MathBox> {
        match c {
            Some(c) => stretch::delimiter(c, target, e),
            None => Ok(MathBox::space(em(e, 0.12))),
        }
    };
    let mut math_box = delim(open)?;
    for (i, content) in contents.iter().enumerate() {
        if i > 0 {
            math_box.append(&delim(middles.get(i - 1).copied())?);
        }
        math_box.append(content);
    }
    math_box.append(&delim(close)?);
    math_box.single_glyph = false;
    Ok(math_box)
}

// ---------- accents, bars, braces, arrows ----------

fn combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x20D0..=0x20FF)
}

/// `\vdotswithin{=}`: the phantom of `{}={}`, and ⋮ drawn without taking width, centered on the
/// phantom's middle (a phantom box after it, the two centered together on the end of the first).
fn compose_vdots_within(symbol: &Node, e: Style, ctx: &mut Context) -> R<MathBox> {
    let within = || Node::Phantom { base: Box::new(Node::row(vec![Node::row(vec![]), symbol.clone(), Node::row(vec![])])), width: true, height: false };
    let drawn = Node::row(vec![within(), Node::Overlapped { base: Box::new(Node::row(vec![Node::VDOTS, within()])), side: 'c' }]);
    compose(&drawn, e, ctx)
}

fn compose_accent(base: &Node, accent: char, stretchy: bool, below: bool, e: Style, ctx: &mut Context) -> R<MathBox> {
    if stretchy && matches!(accent, LINE_SEGMENT | '\u{0304}') {
        return compose_drawn_accent(base, accent, below, e, ctx);
    }
    if below {
        return compose_under_accent(base, accent, stretchy, e, ctx);
    }
    let b = compose(base, e.cramp(), ctx)?;
    let base_point = if b.single_glyph { b.accent_point.unwrap_or(b.width / 2.0) } else { b.width / 2.0 };
    let a = if stretchy { stretch::horizontal(accent, b.width, e)? } else { glyph(accent, e)? };
    let point_a = if stretchy {
        a.width / 2.0
    } else {
        a.accent_point.unwrap_or_else(|| {
            let id = e.font().glyph(accent).expect("glyph already resolved");
            let (x0, _, x1, _) = e.font().limits(id);
            (x0 + x1) / 2.0 * e.scale()
        })
    };
    let dy = if combining(accent) {
        let base_height = k(e, |c| c.accent_base_height());
        (b.height - base_height).max(0.0)
    } else {
        b.height + k(e, |c| c.overbar_vertical_gap()) / 2.0 + a.depth
    };
    let mut math_box = b.clone();
    if b.width == 0.0 && b.items.is_empty() {
        // accent over nothing (\hat{}): takes the width of its drawing instead of leaking left
        // (combining accents have zero advance)
        let glyphs = a.items.iter().filter_map(|i| if let Item::Glyph { id, x, scale, font, .. } = i { Some((*id, *x, *scale, *font)) } else { None });
        let (x0, x1) = glyphs.fold((0.0f32, a.width), |(x0, x1), (id, x, scale, font)| {
            let (g0, _, g1, _) = crate::font::glyph_font(font, e.font()).limits(id);
            (x0.min(x + g0 * scale), x1.max(x + g1 * scale))
        });
        math_box.place(&a, -x0, dy);
        math_box.width = x1 - x0;
    } else {
        math_box.place(&a, base_point - point_a, dy);
    }
    math_box.single_glyph = false;
    math_box.italic = b.italic;
    Ok(math_box)
}

/// The accent that stands for KaTeX's `\overlinesegment` and `\underlinesegment` (U+20E9, the
/// combining wide bridge).
const LINE_SEGMENT: char = '\u{20E9}';

/// The wide accents no glyph of the font draws, drawn with rules:
/// - KaTeX's `\overlinesegment` and `\underlinesegment` (`LINE_SEGMENT`): a line as wide as the base
///   with a tick across each end, in KaTeX's proportions (a 0.04 em line, ticks 0.334 em tall), as far
///   from the base as `\overline`'s bar;
/// - mathabx's `\widebar` (a stretchy macron): `\bar`'s macron, where `\bar` puts it, as wide as the
///   base (the fonts' macron does not stretch, and their overline sits higher).
#[inline(never)]
fn compose_drawn_accent(base: &Node, accent: char, below: bool, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, if below { e } else { e.cramp() }, ctx)?;
    if accent != LINE_SEGMENT {
        let macron = glyph(accent, e)?;
        let (bottom, top) = stretch::ink(&macron, e);
        let lift = if combining(accent) { (b.height - k(e, |c| c.accent_base_height())).max(0.0) } else { b.height };
        let mut math_box = b.clone();
        math_box.single_glyph = false;
        math_box.place(&MathBox::bar(b.width, top - bottom, lift + bottom), 0.0, 0.0);
        return Ok(math_box);
    }
    let (t, half) = (em(e, 0.04), em(e, 0.167));
    let gap = if below { k(e, |c| c.underbar_vertical_gap()) } else { k(e, |c| c.overbar_vertical_gap()) };
    // the middle of the line
    let y = if below { -(b.depth + gap + half) } else { b.height + gap + half };
    let mut math_box = b.clone();
    math_box.single_glyph = false;
    let mut segment = MathBox::bar(b.width, t, y - t / 2.0);
    for x in [0.0, b.width - t] {
        segment.place(&MathBox::bar(t, 2.0 * half, y - half), x, 0.0);
    }
    math_box.place(&segment, 0.0, 0.0);
    Ok(math_box)
}

/// Stretchy accent under the base (`\underrightarrow`, `\underleftarrow`), as far below as `\underline`.
fn compose_under_accent(base: &Node, accent: char, stretchy: bool, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, e, ctx)?;
    if !stretchy {
        return under_mark(b, accent, e);
    }
    let a = stretch::horizontal(accent, b.width, e)?;
    let width = b.width.max(a.width);
    let gap = k(e, |c| c.underbar_vertical_gap());
    let mut math_box = MathBox::space(width);
    math_box.place(&b, (width - b.width) / 2.0, 0.0);
    math_box.place(&a, (width - a.width) / 2.0, -(b.depth + gap + a.height));
    math_box.depth += k(e, |c| c.underbar_extra_descender());
    math_box.italic = b.italic;
    Ok(math_box)
}

/// An accent's mark at its own size under the base (accents' `\underaccent{\tilde}{x}`): its ink
/// centered on the base and as far below it as `\underline`'s rule. The mark of a combining accent
/// sits high over its baseline and left of its origin, so it is placed by its ink.
#[inline(never)]
fn under_mark(b: MathBox, accent: char, e: Style) -> R<MathBox> {
    let a = glyph(accent, e)?;
    let f = e.font();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for item in &a.items {
        if let Item::Glyph { id, x, y, scale, font, .. } = item {
            let (g0, h0, g1, h1) = crate::font::glyph_font(*font, f).limits(*id);
            (x0, y0, x1, y1) = (x0.min(x + g0 * scale), y0.min(y + h0 * scale), x1.max(x + g1 * scale), y1.max(y + h1 * scale));
        }
    }
    if x0 > x1 {
        (x0, y0, x1, y1) = (0.0, -a.depth, a.width, a.height);
    }
    let gap = k(e, |c| c.underbar_vertical_gap());
    let dy = -(b.depth + gap) - y1;
    let mut math_box = b.clone();
    math_box.single_glyph = false;
    math_box.place(&a, (b.width - (x0 + x1)) / 2.0, dy);
    math_box.height = b.height;
    math_box.depth = b.depth.max(-(dy + y0)) + k(e, |c| c.underbar_extra_descender());
    Ok(math_box)
}

fn compose_bar(base: &Node, above: bool, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, if above { e.cramp() } else { e }, ctx)?;
    let mut math_box = b.clone();
    math_box.single_glyph = false;
    if above {
        let gap = k(e, |c| c.overbar_vertical_gap());
        let t = k(e, |c| c.overbar_rule_thickness());
        math_box.place(&MathBox::bar(b.width, t, b.height + gap), 0.0, 0.0);
        math_box.height += k(e, |c| c.overbar_extra_ascender());
    } else {
        let gap = k(e, |c| c.underbar_vertical_gap());
        let t = k(e, |c| c.underbar_rule_thickness());
        math_box.place(&MathBox::bar(b.width, t, -(b.depth + gap + t)), 0.0, 0.0);
        math_box.depth += k(e, |c| c.underbar_extra_descender());
    }
    Ok(math_box)
}

fn compose_brace(base: &Node, above: bool, label: Option<&Node>, bracket: bool, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, e, ctx)?;
    let symbol = match (above, bracket) {
        (true, false) => '⏞',
        (false, false) => '⏟',
        (true, true) => '⎴',
        (false, true) => '⎵',
    };
    // never narrower than the font's first stretched variant: centered on a short base, it sticks
    // out past it and the box widens, as in TeX
    let brace = stretch::horizontal(symbol, b.width.max(stretch::minimum_width(symbol, e)), e)?;
    let r = label.map(|n| compose(n, e.sup(), ctx)).transpose()?;
    let width = b.width.max(brace.width).max(r.as_ref().map_or(0.0, |r| r.width));
    // the brace keeps the bar's gap from its content and the label keeps a limit's gap from the
    // brace (TeX: \overbrace is a \mathop with the label as \limits), measured on the ink
    let (bottom, top) = stretch::ink(&brace, e);
    let mut math_box = MathBox::space(width);
    math_box.place(&b, (width - b.width) / 2.0, 0.0);
    if above {
        let y = b.height + k(e, |c| c.overbar_vertical_gap()) - bottom;
        math_box.place(&brace, (width - brace.width) / 2.0, y);
        if let Some(r) = &r {
            math_box.place(r, (width - r.width) / 2.0, y + top + k(e, |c| c.upper_limit_gap_min()) + r.depth);
        }
    } else {
        let y = -(b.depth + k(e, |c| c.underbar_vertical_gap())) - top;
        math_box.place(&brace, (width - brace.width) / 2.0, y);
        if let Some(r) = &r {
            math_box.place(r, (width - r.width) / 2.0, y + bottom - k(e, |c| c.lower_limit_gap_min()) - r.height);
        }
    }
    Ok(math_box)
}

fn compose_arrow(symbol: char, above: Option<&Node>, below: Option<&Node>, e: Style, ctx: &mut Context) -> R<MathBox> {
    compose_wide_arrow(symbol, above, below, 0.0, e, ctx)
}

pub(crate) fn compose_wide_arrow(symbol: char, above: Option<&Node>, below: Option<&Node>, minimum: f32, e: Style, ctx: &mut Context) -> R<MathBox> {
    let s = above.map(|n| compose(n, e.sup(), ctx)).transpose()?;
    let d = below.map(|n| compose(n, e.sub(), ctx)).transpose()?;
    let labels = s.as_ref().map_or(0.0, |s| s.width).max(d.as_ref().map_or(0.0, |d| d.width));
    let arrow = stretch::horizontal(symbol, (labels + em(e, 0.8)).max(minimum), e)?;
    let width = arrow.width.max(labels);
    let mut math_box = MathBox::space(width);
    math_box.place(&arrow, (width - arrow.width) / 2.0, 0.0);
    if let Some(s) = &s {
        let y = k(e, |c| c.stretch_stack_top_shift_up()).max(arrow.height + k(e, |c| c.stretch_stack_gap_above_min()) + s.depth);
        math_box.place(s, (width - s.width) / 2.0, y);
    }
    if let Some(d) = &d {
        let y = k(e, |c| c.stretch_stack_bottom_shift_down()).max(arrow.depth + k(e, |c| c.stretch_stack_gap_below_min()) + d.height);
        math_box.place(d, (width - d.width) / 2.0, -y);
    }
    Ok(math_box)
}

/// amscd vertical arrow: ↓ stretched, centered on the axis, script-size labels on the sides.
fn compose_vertical_arrow(symbol: char, left: Option<&Node>, right: Option<&Node>, e: Style, ctx: &mut Context) -> R<MathBox> {
    let arrow = stretch::delimiter(symbol, em(e, 2.0), e)?;
    let es = e.sup();
    let l = left.map(|n| compose(n, es, ctx)).transpose()?;
    let r = right.map(|n| compose(n, es, ctx)).transpose()?;
    let axis = k(e, |c| c.axis_height());
    let slack = em(e, 0.17);
    let mut math_box = MathBox::empty();
    let center = |c: &MathBox| axis - (c.height - c.depth) / 2.0;
    if let Some(l) = &l {
        math_box.place(l, 0.0, center(l));
        math_box.width = l.width + slack;
    }
    let x = math_box.width;
    math_box.place(&arrow, x, 0.0);
    math_box.width = x + arrow.width;
    if let Some(r) = &r {
        let x = math_box.width + slack;
        math_box.place(r, x, center(r));
        math_box.width = x + r.width;
    }
    Ok(math_box)
}

// ---------- frames and cancels ----------

fn compose_canceled(base: &Node, kind: Cancel, e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, e, ctx)?;
    if kind == Cancel::Slash {
        return slashed(b, e);
    }
    if kind == Cancel::Bar && is_long_arrow(base) {
        return not_long_arrow(b, e, ctx);
    }
    let t = k(e, |c| c.fraction_rule_thickness());
    let slack = em(e, 0.05);
    let (x0, x1, y0, y1) = (-slack, b.width + slack, -b.depth - slack, b.height + slack);
    let mut math_box = b.clone();
    math_box.single_glyph = false;
    let mut rule = |a: (f32, f32), z: (f32, f32)| {
        math_box.items.push(Item::Line { x1: a.0, y1: a.1, x2: z.0, y2: z.1, thickness: t, color: None });
    };
    if matches!(kind, Cancel::Bar | Cancel::X) {
        rule((x0, y0), (x1, y1));
    }
    if matches!(kind, Cancel::Backslash | Cancel::X) {
        rule((x0, y1), (x1, y0));
    }
    if kind == Cancel::CenterNot {
        center_not(&mut math_box, e, ctx)?;
    }
    Ok(math_box)
}

/// slashed.sty's `\slashed`: the font's "/" over the base, both centered across the wider of the two
/// and the slash's middle on the base's middle, in a box as wide as the wider one. Each is a box of
/// its own in TeX (`\hbox{$X$}`), with its italic correction in its width.
#[inline(never)]
fn slashed(b: MathBox, e: Style) -> R<MathBox> {
    let slash = glyph('/', e)?;
    let (base_width, slash_width) = (b.width + b.italic, slash.width + slash.italic);
    let width = base_width.max(slash_width);
    let mut math_box = MathBox::empty();
    math_box.place(&b, (width - base_width) / 2.0, 0.0);
    let lift = ((b.height - b.depth) - (slash.height - slash.depth)) / 2.0;
    math_box.place(&slash, (width - slash_width) / 2.0, lift);
    math_box.width = width;
    Ok(math_box)
}

/// #205: a long arrow (U+27F5…U+27FF: `\longrightarrow`, `\Longrightarrow`, `\longmapsto`…) as a relation,
/// which `\not` does not cross out whole. `\cancel{\longrightarrow}` has an ordinary arrow (a group) and
/// stays crossed out.
fn is_long_arrow(base: &Node) -> bool {
    matches!(base, Node::Operator { symbol: '\u{27F5}'..='\u{27FF}', class: Class::Rel })
}

/// #205: `\not` before a long arrow, as TeX sets it: `\not` is a relation of zero width whose slash
/// overlaps the left part of the next character (plain.tex, fontmath.ltx), so the slash stands over the
/// arrow's start, where it would cross an `=`, and the arrow keeps its length. The slash is `\centernot`'s.
#[inline(never)]
fn not_long_arrow(mut math_box: MathBox, e: Style, ctx: &mut Context) -> R<MathBox> {
    let equals = glyph('=', e)?;
    let slash = compose(&Node::Operator { symbol: '⧸', class: Class::Ord }, e, ctx)?;
    math_box.single_glyph = false;
    math_box.place(&slash, (equals.width - slash.width) / 2.0, 0.0);
    Ok(math_box)
}

/// centernot's slash: `\not`'s, at its own size, over the middle of the base (centernot.sty kerns half the
/// base's width, back half the width of `=`, and sets `\not`). MathJax's centernot sets ⧸ (U+29F8) there.
#[inline(never)]
fn center_not(math_box: &mut MathBox, e: Style, ctx: &mut Context) -> R<()> {
    let slash = compose(&Node::Operator { symbol: '⧸', class: Class::Ord }, e, ctx)?;
    let x = (math_box.width - slash.width) / 2.0;
    math_box.place(&slash, x, 0.0);
    Ok(())
}

/// `sizes`: the padding and the line's thickness the input gave, in em (`\bbox`); LaTeX's otherwise.
fn compose_frame(base: &Node, shape: Frame, background: Option<&str>, border: Option<&str>, sizes: (Option<f32>, Option<f32>), e: Style, ctx: &mut Context) -> R<MathBox> {
    let b = compose(base, e, ctx)?;
    let (padding, thickness) = sizes;
    let t = em(e, thickness.unwrap_or(0.04));
    match shape {
        Frame::Rectangle | Frame::Fill => {
            // a given padding is all the space there is: no room for a line that is not drawn
            let t = if shape == Frame::Fill && padding.is_some() { 0.0 } else { t };
            let p = em(e, padding.unwrap_or(0.3));
            let width = b.width + 2.0 * (p + t);
            let top = b.height + p + t;
            let bottom = b.depth + p + t;
            let mut math_box = MathBox::space(width);
            // the background goes first, so the content is drawn over it
            if let Some(color) = background {
                math_box.items.push(Item::Rule { x: 0.0, y: -bottom, width, height: top + bottom, color: Some(color.to_string()) });
            }
            math_box.place(&b, p + t, 0.0);
            if shape == Frame::Rectangle {
                let color = border.map(str::to_string);
                math_box.items.push(Item::Rule { x: 0.0, y: top - t, width, height: t, color: color.clone() });
                math_box.items.push(Item::Rule { x: 0.0, y: -bottom, width, height: t, color: color.clone() });
                math_box.items.push(Item::Rule { x: 0.0, y: -bottom, width: t, height: top + bottom, color: color.clone() });
                math_box.items.push(Item::Rule { x: width - t, y: -bottom, width: t, height: top + bottom, color });
            }
            math_box.height = top;
            math_box.depth = bottom;
            Ok(math_box)
        }
        Frame::Circle => {
            // \textcircled: circle around, centered on the content
            let radius = (b.width.max(b.total()) / 2.0 + em(e, 0.12)).max(em(e, 0.42));
            let width = 2.0 * radius + em(e, 0.06);
            let cx = width / 2.0;
            let cy = (b.height - b.depth) / 2.0;
            let mut math_box = MathBox::space(width);
            math_box.place(&b, cx - b.width / 2.0, 0.0);
            math_box.items.push(Item::Ellipse { cx, cy, rx: radius, ry: radius, thickness: t * 1.5, color: None });
            math_box.height = math_box.height.max(cy + radius + t);
            math_box.depth = math_box.depth.max(radius - cy + t);
            Ok(math_box)
        }
        Frame::Phase => {
            // steinmetz.sty: the angle's foot .35ex below the content, its side rising with slope 2
            // to the content's height, the content starting where the side ends; lines of .6pt
            let t = em(e, 0.06);
            let foot = b.depth + 0.35 * k(e, |c| c.accent_base_height());
            let run = (foot + b.height) / 2.0;
            let width = b.width + run;
            let mut math_box = MathBox::space(width);
            math_box.place(&b, run, 0.0);
            let y = -foot + t / 2.0;
            math_box.items.push(Item::Line { x1: 0.0, y1: y, x2: width, y2: y, thickness: t, color: None });
            math_box.items.push(Item::Line { x1: 0.0, y1: y, x2: run, y2: b.height, thickness: t, color: None });
            math_box.depth = math_box.depth.max(foot + t);
            Ok(math_box)
        }
        Frame::ActuarialAngle => {
            // actuarialangle.sty: 1mu, the content and 2mu under an \overline rule, a rule down its
            // right side to the depth of a \mathstrut, 1mu
            let mu = em(e, 1.0 / 18.0);
            let t = k(e, |c| c.overbar_rule_thickness());
            let gap = k(e, |c| c.overbar_vertical_gap());
            let inner = b.width + 2.0 * mu;
            let top = b.height + gap;
            let strut = glyph('(', e).map_or(b.depth, |p| p.depth);
            let bottom = b.depth.max(strut);
            let mut math_box = MathBox::space(mu + inner + t + mu);
            math_box.place(&b, mu, 0.0);
            math_box.items.push(Item::Rule { x: mu, y: top, width: inner + t, height: t, color: None });
            math_box.items.push(Item::Rule { x: mu + inner, y: -bottom, width: t, height: top + bottom, color: None });
            math_box.height = top + t + k(e, |c| c.overbar_extra_ascender());
            math_box.depth = bottom;
            Ok(math_box)
        }
    }
}
