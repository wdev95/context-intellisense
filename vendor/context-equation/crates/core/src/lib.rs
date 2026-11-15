//! contextEquation: equations as a tree of meaning, drawn with explicit rules.
//!
//! It reads math LaTeX (and MathML and AsciiMath), commutative diagrams (amscd, tikz-cd, xymatrix)
//! and chemistry (mhchem formulas, chemfig structures, reaction schemes and mechanism arrows), and
//! writes an accessible SVG, its parsed tree, diagnostics, and editor hit boxes.
//!
//! ```
//! let svg = context_equation_core::tex_to_svg("3.14").unwrap();
//! assert!(svg.contains("-speech\">3.14</title>"));
//!
//! # #[cfg(feature = "chem")] {
//! let r = context_equation_core::convert(r"\chemfig{H_3C-*6(=-=-=-)}", &context_equation_core::Options::default()).unwrap();
//! assert_eq!(r.speech, "structure of C7 H8, with 1 aromatic ring");
//! # }
//! ```
//!
//! # Features
//!
//! - `chem` (default): structural chemistry, the `chem` module (`spec/chemistry.md`). Without it,
//!   `\chemfig` and `\schemestart` are unsupported commands.
//! - `latin-modern`: Latin Modern Math as a second font, chosen with [`convert_in_font`]; some 700 KB
//!   more in the binary.
//! - `emoji`: Noto Emoji (monochrome, OFL) as the default fallback font ([`Options::fallback_fonts`]),
//!   for the emoji the math font lacks; about 780 KB more in the binary.

#[cfg(feature = "chem")]
pub mod chem;
pub mod composite;
pub mod font;
pub mod input;
pub mod layout;
pub mod locale;
pub mod output;
pub mod rules;
pub mod tree;

use std::fmt;

pub use locale::Locale;
pub use rules::Applied;
pub use tree::{Class, Node};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The notation the equation is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Input {
    /// Math LaTeX. MathML pasted here (starting with `<math`) is recognized and read as MathML (L-ENT-4).
    #[default]
    Latex,
    /// Presentation MathML: what Word, Google Docs, MathType and MathJax export.
    MathMl,
    /// AsciiMath (asciimath.org): `sum_(i=1)^n i^2`.
    AsciiMath,
}

impl Input {
    pub const ALL: [Input; 3] = [Input::Latex, Input::MathMl, Input::AsciiMath];

