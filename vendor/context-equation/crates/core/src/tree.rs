//! Tree of meaning: each node says what the thing is, not how it is drawn.
//! The form (comma or dot, "sen" or "sin", where limits go) is decided later,
//! by the locale and the style.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Node {
    /// Number kept in canonical spelling, with a decimal point: "3.14".
    Number {
        value: String,
        /// Group thousands (`\num`). Plain numbers are not grouped.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        group: bool,
        /// L-NUM-3: thousands separator the author wrote (`1.000` in pt-BR), kept in the drawing.
        #[serde(skip_serializing_if = "Option::is_none")]
        thousands: Option<char>,
        /// #203: the alphabet of its digits (`\mathbf{1}` is 𝟏): bold, sans-serif, bold sans-serif or
        /// monospace, the ones Unicode has digits for; upright otherwise.
        #[serde(skip_serializing_if = "Variant::is_upright")]
        variant: Variant,
    },
    /// Single-letter variable or constant: "x".
    Identifier {
        name: String,
        #[serde(skip_serializing_if = "Variant::is_default")]
        variant: Variant,
    },
    /// Named function, stored by its canonical English name: "sin", "lim".
    Function {
        name: String,
        /// `lim`, `max`, `min`: limits below in display style.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        limits: bool,
    },
    /// Operator, relation, punctuation or delimiter.
    Operator {
        #[serde(serialize_with = "composite_json::symbol")]
        symbol: char,
        class: Class,
    },
    /// Fixed-size delimiter: `\big(`, `\Bigg]`. Size from 1 to 4.
    FixedDelimiter {
        #[serde(serialize_with = "composite_json::symbol")]
        symbol: char,
        class: Class,
        size: u8,
    },
    /// Sum, product, integral. `limits`: None lets the style decide.
    LargeOperator {
        #[serde(serialize_with = "composite_json::symbol")]
        symbol: char,
        #[serde(skip_serializing_if = "Option::is_none")]
        limits: Option<bool>,
    },
    /// Horizontal sequence.
    Row {
        children: Vec<Node>,
    },
    Fraction {
        num: Box<Node>,
        den: Box<Node>,
        #[serde(skip_serializing_if = "FractionStyle::is_default")]
        style: FractionStyle,
        /// `\binom` has no rule.
        #[serde(skip_serializing_if = "std::clone::Clone::clone")]
        rule: bool,
        /// The rule's thickness in em when the input gives one (`\above`, `\genfrac`); the font's otherwise.
        #[serde(skip_serializing_if = "Option::is_none")]
        thickness: Option<f32>,
        /// `\cfrac[l]`, `\cfrac[r]`: the numerator at the left (`'l'`) or the right (`'r'`); centered otherwise.
        #[serde(skip_serializing_if = "Option::is_none")]
        numerator_align: Option<char>,
    },
    Root {
        radicand: Box<Node>,
        #[serde(skip_serializing_if = "Option::is_none")]
        index: Option<Box<Node>>,
    },
    /// Base with superscript and/or subscript. Does not yet tell powers from indices.
    Scripts {
        base: Box<Node>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sup: Option<Box<Node>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sub: Option<Box<Node>>,
    },
    /// `\left( … \middle| … \right)`: delimiters that follow the height of the content.
    Delimited {
        #[serde(skip_serializing_if = "Option::is_none", serialize_with = "composite_json::optional")]
        open: Option<char>,
        #[serde(skip_serializing_if = "Option::is_none", serialize_with = "composite_json::optional")]
        close: Option<char>,
        parts: Vec<Node>,
        #[serde(skip_serializing_if = "Vec::is_empty", serialize_with = "composite_json::list")]
        middles: Vec<char>,
    },
    /// Matrices, cases, alignments and `array`.
    Table {
        shape: TableKind,
        rows: Vec<Vec<Node>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        columns: Vec<char>,
        /// Vertical rules: column indices preceded by `|`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        verticals: Vec<usize>,
        /// Dashed vertical rules (arydshln): column indices preceded by `:`.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        dashed: Vec<usize>,
        /// Horizontal rules (`\hline`, `\cline{a-b}`) before a table row.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        horizontals: Vec<TableRule>,
        /// Content of `@{…}` in place of the inter-column space: boundary index (0 = before the 1st column).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        between: Vec<(usize, Node)>,
        /// Rows that are full-width running text (`\intertext`).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        intertexts: Vec<usize>,
        /// `\arraystretch`: how much taller each row's strut is (1 by default).
        #[serde(skip_serializing_if = "is_one")]
        stretch: f32,
        /// `\arraycolsep`: half the space between two columns of an array or a matrix, in thousandths of
        /// an em (500, 5pt, by default; `\setlength{\arraycolsep}{…}` changes it). A `u16` keeps the node
        /// at 176 bytes; the JSON writes it in em.
        #[serde(skip_serializing_if = "is_default_colsep", serialize_with = "milli_em")]
        colsep: u16,
    },
    /// Accent over the base: hat, bar, vector, tilde, dot. `below`: under the base (`\underrightarrow`).
    Accent {
        base: Box<Node>,
        accent: char,
        stretchy: bool,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        below: bool,
    },
    /// `\overline`, `\underline`.
    Bar {
        base: Box<Node>,
        above: bool,
    },
    /// `\overbrace`, `\underbrace`, with an optional label. `bracket`: square (`\overbracket`).
    Brace {
        base: Box<Node>,
        above: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<Box<Node>>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        bracket: bool,
    },
    /// Running text inside the equation, with math snippets (`\text{if $x>0$}`).
    Text {
        parts: Vec<TextPart>,
    },
    /// Explicit space in em: `\,` `\quad`.
    Space {
        em: f32,
    },
    Canceled {
        base: Box<Node>,
        shape: Cancel,
    },
    Framed {
        base: Box<Node>,
        #[serde(skip_serializing_if = "Frame::is_default")]
        shape: Frame,
        /// `\colorbox`, `\fcolorbox`: filled behind the content.
        #[serde(skip_serializing_if = "Option::is_none")]
        background: Option<String>,
        /// `\fcolorbox`: color of the frame line.
        #[serde(skip_serializing_if = "Option::is_none")]
        border: Option<String>,
        /// Space between the content and the frame, in em, when the input gives one (`\bbox[5px]`);
        /// the `\fboxsep` of LaTeX otherwise.
        #[serde(skip_serializing_if = "Option::is_none")]
        padding: Option<f32>,
        /// Thickness of the frame line, in em, when the input gives one (`\bbox[border:2px solid]`).
        #[serde(skip_serializing_if = "Option::is_none")]
        thickness: Option<f32>,
    },
    /// Arrow that stretches to fit its text: `\xrightarrow[below]{above}`.
    Arrow {
        symbol: char,
        #[serde(skip_serializing_if = "Option::is_none")]
        above: Option<Box<Node>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        below: Option<Box<Node>>,
    },
    /// `\overset`, `\underset`, `\stackrel`: something above or below, at script size.
    Stacked {
        base: Box<Node>,
        #[serde(skip_serializing_if = "Option::is_none")]
        above: Option<Box<Node>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        below: Option<Box<Node>>,
    },
    /// Takes the space of the base without drawing it (`\phantom`).
    Phantom {
        base: Box<Node>,
        width: bool,
        height: bool,
    },
    /// `\mathrel`, `\mathbin`, `\mathop`…: changes the atom class of the base.
    ForcedClass {
        class: Class,
        base: Box<Node>,
    },
    /// `\displaystyle` and friends: fixed style for the rest of the group. 0 display, 1 text, 2 script, 3 scriptscript.
    Style {
        level: u8,
        base: Box<Node>,
    },
    Colored {
        color: String,
        base: Box<Node>,
    },
    /// HTML attributes for the content's group in the SVG (`class`, `id`, `style`, `data-*`), so a
    /// page can style or animate it: `\class`, `\cssId`, `\style` (MathJax) and `\htmlClass`,
    /// `\htmlId`, `\htmlStyle`, `\htmlData` (KaTeX). The reader keeps only safe names and values;
    /// speech, text and braille ignore them.
    Attributed {
        attributes: Vec<(String, String)>,
        base: Box<Node>,
    },
    /// Quantity with a unit (`\SI`, `\qty`): value and unit in upright form.
    Quantity {
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        unit: Box<Node>,
    },
    /// One siunitx unit (`\kilo\gram`): `base` draws its symbol (kg); `prefix` and `name` are the
    /// siunitx names it was written with, for the speech ("kilograms").
    Unit {
        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
        name: String,
        base: Box<Node>,
    },
    /// Table cell spanning several columns (`\multicolumn`, `\hdotsfor`).
    Multicolumn {
        columns: usize,
        alignment: char,
        base: Box<Node>,
    },
    /// `\raisebox{lift}[height][depth]`: shifts the content vertically, in em, and gives it the
    /// height and depth asked for, if any.
    Raised {
        em: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<BoxLength>,
        #[serde(skip_serializing_if = "Option::is_none")]
        depth: Option<BoxLength>,
        base: Box<Node>,
    },
    /// `\scalebox{factor}[vertical]`: changes the size of the content; a negative factor mirrors it.
    Scaled {
        factor: f32,
        /// The vertical factor, when it is not `factor`.
        #[serde(skip_serializing_if = "Option::is_none")]
        vertical: Option<f32>,
        base: Box<Node>,
    },
    /// `\resizebox{width}{height}` (graphicx): the content scaled to that size; `None` (`!`) keeps the
    /// ratio. With `total` (`\resizebox*`), the height is the height plus the depth.
    Resized {
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<BoxLength>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<BoxLength>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        total: bool,
        base: Box<Node>,
    },
    /// `\rotatebox[origin=…]{degrees}{…}` (graphicx): the content turned counterclockwise around a point
    /// of its box: `origin` holds graphicx's letters (l, r, c, t, b, B); the default is the left end of
    /// the baseline.
    Rotated {
        degrees: f32,
        #[serde(skip_serializing_if = "String::is_empty")]
        origin: String,
        base: Box<Node>,
    },
    /// `\rule`: filled rectangle, sizes in em.
    Rule {
        width: f32,
        height: f32,
        raise: f32,
    },
    /// `\mathchoice{display}{text}{script}{scriptscript}`: the branch of the style it is set in.
    Choice {
        branches: Vec<Node>,
    },
    /// `\vcenter`: the content centered on the math axis.
    VCenter {
        base: Box<Node>,
    },
    /// `\reflectbox` (graphicx): the content mirrored left to right.
    Reflected {
        base: Box<Node>,
    },
    /// `\smash`: draws, but takes no height (and/or depth).
    Smashed {
        base: Box<Node>,
        height: bool,
        depth: bool,
    },
    /// mathtools' `\vdotswithin{=}`: ⋮ centered in a box as wide as `{}={}`, so that it sits under
    /// the symbol of the line above. `short` (`\shortvdotswithin`): its table row is set close to
    /// the rows around it, with no strut.
    VdotsWithin {
        symbol: Box<Node>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        short: bool,
    },
    /// `\mathllap`, `\mathclap`, `\mathrlap`: draws without taking width. Side: 'l', 'c' or 'r'.
    Overlapped {
        base: Box<Node>,
        side: char,
    },
    /// Vertical arrow of a commutative diagram (amscd's `@VVV`), with side labels.
    VerticalArrow {
        symbol: char,
        #[serde(skip_serializing_if = "Option::is_none")]
        left: Option<Box<Node>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        right: Option<Box<Node>>,
    },
    /// Commutative diagram whose arrows go between any two cells, straight or bent (tikz-cd, xymatrix).
    ArrowDiagram {
        cells: Vec<Vec<Node>>,
        arrows: Vec<DiagramArrow>,
    },
    /// Structures, reaction schemes and mechanisms (chemfig): L-CHE-2.
    #[cfg(feature = "chem")]
    Chemistry {
        scheme: crate::chem::Scheme,
    },
    /// Long division, Brazilian school layout. contextEquation computes the steps.
    LongDivision {
        dividend: String,
        divisor: String,
    },
    /// Column arithmetic: addition, subtraction or multiplication, computed by contextEquation.
    ColumnArithmetic {
        operation: char,
        terms: Vec<String>,
    },
}

