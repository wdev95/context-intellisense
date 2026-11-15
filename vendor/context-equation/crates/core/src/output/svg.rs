//! SVG with a contract (spec/svg.md): outlines plus speech, source, tree and a text layer.

use std::collections::HashSet;
use std::fmt::Write;

use crate::font::{glyph_font, FallbackFont, Font, MathFont};
use crate::layout::{Item, MathBox};
use crate::locale::Locale;
use crate::VERSION;

pub struct Content<'a> {
    pub math_box: &'a MathBox,
    /// S-SVG-12: the lines of a broken formula, in final coordinates; empty for one line.
    pub lines: &'a [MathBox],
    pub source: &'a str,
    /// The notation of the source: `latex`, `mathml` or `asciimath`.
    pub input: crate::Input,
    pub tree_json: &'a str,
    pub speech: &'a str,
    pub text: &'a str,
    pub locale: Locale,
    pub warnings: &'a [crate::Applied],
    /// The font the equation was set in: its glyphs are drawn from it.
    pub font: MathFont,
    /// S-SVG-10: each element numbered by the order the layout set it (`data-step`).
    pub steps: bool,
    /// S-SVG-10: the equation writes itself in that order (CSS in the SVG; steps included).
    pub animate: bool,
}

/// S-SVG-10: the steps of the drawing, one per glyph, rule, line or ellipse, in the order the layout
/// set them: each one numbered and, animated, timed. Off, it writes nothing.
struct Steps {
    on: bool,
    animate: bool,
    count: usize,
}

/// How a step comes in: a glyph fades in, a rule grows along its length (wide from the left, tall
/// from the top), a line or an ellipse is drawn along its length.
#[derive(Clone, Copy)]
enum Entry {
    Fade,
    Wide,
    Tall,
    Drawn,
}