    pub fn code(self) -> &'static str {
        match self {
            Input::Latex => "latex",
            Input::MathMl => "mathml",
            Input::AsciiMath => "asciimath",
        }
    }
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::str::FromStr for Input {
    type Err = String;

    /// `latex` (or `tex`), `mathml` (or `mml`), `asciimath` (or `am`), in any case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "latex" | "tex" => Ok(Input::Latex),
            "mathml" | "mml" => Ok(Input::MathMl),
            "asciimath" | "am" => Ok(Input::AsciiMath),
            other => Err(format!("unknown input format: {other} (use latex, mathml or asciimath)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub locale: Locale,
    /// Display equation (like `$$…$$`), or inline with the text (like `$…$`).
    pub display: bool,
    /// Refuse invalid LaTeX, like TeX does. Without it, contextEquation recovers what it can and returns warnings.
    pub strict: bool,
    /// The notation of the input. LaTeX by default.
    pub input: Input,
    /// The equation writes itself in the browser, glyph by glyph and stroke by stroke, in the order
    /// the layout set it (CSS in the SVG, S-SVG-10). Off by default; without it the SVG is unchanged.
    pub animate: bool,
    /// Each element of the SVG numbered by that order (`data-step`, and `data-steps` on the `<svg>`),
    /// with no CSS: for a page that walks through the equation itself. The animation numbers them too.
    pub steps: bool,
    /// Text kerned as TeX's OT1 or T1 fonts kern it; none by default.
    pub text_kerning: layout::kerning::TextKerning,
    /// The math font the equation is set in: STIX Two Math by default, or one loaded with
    /// [`font::MathFont::load`].
    pub font: font::MathFont,
    /// The widest a line may be, in em: a wider formula is broken after a relation, or after `+`
    /// and `−` when relations alone are not enough, at its top level only (L-BRK-1); each line is
    /// a `<g data-line>` of the SVG (S-SVG-12). `None` (the default): one line, however wide.
    /// [`convert`] breaks lines; [`typeset`] and [`pen`] draw one line.
    pub max_width: Option<f32>,
    /// Fonts for the characters the math font lacks (emoji, CJK in `\text{…}`), looked up in order;
    /// a character none has is drawn as a box (E-REC-8) or, strict, refused. Noto Emoji with the
    /// `emoji` feature ([`font::FallbackFont::defaults`]), else none.
    pub fallback_fonts: Vec<font::FallbackFont>,
    /// The numbers of the document's labels, which an equation converted alone cannot know:
    /// `{"eq:a": "3"}` makes `\eqref{eq:a}` show `(3)` and `\ref{eq:a}` show `3`. A label missing here
    /// (and not numbered by a `\tag` in the equation) is `??`, with a warning (E-REC-9; an error in
    /// strict mode), as on LaTeX's first run.
    pub labels: Labels,
    /// The numbers of the document's citations: `{"knuth": "1"}` makes `\cite{knuth}` show `[1]`. A
    /// key missing here is `?`, with a warning (E-REC-9; an error in strict mode).
    pub citations: Labels,
}

/// Keys and the numbers they stand for, for [`Options::labels`] and [`Options::citations`].
pub use input::latex::Labels;

impl Default for Options {
    fn default() -> Self {
        Options {
            locale: Locale::default(),
            display: true,
            strict: false,
            input: Input::Latex,
            animate: false,
            steps: false,
            text_kerning: Default::default(),
            font: font::MathFont::StixTwo,
            max_width: None,
            fallback_fonts: font::FallbackFont::defaults(),
            labels: Labels::new(),
            citations: Labels::new(),
        }
    }
}

/// The tree of meaning of an equation, in the notation `options.input` says, and the labels it numbers.
/// `font` in place of `options.font` (the `_in_font` functions).
fn read(source: &str, options: &Options, font: font::MathFont, registry: &mut rules::Registry) -> Result<(Node, Input, Labels), Error> {
    let input = match options.input {
        Input::Latex if input::mathml::looks_like_mathml(source) => {
            registry.note(rules::L_ENT_4, "MathML pasted as LaTeX: read as MathML");
            Input::MathMl
        }
        other => other,
    };
    let (tree, labels) = match input {
        Input::Latex => input::latex::read_in_document(source, options.locale, registry, options.strict, font, &options.labels, &options.citations)?,
        Input::MathMl => (input::mathml::read_with(source, options.locale, registry, options.strict)?, Labels::new()),
        Input::AsciiMath => (input::asciimath::read_with(source, options.locale, registry, options.strict)?, Labels::new()),
    };
    Ok((tree, input, labels))
}

#[derive(Debug, Clone)]
pub struct Output {
    pub svg: String,
    pub tree: Node,
    pub speech: String,
    /// Input problems the tolerant mode worked around (empty in strict mode).
    pub warnings: Vec<Applied>,
    /// The labels this equation numbers itself, a `\label` with a `\tag` on the same line
    /// (`\label{eq:a} \tag{3}` gives `{"eq:a": "3"}`): add them to [`Options::labels`] to convert the
    /// equations that come after it, as the CLI's `--batch` does.
    pub labels: Labels,
    /// The boxes of the groups the equation marks (`\htmlData`, `\htmlId`, `\class`…), in the SVG's
    /// coordinates: where an editor reads where each marked piece sits. Empty when nothing is marked.
    pub boxes: Vec<output::boxes::GroupBox>,
}

impl Output {
    /// The editor-facing SVG, accessible description, tree, warnings, and hit boxes.
    pub fn to_json(&self) -> String {
        let rules = |list: &[Applied]| -> Vec<serde_json::Value> { list.iter().map(|a| serde_json::json!({ "rule": a.rule, "description": a.description })).collect() };
        let tree: serde_json::Value = serde_json::from_str(&self.tree.to_json()).expect("the tree serializes to valid JSON");
        serde_json::json!({
            "svg": self.svg,
            "speech": self.speech,
            "tree": tree,
            "warnings": rules(&self.warnings),
            "boxes": self.boxes.iter().map(output::boxes::GroupBox::to_json).collect::<Vec<_>>(),
        })
        .to_string()
    }

}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Syntax { column: usize, message: String },
    MissingGlyph { character: char },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax { column, message } => write!(f, "column {column}: {message}"),
            Error::MissingGlyph { character } => {
                write!(f, "the math font has no '{character}' character (U+{:04X})", *character as u32)
            }
        }
    }
}

impl std::error::Error for Error {}

/// Converts an equation (LaTeX by default; see [`Options::input`]) to SVG, speech, text and tree.
pub fn convert(tex: &str, options: &Options) -> Result<Output, Error> {
    convert_in_font(tex, options, options.font)
}