/// A length of graphicx's box commands: `em` plus multiples of the box's own `\width`, `\height` and
/// `\depth` (`\totalheight` is height plus depth), as in `\raisebox{0pt}[\height][0pt]`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct BoxLength {
    #[serde(skip_serializing_if = "is_zero")]
    pub em: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub width: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub height: f32,
    #[serde(skip_serializing_if = "is_zero")]
    pub depth: f32,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum TextPart {
    Text(String),
    Math(Node),
}

/// TeX atom classes (TeXbook, ch. 17). They decide the spacing between neighbors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Variant {
    #[default]
    Italic,
    Upright,
    Bold,
    BoldItalic,
    DoubleStruck,
    Calligraphic,
    Fraktur,
    SansSerif,
    Monospace,
    // #173: the other alphabets of Unicode's Mathematical Alphanumeric Symbols, which unicode-math
    // names (`\mathbfcal`, `\mathbffrak`, `\mathbfsf`, `\mathsfit`, `\mathbfsfit`)
    BoldScript,
    BoldFraktur,
    BoldSansSerif,
    SansSerifItalic,
    SansSerifBoldItalic,
}

impl Variant {
    pub fn is_default(&self) -> bool {
        *self == Variant::Italic
    }

    pub fn is_upright(&self) -> bool {
        *self == Variant::Upright
    }