impl Steps {
    fn next(&mut self, entry: Entry) -> String {
        if !self.on {
            return String::new();
        }
        let i = self.count;
        self.count += 1;
        if !self.animate {
            return format!(r#" data-step="{i}""#);
        }
        let (class, extra) = match entry {
            Entry::Fade => ("mr-f", ""),
            Entry::Wide => ("mr-w", ""),
            Entry::Tall => ("mr-v", ""),
            Entry::Drawn => ("mr-d", r#" pathLength="1""#),
        };
        format!(r#" data-step="{i}" class="{class}"{extra} style="--i:{i}""#)
    }

    /// The seconds between two steps: the whole equation in about three seconds, however long.
    fn interval(&self) -> f32 {
        (3.0 / self.count.max(1) as f32).min(0.15)
    }

    /// The rules, the same in every equation: an inline SVG's `<style>` applies to the whole page, so
    /// what differs (the interval) is a property of each `<svg>`, `--mr-step`.
    fn style(&self) -> &'static str {
        if !self.animate || self.count == 0 {
            return "";
        }
        "<style>@keyframes mr-f{from{opacity:0}}@keyframes mr-w{from{transform:scaleX(0)}}@keyframes mr-v{from{transform:scaleY(0)}}\
         @keyframes mr-d{from{stroke-dashoffset:1.01}}\
         .mr-f{animation:mr-f .4s ease-out both}\
         .mr-w{transform-box:fill-box;transform-origin:left center;animation:mr-w .5s ease-in-out both}\
         .mr-v{transform-box:fill-box;transform-origin:center bottom;animation:mr-v .5s ease-in-out both}\
         .mr-d{stroke-dasharray:1 2;animation:mr-d .6s ease-in-out both}\
         .mr-f,.mr-w,.mr-v,.mr-d{animation-delay:calc(var(--i)*var(--mr-step,.1s))}\
         @media (prefers-reduced-motion:reduce){.mr-f,.mr-w,.mr-v,.mr-d{animation:none;stroke-dasharray:none}}</style>"
    }
}

pub(crate) fn n(v: f32) -> String {
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

pub fn generate(c: &Content) -> String {
    let f = c.font.get();
    let em = f.units_per_em();
    let math_box = c.math_box;
    let id = id_prefix(c);
    let mut s = String::new();

    // S-SVG-1: size in em and alignment on the baseline. The view covers the box and the ink that
    // passes it (the hook of a bold f at the end, a cancel's line): an SVG clips what falls outside.
    let (mut left, mut right, mut top, mut bottom) = (0.0f32, math_box.width, math_box.height, math_box.depth);
    if let Some((x0, y0, x1, y1)) = math_box.ink(f) {
        (left, right, top, bottom) = (left.min(x0), right.max(x1), top.max(y1), bottom.max(-y0));
    }
    let (width, total_height) = (right - left, top + bottom);
    // the drawing first: the <svg> carries how many steps it has
    let mut steps = Steps { on: c.steps || c.animate, animate: c.animate, count: 0 };
    let body = items_svg(math_box, c.lines, &id, &mut steps);
    let animated = if steps.animate && steps.count > 0 { format!("; --mr-step: {}s", n(steps.interval())) } else { String::new() };
    let mut counted = if steps.on { format!(r#" data-steps="{}""#, steps.count) } else { String::new() };
    if !c.lines.is_empty() {
        let _ = write!(counted, r#" data-lines="{}""#, c.lines.len());
    }
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:context-equation="urn:context-equation:1" viewBox="{} {} {} {}" width="{}em" height="{}em" style="vertical-align: {}em{animated}" role="img" aria-labelledby="{id}-speech" lang="{}" data-context-equation-version="{}"{counted}>"#,
        n(left),
        n(-top),
        n(width),
        n(total_height),
        n(width / em),
        n(total_height / em),
        n(-bottom / em),
        c.locale,
        VERSION,
        id = id,
    );

    // S-SVG-2: speech for screen readers
    let _ = write!(s, r#"<title id="{id}-speech">{}</title>"#, escape(c.speech));

    // S-SVG-3: source and tree, to recover the equation
    let _ = write!(
        s,
        r#"<metadata><context-equation:source type="{}">{}</context-equation:source><context-equation:tree>{}</context-equation:tree><context-equation:font>{}</context-equation:font>{}</metadata>"#,
        c.input,
        escape(c.source),
        escape(c.tree_json),
        c.font.name(),
        c.warnings.iter().map(|a| format!(r#"<context-equation:warning rule="{}">{}</context-equation:warning>"#, a.rule, escape(&a.description))).collect::<String>(),
    );

    // S-SVG-4: color. Embedded in a page, it inherits the text color (currentColor).
    // As a standalone document (<img>, opened file), svg:root matches and uses a fixed dark ink:
    // the image does not know the page theme, and following the system theme vanishes on a light page.
    // A <style> inside an inline SVG applies to the whole page: hence nothing beyond svg:root.
    s.push_str("<style>svg:root{color:#111}</style>");
    s.push_str(steps.style());

    // S-SVG-5: each glyph drawn only once, in order of first appearance (deterministic)
    s.push_str("<defs>");
    glyph_defs(&math_box.items, &id, f, &mut HashSet::new(), &mut s);
    s.push_str("</defs>");

    s.push_str(&body);

    // S-SVG-6: transparent text layer, to select and copy when the SVG is inline in the page
    let _ = write!(
        s,
        r#"<text x="0" y="0" font-size="{}" textLength="{}" lengthAdjust="spacingAndGlyphs" fill="transparent" aria-hidden="true">{}</text>"#,
        n(em),
        n(math_box.width),
        escape(c.text),
    );

    s.push_str("</svg>");
    s
}

/// The name of a glyph's `<path>` after the equation's id: `g42` for the math font's glyph 42, and
/// `f1a2b3c4g42` for a fallback font's, the font's tag first (S-SVG-5).
pub(crate) fn glyph_name(font: Option<FallbackFont>, g: crate::font::GlyphId) -> String {
    match font {
        Some(f) => format!("f{:08x}g{}", f.tag(), g.0),
        None => format!("g{}", g.0),
    }
}

/// What tells two glyphs apart: the fallback font's tag (none for the math font) and the glyph id.
pub(crate) type GlyphKey = (Option<u32>, u16);

/// S-SVG-5: the outline of each glyph, once, in order of first appearance, inside turned boxes too.
fn glyph_defs(items: &[Item], id: &str, f: &Font, seen: &mut HashSet<GlyphKey>, s: &mut String) {
    for item in items {
        match item {
            Item::Glyph { id: g, font, .. } if seen.insert((font.map(FallbackFont::tag), g.0)) => {
                let _ = write!(s, r#"<path id="{id}-{}" d="{}"/>"#, glyph_name(*font, *g), glyph_font(*font, f).outline(*g));
            }
            Item::Transformed { items, .. } | Item::Group { items, .. } => glyph_defs(items, id, f, seen, s),
            _ => {}
        }
    }
}

/// S-SVG-5 and S-SVG-10: the drawing, glyphs as `<use>` of their `<path>`, in the order the layout
/// set them; S-SVG-12: a broken formula, one `<g data-line="k">` per line.
fn items_svg(math_box: &MathBox, lines: &[MathBox], id: &str, steps: &mut Steps) -> String {
    let mut out = String::new();
    out.push_str(r#"<g fill="currentColor" stroke="none" transform="scale(1,-1)">"#);
    if lines.is_empty() {
        item_svg(&math_box.items, id, steps, &mut out);
    }
    for (k, line) in lines.iter().enumerate() {
        let _ = write!(out, r#"<g data-line="{k}">"#);
        item_svg(&line.items, id, steps, &mut out);
        out.push_str("</g>");
    }
    out.push_str("</g>");
    out
}

fn item_svg(items: &[Item], id: &str, steps: &mut Steps, out: &mut String) {
    for item in items {
        match item {
            Item::Glyph { id: g, x, y, scale, color, flip, font } => {
                let transform = if *flip {
                    format!(r#"transform="translate({} {}) scale({} {})""#, n(*x), n(*y), n(-*scale), n(*scale))
                } else if (*scale - 1.0).abs() < 1e-4 {
                    format!(r#"x="{}" y="{}""#, n(*x), n(*y))
                } else {
                    format!(r#"transform="translate({} {}) scale({})""#, n(*x), n(*y), n(*scale))
                };
                let _ = write!(out, r##"<use href="#{id}-{}" {transform}{}{}/>"##, glyph_name(*font, *g), fill(color), steps.next(Entry::Fade));
            }
            Item::Rule { x, y, width, height, color } => {
                let entry = if width >= height { Entry::Wide } else { Entry::Tall };
                let _ = write!(out, r#"<rect x="{}" y="{}" width="{}" height="{}"{}{}/>"#, n(*x), n(*y), n(*width), n(*height), fill(color), steps.next(entry));
            }
            Item::Line { x1, y1, x2, y2, thickness, color } => {
                let _ = write!(
                    out,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"{}/>"#,
                    n(*x1),
                    n(*y1),
                    n(*x2),
                    n(*y2),
                    color.as_deref().unwrap_or("currentColor"),
                    n(*thickness),
                    steps.next(Entry::Drawn)
                );
            }
            Item::Ellipse { cx, cy, rx, ry, thickness, color } => {
                let _ = write!(
                    out,
                    r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}"{}/>"#,
                    n(*cx),
                    n(*cy),
                    n(*rx),
                    n(*ry),
                    color.as_deref().unwrap_or("currentColor"),
                    n(*thickness),
                    steps.next(Entry::Drawn)
                );
            }
            Item::Transformed { matrix: [a, b, c, d, e, f], items } => {
                let _ = write!(out, r#"<g transform="matrix({} {} {} {} {} {})">"#, n(*a), n(*b), n(*c), n(*d), n(*e), n(*f));
                item_svg(items, id, steps, out);
                out.push_str("</g>");
            }
            // S-SVG-11: the attributes of \class, \htmlId… on a group around the content
            Item::Group { attributes, items, .. } => {
                out.push_str("<g");
                for (name, value) in attributes {
                    let _ = write!(out, r#" {}="{}""#, escape(name), escape(value));
                }
                out.push('>');
                item_svg(items, id, steps, out);
                out.push_str("</g>");
            }
        }
    }
}

/// S-SVG-7: unique ids per equation, so several equations on one page do not clash.
/// Derived from the input (FNV-1a), so the same equation always gets the same ids (D-DET-1).
fn id_prefix(c: &Content) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    // a loaded font's hash too; the embedded fonts keep the ids they always had
    let salt = c.font.id_salt().map(u64::to_le_bytes);
    for b in c.source.bytes().chain(c.locale.code().bytes()).chain(VERSION.bytes()).chain(salt.into_iter().flatten()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("context-equation-{:08x}", h as u32)
}

fn fill(color: &Option<String>) -> String {
    match color {
        Some(c) => format!(r#" fill="{}""#, escape(c)),
        None => String::new(),
    }
}

pub(crate) fn escape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            // XML 1.0 cannot represent these, not even escaped (S-SVG-8)
            '\t' | '\n' | '\r' => output.push(c),
            '\u{0}'..='\u{1F}' | '\u{FFFE}' | '\u{FFFF}' => output.push('\u{FFFD}'),
            other => output.push(other),
        }
    }
    output
}