/// [`convert`], with the equation set in `font` (in place of [`Options::font`]).
pub fn convert_in_font(tex: &str, options: &Options, font: font::MathFont) -> Result<Output, Error> {
    let mut registry = rules::Registry::default();
    let (tree, input, labels) = read(tex, options, font, &mut registry)?;
    let style = if options.display { layout::Style::display() } else { layout::Style::text() }.in_font(font);
    let math_box = {
        let mut ctx =
            layout::Context { locale: options.locale, registry: &mut registry, strict: options.strict, kerning: options.text_kerning, fallbacks: &options.fallback_fonts };
        layout::compose_list(&tree, style, &mut ctx)?
    };
    finish(tex, options, tree, input, style, math_box, registry, labels)
}

/// What follows layout: line breaking, the accessible description, and the SVG. A frame
/// of its own, not on the stack while the deep recursions of reading and layout run.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn finish(
    tex: &str,
    options: &Options,
    tree: Node,
    input: Input,
    style: layout::Style,
    math_box: layout::MathBox,
    mut registry: rules::Registry,
    labels: Labels,
) -> Result<Output, Error> {
    let formula = {
        let mut ctx =
            layout::Context { locale: options.locale, registry: &mut registry, strict: options.strict, kerning: options.text_kerning, fallbacks: &options.fallback_fonts };
        layout::break_lines(&tree, math_box, style, &mut ctx, options.max_width)?
    };
    let speech = output::speech::speak(&tree, options.locale);
    let text = output::text::linear(&tree, options.locale);
    let tree_json = tree.to_json();
    registry.dedup();
    let svg = output::svg::generate(&output::svg::Content {
        math_box: &formula.math_box,
        lines: &formula.lines,
        source: tex,
        input,
        tree_json: &tree_json,
        speech: &speech,
        text: &text,
        locale: options.locale,
        warnings: &registry.warnings,
        // the font `convert_in_font` was given, which may not be `options.font`
        font: style.font,
        steps: options.steps,
        animate: options.animate,
    });
    let boxes = output::boxes::group_boxes(&formula.math_box, &formula.lines);
    Ok(Output { svg, tree, speech, warnings: registry.warnings, labels, boxes })
}

/// The composed box, without generating SVG: for callers that draw the equation inside something
/// else (tikra's figure labels). Also returns the tolerant mode's warnings.
pub fn typeset(tex: &str, options: &Options) -> Result<(layout::MathBox, Vec<Applied>), Error> {
    typeset_in_font(tex, options, options.font)
}

/// [`typeset`], with the equation set in `font` (in place of [`Options::font`]; its glyph ids are
/// that font's).
pub fn typeset_in_font(tex: &str, options: &Options, font: font::MathFont) -> Result<(layout::MathBox, Vec<Applied>), Error> {
    let mut registry = rules::Registry::default();
    let (tree, _, _) = read(tex, options, font, &mut registry)?;
    let style = if options.display { layout::Style::display() } else { layout::Style::text() }.in_font(font);
    let mut ctx = layout::Context { locale: options.locale, registry: &mut registry, strict: options.strict, kerning: options.text_kerning, fallbacks: &options.fallback_fonts };
    let math_box = layout::compose_list(&tree, style, &mut ctx)?;
    registry.dedup();
    Ok((math_box, registry.warnings))
}

/// The equation written by a pen: an SVG where a pen tip goes over each glyph's outline, rule and
/// line, in the order the layout set them, and each fills in once drawn; SMIL, in a loop.
pub fn pen(tex: &str, options: &Options, pen: &output::pen::Pen) -> Result<String, Error> {
    let font = options.font;
    let mut registry = rules::Registry::default();
    let (tree, _, _) = read(tex, options, font, &mut registry)?;
    let style = if options.display { layout::Style::display() } else { layout::Style::text() }.in_font(font);
    let mut ctx = layout::Context { locale: options.locale, registry: &mut registry, strict: options.strict, kerning: options.text_kerning, fallbacks: &options.fallback_fonts };
    let math_box = layout::compose_list(&tree, style, &mut ctx)?;
    let speech = output::speech::speak(&tree, options.locale);
    Ok(output::pen::generate(&math_box, font, tex, &speech, pen))
}

/// The tree of meaning as MathML (spec/mathml.md), for a tree built or changed by hand; [`convert`]
/// already returns it in [`Output::mathml`]. The fallback's annotation, if any, holds `source`.
pub fn mathml(tree: &Node, locale: Locale, display: bool, source: &str) -> String {
    output::mathml::generate(tree, locale, display, source, Input::Latex).text
}

/// Shortcut with the default locale (English).
pub fn tex_to_svg(tex: &str) -> Result<String, Error> {
    convert(tex, &Options::default()).map(|r| r.svg)
}