    /// #203: the alphabet a digit takes in this one: the digits of bold, sans-serif, bold sans-serif and
    /// monospace (U+1D7CE…), which the italic forms share; the others have none and leave it upright.
    /// Double-struck digits are symbols (𝟙, #159), not numbers.
    pub fn of_digits(self) -> Variant {
        match self {
            Variant::Bold | Variant::BoldItalic => Variant::Bold,
            Variant::SansSerif | Variant::SansSerifItalic => Variant::SansSerif,
            Variant::BoldSansSerif | Variant::SansSerifBoldItalic => Variant::BoldSansSerif,
            Variant::Monospace => Variant::Monospace,
            _ => Variant::Upright,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FractionStyle {
    #[default]
    Auto,
    Display,
    Row,
}

impl FractionStyle {
    pub fn is_default(&self) -> bool {
        *self == FractionStyle::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cancel {
    Bar,
    Backslash,
    X,
    /// The Feynman slash of slashed.sty (`\slashed{D}`): a "/" over the base, which keeps its width.
    /// Notation, not a cancellation.
    Slash,
    /// centernot's `\centernot`: the slash of `\not`, at its own size, over the middle of the base.
    CenterNot,
}

/// Horizontal table rule before row `row`, covering columns `from..=to` (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TableRule {
    pub row: usize,
    pub from: usize,
    pub to: usize,
    /// `\hdashline`, `\cdashline` (arydshln).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dashed: bool,
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

fn is_default_colsep(v: &u16) -> bool {
    *v == 500
}

fn milli_em<S: serde::Serializer>(v: &u16, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f32(f32::from(*v) / 1000.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Frame {
    #[default]
    Rectangle,
    Circle,
    /// No line, only the padding (and the background, if any): `\colorbox`.
    Fill,
    /// The Steinmetz angle of a phasor, `\phase` (steinmetz package): a line under the content and
    /// one rising to its left.
    Phase,
    /// The actuarial angle, `\angl` (actuarialangle package): a line over the content and one down
    /// its right side, as in the annuity a with angle n.
    ActuarialAngle,
}

impl Frame {
    pub fn is_default(&self) -> bool {
        *self == Frame::Rectangle
    }
}

/// An arrow of an [`Node::ArrowDiagram`], between cells given as (row, column), from 0.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiagramArrow {
    pub from: (usize, usize),
    pub to: (usize, usize),
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<ArrowLabel>,
    #[serde(skip_serializing_if = "ArrowLine::is_default")]
    pub line: ArrowLine,
    #[serde(skip_serializing_if = "ArrowHead::is_default")]
    pub head: ArrowHead,
    #[serde(skip_serializing_if = "ArrowTail::is_default")]
    pub tail: ArrowTail,
    /// Degrees the arrow leaves its straight path by; positive bends to the left of its direction.
    #[serde(skip_serializing_if = "is_zero_i16")]
    pub bend: i16,
    /// Parallel offset in em, positive to the left of its direction (two arrows between the same cells).
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub shift: f32,
    /// xy-pic's 2-cell curves: a quadratic curve whose control point is the middle of the straight
    /// path moved this many em to its left (negative: to its right). For an `across` arrow, how many em
    /// it is moved along itself.
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub sag: f32,
    /// xy-pic's 2-cell Arrow (`\rtwocell`): not an arrow between the cells but a short one across the
    /// middle of `from`–`to`, from its left side to its right (clockwise, looking from `from`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub across: bool,
    /// Cells the path goes through on its way, in straight segments (xymatrix's `\ar'[d][dd]`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<(usize, usize)>,
    /// xymatrix's `\ar@(d1,d2)` (xy-pic reference manual, note 24p): the directions, in degrees
    /// counterclockwise from the right, the arrow leaves `from` in and enters `to` from. The arrow is
    /// a curve; with `to` equal to `from`, a loop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directions: Option<(i16, i16)>,
}

impl DiagramArrow {
    /// The arrow as the row `from → to`, its first label above the arrow: how text and braille write it.
    pub fn as_row(&self, cells: &[Vec<Node>]) -> Node {
        let cell = |(r, c): (usize, usize)| cells.get(r).and_then(|row| row.get(c)).cloned().unwrap_or_else(|| Node::row(vec![]));
        let symbol = match (self.line, self.tail) {
            (ArrowLine::Double, ArrowTail::Head) if self.head == ArrowHead::None => '⇐',
            (ArrowLine::Double, ArrowTail::Head) => '⇔',
            (ArrowLine::Double, _) if self.head == ArrowHead::None => '=',
            (ArrowLine::Double, _) => '⇒',
            (_, ArrowTail::Head) if self.head == ArrowHead::None => '←',
            (_, ArrowTail::Head) => '↔',
            (_, ArrowTail::Bar) => '↦',
            (_, ArrowTail::Hook | ArrowTail::HookRight) => '↪',
            (_, ArrowTail::Tail) => '↣',
            _ if self.head == ArrowHead::TwoHeads => '↠',
            _ if self.line == ArrowLine::Squiggly => '⇝',
            _ if self.head == ArrowHead::None => '—',
            _ => '→',
        };
        // a label that draws nothing (xymatrix's \hole) is not written
        let invisible = |n: &Node| match n {
            Node::Phantom { .. } => true,
            Node::Row { children } => children.iter().all(|c| matches!(c, Node::Phantom { .. })),
            _ => false,
        };
        let above = self.labels.iter().find(|l| !invisible(&l.node)).map(|l| Box::new(l.node.clone()));
        Node::row(vec![cell(self.from), Node::Arrow { symbol, above, below: None }, cell(self.to)])
    }
}

fn is_zero_i16(v: &i16) -> bool {
    *v == 0
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArrowLabel {
    pub node: Node,
    #[serde(skip_serializing_if = "LabelSide::is_default")]
    pub side: LabelSide,
    /// Where along the arrow, from 0 (start) to 1 (end): tikz-cd's `near start`, xymatrix's `^<`.
    #[serde(skip_serializing_if = "is_middle")]
    pub position: f32,
    /// On an arrow with waypoints (`via`), the straight segment it is on, from 0; `position` is along it.
    #[serde(skip_serializing_if = "is_zero_usize")]
    pub segment: usize,
}

fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

fn is_middle(v: &f32) -> bool {
    *v == 0.5
}

/// Where a label sits, looking along the arrow: tikz-cd's `"f"` and `"f"'`, xymatrix's `^f` and `_f`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelSide {
    #[default]
    Left,
    Right,
    /// On the line, which is broken around it (`description`, `|f`).
    On,
}

impl LabelSide {
    pub fn is_default(&self) -> bool {
        *self == LabelSide::Left
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowLine {
    #[default]
    Solid,
    Dashed,
    Dotted,
    /// Two parallel lines: `Rightarrow`, `equal`, `@{=>}`.
    Double,
    Squiggly,
    /// Nothing drawn, only the labels (`phantom`, `@{}`).
    None,
}

impl ArrowLine {
    pub fn is_default(&self) -> bool {
        *self == ArrowLine::Solid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowHead {
    #[default]
    Normal,
    TwoHeads,
    None,
}

impl ArrowHead {
    pub fn is_default(&self) -> bool {
        *self == ArrowHead::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowTail {
    #[default]
    None,
    /// `hookrightarrow`: the hook curls to the left of the direction; `HookRight` to the right (`hook'`).
    Hook,
    HookRight,
    /// `mapsto`: a bar across the start.
    Bar,
    /// A second head at the start, pointing back: `leftrightarrow`.
    Head,
    /// `rightarrowtail`: a reversed head at the start.
    Tail,
}

impl ArrowTail {
    pub fn is_default(&self) -> bool {
        *self == ArrowTail::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TableKind {
    Matrix,
    PMatrix,
    BMatrix,
    BraceMatrix,
    VMatrix,
    VVMatrix,
    SmallMatrix,
    Cases,
    /// mathtools `rcases`: the brace on the right.
    RCases,
    Aligned,
    Centered,
    Array,
    /// `\substack`, `subarray`: cramped rows, in the caller's style.
    Substack,
    /// amscd commutative diagram (`CD`).
    Diagram,
    /// `\bordermatrix`: the first row and first column are labels outside the parentheses.
    BorderMatrix,
}

impl Node {
    /// The ⋮ of `\vdotswithin`: what speech, text and braille read for it.
    pub const VDOTS: Node = Node::Operator { symbol: '⋮', class: Class::Ord };

    pub fn children(&self) -> &[Node] {
        match self {
            Node::Row { children } => children,
            _ => &[],
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("the tree always serializes")
    }

    pub fn row(children: Vec<Node>) -> Node {
        Node::Row { children }
    }

    /// The node inside one-child rows, borrowed: what `simplify` would return.
    pub fn single(&self) -> &Node {
        match self {
            Node::Row { children } if children.len() == 1 => children[0].single(),
            other => other,
        }
    }

    /// The function or large operator a `\mathop{…}` holds, with phantoms beside it at most (as
    /// mathtools' `\adjustlimits` writes it): what speech, text, braille and MathML take the limits
    /// of. The node itself otherwise.
    pub fn operator_core(&self) -> &Node {
        let Node::ForcedClass { class: Class::Op, base } = self else { return self };
        let mut kept = base.single();
        if let Node::Row { children } = kept {
            let mut operators = children.iter().filter(|c| !matches!(c, Node::Phantom { .. }));
            match (operators.next(), operators.next()) {
                (Some(only), None) => kept = only,
                _ => return self,
            }
        }
        match kept {
            Node::Function { .. } | Node::LargeOperator { .. } => kept,
            _ => self,
        }
    }

    /// A single node, without the surrounding Row when it has only one child.
    pub fn simplify(self) -> Node {
        match self {
            // recursive: {{2}} coming from a macro is also just the 2
            Node::Row { mut children } if children.len() == 1 => children.pop().unwrap().simplify(),
            other => other,
        }
    }
}

/// #207: a symbol drawn by composition (#176) is kept in the tree as a private marker character
/// (`crate::composite`), which means nothing outside contextEquation; the JSON of the tree (`--tree`,
/// [`Node::to_json`], the SVG's metadata) names it by its command instead: `"symbol":"\\llceil"`.
/// Every other symbol is its character.
mod composite_json {
    use serde::{Serialize, Serializer};

    struct Symbol(char);

    impl Serialize for Symbol {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            match crate::composite::get(self.0) {
                Some(k) => s.collect_str(&format_args!("\\{}", k.command)),
                None => s.serialize_char(self.0),
            }
        }
    }

    pub fn symbol<S: Serializer>(c: &char, s: S) -> Result<S::Ok, S::Error> {
        Symbol(*c).serialize(s)
    }

    pub fn optional<S: Serializer>(c: &Option<char>, s: S) -> Result<S::Ok, S::Error> {
        c.map(Symbol).serialize(s)
    }

    pub fn list<S: Serializer>(c: &[char], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(c.iter().map(|c| Symbol(*c)))
    }
}
