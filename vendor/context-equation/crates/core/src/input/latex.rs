//! Reader for math LaTeX: the commands and environments listed in the README, plus contextEquation's extensions
//! (`\divchave`, `\contaarmada`).

use crate::font::MathFont;
use crate::locale::Locale;
use crate::rules::{self, Registry};
use crate::tree::*;
use crate::Error;

use super::symbols::{self, Symbol};
use siunitx::{joined_units, unit_node, PerMode, UnitEntry, UnitSymbol};

#[cfg(feature = "chem")]
mod chemistry;
mod color;
mod diagram;
mod mathtools;
mod siunitx;
mod tex;

pub fn read(tex: &str, locale: Locale, registry: &mut Registry) -> Result<Node, Error> {
    read_with(tex, locale, registry, true)
}

/// `strict`: refuse invalid LaTeX like TeX does. Without it, recover what is possible and record warnings.
pub fn read_with(tex: &str, locale: Locale, registry: &mut Registry, strict: bool) -> Result<Node, Error> {
    read_in_font(tex, locale, registry, strict, MathFont::default())
}

/// [`read_with`], for an equation set in `font`: the words it writes in the language (a range's
/// "to", a list's "and") are kept only if that font has their letters.
pub fn read_in_font(tex: &str, locale: Locale, registry: &mut Registry, strict: bool, font: MathFont) -> Result<Node, Error> {
    read_in_document(tex, locale, registry, strict, font, &Labels::new(), &Labels::new()).map(|(body, _)| body)
}

/// Keys and what they stand for: the number of a label (`"eq:a"` → `"3"`) or of a citation.
pub type Labels = std::collections::BTreeMap<String, String>;

/// [`read_in_font`], for an equation of a document: `\ref` and `\eqref` show the number `labels` gives
/// their key, `\cite` the number `citations` gives it. Also returns the labels the equation numbers
/// itself, a `\label` with a `\tag` on the same line, for the equations that come after it.
pub fn read_in_document(tex: &str, locale: Locale, registry: &mut Registry, strict: bool, font: MathFont, labels: &Labels, citations: &Labels) -> Result<(Node, Labels), Error> {
    let clean = prepare(tex, registry, strict);
    let c: Vec<char> = clean.chars().collect();
    let mut macros = Macros { max_size: c.len() + MAX_GROWTH, json_text: json_signs(tex, &c), ..Macros::default() };
    let mut refs = References { given: labels.clone(), citations: citations.clone(), ..References::default() };
    let mut reader = Reader { c, pos: 0, locale, registry, macros: &mut macros, refs: &mut refs, variant: Variant::Italic, strict, font };
    let body = reader.top()?;
    refs.end_line();
    Ok((body, refs.defined))
}

/// The cross-references of the equation (`\label`, `\tag`, `\ref`, `\eqref`, `\cite`).
#[derive(Default)]
struct References {
    /// The numbers of labels the caller knows (the document around the equation).
    given: Labels,
    citations: Labels,
    /// Labels this equation numbers with `\tag`.
    defined: Labels,
    /// The `\label`s and the `\tag` of the line being read: they pair when the line ends.
    line_labels: Vec<String>,
    line_tag: Option<String>,
}

impl References {
    /// The end of a numbered line (`\\` in `align`, the end of `equation` or of the input): a `\tag`
    /// numbers the `\label`s of its line. A `\label` without a `\tag` would take the automatic number,
    /// which contextEquation does not give: it stays unknown.
    fn end_line(&mut self) {
        let labels = std::mem::take(&mut self.line_labels);
        if let Some(tag) = self.line_tag.take() {
            for label in labels {
                self.defined.insert(label, tag.clone());
            }
        }
    }

    /// The number of a label: numbered by this equation (on a line already read, or on this line, when
    /// its `\label` and `\tag` came before), or given by the caller.
    fn number(&self, key: &str) -> Option<&String> {
        let this_line = self.line_tag.as_ref().filter(|_| self.line_labels.iter().any(|l| l == key));
        this_line.or_else(|| self.defined.get(key)).or_else(|| self.given.get(key))
    }
}

/// The environments that number their lines (and so take `\tag` and `\label` per line).
fn numbered_environment(name: &str) -> bool {
    matches!(name.trim_end_matches('*'), "equation" | "align" | "gather" | "multline" | "flalign" | "alignat" | "eqnarray")
}

/// Substitution macros defined in the equation itself (`\newcommand`, `\def`, `\let`…).
/// There are no conditionals and no `\catcode`: contextEquation expands macros, it does not run TeX programs.
#[derive(Default)]
struct Macros {
    commands: std::collections::HashMap<String, Macro>,
    environments: std::collections::HashMap<String, (String, String)>,
    /// Colors defined by `\definecolor` and `\colorlet`, for the rest of the input.
    colors: color::Colors,
    /// Units declared by `\DeclareSIUnit` and options set by `\sisetup`, for the rest of the input.
    siunitx: siunitx::State,
    expansions: usize,
    /// How much the text may grow through expansions, counting the original size.
    max_size: usize,
    /// The too-many-expansions warning was already given (there is one, not one per discarded expansion).
    warned_excess: bool,
    /// Groups, commands and scripts open right now: protects the stack from inputs like `{{{{…`.
    depth: usize,
    /// mathtools' `\shortvdotswithin` wrote the `\vdotswithin` that comes next: that one is short.
    short_row: bool,
    /// E-REC-2, [`json_signs`]: `None` when the input has a real tab or line break (a JSON string has
    /// none, so it shows no escape); otherwise, does the input show a sign of JSON ([`json_evidence`]).
    json_text: Option<bool>,
    /// `\setlength{\arraycolsep}{…}`, in em, for the arrays and matrices after it.
    array_colsep: Option<f32>,
    /// chemfig's `\definesubmol`s, for the structures after them.
    #[cfg(feature = "chem")]
    submols: crate::chem::model::Submols,
}

#[derive(Clone)]
struct Macro {
    arguments: usize,
    default_arg: Option<String>,
    body: String,
    /// `\DeclarePairedDelimiter` and mathtools' X and XPP forms: the delimiters around the body.
    paired: Option<Box<Paired>>,
}

/// The delimiters of a mathtools paired delimiter, and what `\DeclarePairedDelimiterXPP` puts before
/// and after them.
#[derive(Clone)]
struct Paired {
    open: String,
    close: String,
    pre: String,
    post: String,
}

/// More than this is almost certainly a macro that calls itself.
const EXPANSION_LIMIT: usize = 2000;
/// Characters the expansions may add to the original text.
const MAX_GROWTH: usize = 200_000;
/// Group, command and script levels open at the same time. Each level costs ~10 KB of stack unoptimized, so 64 fit in 1 MB.
pub const NESTING_LIMIT: usize = 64;
/// Columns of an `array`, even with a repeated `*{n}{…}`.
const MAX_COLUMNS: usize = 1000;

/// Bold markers inside the text of `\text{…}` (Unicode private use area).
pub const BOLD_START: char = '\u{E000}';
pub const BOLD_END: char = '\u{E001}';
/// Small capitals markers (`\textsc{…}`).
pub const SMALL_CAPS_START: char = '\u{E002}';
pub const SMALL_CAPS_END: char = '\u{E003}';
/// Typewriter markers (`\texttt{…}`, `\url{…}`).
pub const MONO_START: char = '\u{E004}';
pub const MONO_END: char = '\u{E005}';
/// Italic markers (`\itshape`); `\textit{…}` and `\emph{…}` close with the second end, after which
/// LaTeX adds the italic correction of the last letter (none before a period or a comma).
pub const ITALIC_START: char = '\u{E006}';
pub const ITALIC_END: char = '\u{E007}';
pub const ITALIC_CORRECTED_END: char = '\u{E008}';
/// Every marker, for the outputs that drop them.
pub const MARKERS: [char; 9] = [BOLD_START, BOLD_END, SMALL_CAPS_START, SMALL_CAPS_END, MONO_START, MONO_END, ITALIC_START, ITALIC_END, ITALIC_CORRECTED_END];

/// L-ENT-1 and L-ENT-2: HTML entities and outer delimiters that arrive with the equation.
fn prepare(tex: &str, registry: &mut Registry, strict: bool) -> String {
    // the repair comes before the trim: a \frac eaten by JSON starts with a form feed, which trim would delete
    let mut s = tex.to_string();
    // an unescaped % starts a comment in TeX; pasted equations usually mean a percent sign (15%)
    let (uncommented, percents) = comments(&s);
    if strict {
        s = uncommented;
    } else {
        for column in percents {
            registry.warn(rules::E_REC_9, format!("column {column}: '%' starts a comment in TeX (write \\% for a percent sign); drawn as a percent sign"));
        }
    }
    control_spaces(&mut s);
    if !strict {
        if let Some(repaired) = repair_controls(&s) {
            registry.warn(rules::E_REC_2, "JSON control character in place of the backslash (\\frac stored unescaped): command rebuilt");
            s = repaired;
        }
    }
    s = trim_keeping_control_space(&s);
    if !strict {
        if let Some(undone) = undo_doubled_backslashes(&s) {
            registry.warn(rules::E_REC_2, "doubled backslashes (LaTeX escaped twice): one level undone");
            s = undone;
        }
        // a lone backslash at the end is left over from cut text
        let trailing_backslashes = s.chars().rev().take_while(|c| *c == '\\').count();
        if trailing_backslashes % 2 == 1 {
            registry.warn(rules::E_REC_9, "lone '\\' at the end: ignored");
            s.pop();
        }
    }
    for (open, close) in [("$$", "$$"), ("\\[", "\\]"), ("\\(", "\\)"), ("$", "$")] {
        if s.len() >= open.len() + close.len() && s.starts_with(open) && s.ends_with(close) {
            s = s[open.len()..s.len() - close.len()].trim().to_string();
            registry.note(rules::L_ENT_2, format!("delimitadores externos {open}…{close} removidos"));
            break;
        }
    }
    s = s.replace("\\begingroup", "{").replace("\\endgroup", "}");
    let entities = [("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&"), ("&nbsp;", "~"), ("&quot;", "\""), ("&#39;", "'")];
    for (ent, swap) in entities {
        if s.contains(ent) {
            s = s.replace(ent, swap);
            registry.note(rules::L_ENT_1, format!("HTML entity {ent} read as '{swap}'"));
        }
    }
    s
}

#[derive(Debug, Clone, PartialEq)]
enum Stop {
    Eof,
    CloseBrace,
    E,
    NewRow,
    End(String),
    ContextEnd(String),
    Right(Option<char>),
    Middle(Option<char>),
    /// `\hline`, or `\hdashline` when dashed.
    Hline(bool),
    /// `\cline{a-b}`, or `\cdashline{a-b}` when dashed.
    Cline(usize, usize, bool),
    /// `\intertext{…}` inside an alignment.
    Intertext(Node),
}

struct Reader<'a> {
    c: Vec<char>,
    pos: usize,
    locale: Locale,
    registry: &'a mut Registry,
    macros: &'a mut Macros,
    refs: &'a mut References,
    /// Current variant, changed by `\bf`, `\rm`, `\mathrm{…}`.
    variant: Variant,
    strict: bool,
    /// The font the equation will be set in.
    font: MathFont,
}

type R<T> = Result<T, Error>;

/// Neighbor letters of one variant as one identifier: a word, as a text font sets it.
fn words(node: Node) -> Node {
    let Node::Row { children } = node else { return node };
    let mut merged: Vec<Node> = Vec::with_capacity(children.len());
    for child in children {
        if let (Some(Node::Identifier { name, variant }), Node::Identifier { name: next, variant: v }) = (merged.last_mut(), &child) {
            if variant == v && name.chars().chain(next.chars()).all(char::is_alphabetic) {
                name.push_str(next);
                continue;
            }
        }
        merged.push(child);
    }
    Node::row(merged).simplify()
}

impl Reader<'_> {
    // ---------- utilities ----------

    fn current(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }

    fn em(&self, d: usize) -> Option<char> {
        self.c.get(self.pos + d).copied()
    }

    fn error<T>(&self, column: usize, message: impl Into<String>) -> R<T> {
        Err(Error::Syntax { column, message: message.into() })
    }

    /// In strict mode, an error; in tolerant mode, a warning, and the caller's recovery goes on.
    fn tolerate(&mut self, rule: &'static str, column: usize, problem: impl Into<String>, recovery: &str) -> R<()> {
        let problem = problem.into();
        if self.strict {
            return Err(Error::Syntax { column, message: problem });
        }
        self.registry.warn(rule, format!("column {column}: {problem}; {recovery}"));
        Ok(())
    }

    fn column(&self) -> usize {
        self.pos + 1
    }

    /// What `\ref{key}` (`name` "ref") or `\eqref{key}` shows: the label's number, `(3)` for `\eqref`.
    /// A label with no number is `??`, as on LaTeX's first run: a warning, or an error in strict mode.
    fn label_number(&mut self, name: &str, key: &str, col: usize) -> R<String> {
        let key = key.trim();
        let number = match self.refs.number(key).cloned() {
            Some(n) => n,
            None => {
                let shown = if name == "eqref" { "(??)" } else { "??" };
                self.tolerate(
                    rules::E_REC_9,
                    col,
                    format!("\\{name}{{{key}}}: no number for this label (a \\tag on its line, or the labels option)"),
                    &format!("drawn as {shown}"),
                )?;
                "??".to_string()
            }
        };
        Ok(if name == "eqref" { format!("({number})") } else { number })
    }

    /// What `\cite[note]{a,b}` shows: `[1, 2, note]`, each key by its number in the citations; a key with
    /// no number is `?`, as on LaTeX's first run: a warning, or an error in strict mode.
    fn citation(&mut self, keys: &str, note: Option<&str>, col: usize) -> R<String> {
        let mut shown = Vec::new();
        for key in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            match self.refs.citations.get(key).cloned() {
                Some(n) => shown.push(n),
                None => {
                    self.tolerate(rules::E_REC_9, col, format!("\\cite{{{key}}}: no number for this citation (the citations option)"), "drawn as ?")?;
                    shown.push("?".to_string());
                }
            }
        }
        if let Some(note) = note.map(|n| n.replace('~', " ")).filter(|n| !n.trim().is_empty()) {
            shown.push(note.trim().to_string());
        }
        Ok(format!("[{}]", shown.join(", ")))
    }

    /// Runs `f` one level deeper. Going past the limit is an error in both modes: there is nothing to recover.
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        if self.macros.depth >= NESTING_LIMIT {
            return self.error(self.column(), format!("nesting deeper than {NESTING_LIMIT} levels"));
        }
        self.macros.depth += 1;
        let r = f(self);
        self.macros.depth -= 1;
        r
    }

    fn skip_spaces(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn read_command_name(&mut self) -> String {
        // we are already past the backslash
        let mut name = String::new();
        while let Some(c) = self.current().filter(|c| c.is_ascii_alphabetic()) {
            name.push(c);
            self.pos += 1;
        }
        if name.is_empty() {
            if let Some(c) = self.current() {
                name.push(c);
                self.pos += 1;
            }
        }
        name
    }

    /// Raw content between braces, honoring inner braces. Accepts a single character without braces.
    fn raw_text(&mut self) -> R<String> {
        self.skip_spaces();
        let col = self.column();
        match self.current() {
            Some('{') => {
                self.pos += 1;
                let mut level = 1;
                let mut s = String::new();
                while let Some(c) = self.current() {
                    self.pos += 1;
                    match c {
                        '\\' => {
                            s.push(c);
                            if let Some(d) = self.current() {
                                s.push(d);
                                self.pos += 1;
                            }
                            continue;
                        }
                        '{' => level += 1,
                        '}' => {
                            level -= 1;
                            if level == 0 {
                                return Ok(s);
                            }
                        }
                        _ => {}
                    }
                    s.push(c);
                }
                self.tolerate(rules::E_REC_4, col, "'{' without a matching '}'", "closed at the end")?;
                Ok(s)
            }
            // an unbraced command is a whole argument, as in TeX: \boldsymbol\alpha, \mathbb\R
            Some('\\') => {
                let start = self.pos;
                self.pos += 1;
                self.read_command_name();
                Ok(self.c[start..self.pos].iter().collect())
            }
            Some(c) => {
                self.pos += 1;
                Ok(c.to_string())
            }
            None => {
                self.tolerate(rules::E_REC_7, col, "missing argument", "left empty")?;
                Ok(String::new())
            }
        }
    }

    fn raw_optional(&mut self) -> Option<String> {
        let saved = self.pos;
        self.skip_spaces();
        if self.current() != Some('[') {
            self.pos = saved;
            return None;
        }
        self.pos += 1;
        let mut level = 0;
        let mut s = String::new();
        while let Some(c) = self.current() {
            self.pos += 1;
            match c {
                '{' => level += 1,
                '}' => level -= 1,
                ']' if level == 0 => return Some(s),
                _ => {}
            }
            s.push(c);
        }
        self.pos = saved;
        None
    }

    /// Reads a raw snippet as math, with the current variant.
    fn sub_reader(&mut self, tex: &str, variant: Variant) -> R<Node> {
        let mut sub = Reader {
            c: tex.chars().collect(),
            pos: 0,
            locale: self.locale,
            registry: self.registry,
            macros: self.macros,
            refs: self.refs,
            variant,
            strict: self.strict,
            font: self.font,
        };
        let (items, stop) = sub.list()?;
        match stop {
            Stop::Eof => Ok(Node::row(items).simplify()),
            other => {
                let col = sub.column();
                sub.tolerate(rules::E_REC_5, col, format!("unexpected closing: {other:?}"), "ignored")?;
                Ok(Node::row(items).simplify())
            }
        }
    }

    // ---------- structure ----------

    /// Whole body. Loose `\\` and `&` become rows and columns of an alignment (a loose `&` is misplaced in TeX: E-REC-9).
    fn top(&mut self) -> R<Node> {
        let mut rows: Vec<Vec<Node>> = Vec::new();
        let mut cells: Vec<Node> = Vec::new();
        let mut had_e = false;
        loop {
            let (items, stop) = self.list()?;
            cells.push(Node::row(items).simplify());
            match stop {
                Stop::Eof => break,
                Stop::E => {
                    // TeX: "Misplaced alignment tab character &"
                    self.tolerate(rules::E_REC_9, self.pos, "'&' outside an alignment", "read as a column of an alignment")?;
                    had_e = true;
                }
                Stop::NewRow => {
                    rows.push(std::mem::take(&mut cells));
                    self.refs.end_line();
                }
                Stop::CloseBrace => self.tolerate(rules::E_REC_4, self.pos, "'}' without a matching '{'", "ignored")?,
                Stop::End(n) => self.tolerate(rules::E_REC_5, self.pos, format!("\\end{{{n}}} without a matching \\begin"), "ignored")?,
                Stop::ContextEnd(n) => self.tolerate(rules::E_REC_5, self.pos, format!("\\stop{n} without a matching \\start"), "ignored")?,
                Stop::Right(d) => {
                    self.tolerate(rules::E_REC_5, self.pos, "\\right without a matching \\left", "delimiter drawn alone")?;
                    if let Some(d) = d {
                        cells.push(Node::Operator { symbol: d, class: Class::Close });
                    }
                }
                Stop::Middle(_) => self.tolerate(rules::E_REC_5, self.pos, "\\middle outside \\left…\\right", "ignored")?,
                Stop::Hline(_) | Stop::Cline(..) => {}
                Stop::Intertext(t) => cells.push(t),
            }
        }
        if rows.is_empty() && !had_e {
            return Ok(Node::row(cells.into_iter().flat_map(undo_line).collect()));
        }
        if !cells.iter().all(empty) || rows.is_empty() {
            rows.push(cells);
        }
        let kind = if had_e { TableKind::Aligned } else { TableKind::Centered };
        Ok(Node::Table {
            shape: kind,
            rows,
            columns: vec![],
            verticals: vec![],
            dashed: vec![],
            horizontals: vec![],
            between: vec![],
            intertexts: vec![],
            stretch: 1.0,
            colsep: 500,
        })
    }

    /// Reads atoms up to a stop (end, `}`, `&`, `\\`, `\end`, `\right`…).
    fn list(&mut self) -> R<(Vec<Node>, Stop)> {
        self.nested(Self::list_unguarded)
    }

    fn list_unguarded(&mut self) -> R<(Vec<Node>, Stop)> {
        let mut items: Vec<Node> = Vec::new();
        loop {
            self.skip_spaces();
            let Some(c) = self.current() else {
                return Ok((items, Stop::Eof));
            };
            match c {
                '}' => {
                    self.pos += 1;
                    return Ok((items, Stop::CloseBrace));
                }
                '&' => {
                    self.pos += 1;
                    return Ok((items, Stop::E));
                }
                '^' | '_' | '\'' => self.scripts(&mut items)?,
                '\\' => {
                    if let Some(p) = self.structural_command(&mut items)? {
                        return Ok((items, p));
                    }
                }
                _ => {
                    let node = self.atom()?;
                    items.extend(node);
                }
            }
        }
    }
}

fn empty(n: &Node) -> bool {
    matches!(n, Node::Row { children } if children.is_empty())
}

fn undo_line(n: Node) -> Vec<Node> {
    match n {
        Node::Row { children } => children,
        other => vec![other],
    }
}

impl Reader<'_> {
    // ---------- atoms ----------

    /// One atom, without scripts. May return several nodes (a number with a thin space, for example).
    fn atom(&mut self) -> R<Vec<Node>> {
        let col = self.column();
        let c = self.current().expect("called with a character");
        Ok(match c {
            '{' => vec![self.group()?],
            '0'..='9' | '.' if c != '.' || self.em(1).is_some_and(|d| d.is_ascii_digit()) => {
                vec![self.number()]
            }
            '~' => {
                self.pos += 1;
                vec![Node::Space { em: 0.333 }]
            }
            '$' => {
                // a stray dollar sign in the middle of the equation: a leftover delimiter (TeX would leave math mode)
                self.tolerate(rules::E_REC_9, col, "'$' in the middle of the equation", "ignored")?;
                self.pos += 1;
                vec![]
            }
            c if c.is_control() => {
                self.tolerate(rules::E_REC_9, col, format!("control character U+{:04X}", c as u32), "ignored")?;
                self.pos += 1;
                vec![]
            }
            '#' => {
                self.tolerate(rules::E_REC_9, col, "'#' outside a macro definition", "drawn as a symbol")?;
                self.pos += 1;
                vec![Node::Operator { symbol: '#', class: Class::Ord }]
            }
            c if c.is_alphabetic() => {
                self.pos += 1;
                vec![self.letter(c)]
            }
            c => {
                self.pos += 1;
                match symbols::character(c) {
                    Some(Symbol { symbol, class }) => vec![Node::Operator { symbol, class }],
                    None => vec![Node::Operator { symbol: c, class: Class::Ord }],
                }
            }
        })
    }

    fn letter(&self, c: char) -> Node {
        let variant = if c.is_ascii_alphabetic() || ('α'..='ω').contains(&c) || c == 'ı' || c == 'ȷ' { self.variant } else { Variant::Upright };
        Node::Identifier { name: c.to_string(), variant }
    }

    fn group(&mut self) -> R<Node> {
        let col = self.column();
        self.pos += 1;
        let variant = self.variant;
        let (items, stop) = self.list()?;
        self.variant = variant;
        match stop {
            Stop::CloseBrace => {}
            Stop::Eof => self.tolerate(rules::E_REC_4, col, "'{' without a matching '}'", "closed at the end")?,
            other => {
                self.tolerate(rules::E_REC_4, col, format!("group opened with '{{' ended in {other:?}"), "closed there")?;
            }
        }
        // as in TeX, an operator alone in braces becomes an ordinary atom: {,} gets no space
        Ok(match items.as_slice() {
            [Node::Operator { symbol, .. }] => Node::Operator { symbol: *symbol, class: Class::Ord },
            _ => Node::row(items),
        })
    }

    #[inline(never)]
    fn number(&mut self) -> Node {
        // \mathbb{1}, \mathds{1}, \mathbbm{1}: the double-struck digit 𝟙 (U+1D7D9), a symbol, not a number
        if self.variant == Variant::DoubleStruck {
            if let Some(digit) = self.current().filter(char::is_ascii_digit) {
                self.pos += 1;
                return Node::Identifier { name: digit.to_string(), variant: Variant::DoubleStruck };
            }
        }
        if let Some(node) = self.number_with_thousands() {
            return node;
        }
        let start = self.pos;
        let mut value = String::new();
        while let Some(c) = self.current() {
            let decimal = (c == '.' || (c == ',' && self.locale.comma_is_decimal())) && self.em(1).is_some_and(|d| d.is_ascii_digit()) && !value.contains('.');
            if c.is_ascii_digit() {
                value.push(c);
            } else if decimal {
                value.push('.');
            } else {
                break;
            }
            self.pos += 1;
        }
        let written: String = self.c[start..self.pos].iter().collect();
        if written.contains(',') {
            self.registry.note(rules::L_NUM_1, format!("'{written}': comma between digits read as decimal separator ({value})"));
        }
        Node::Number { value, group: false, thousands: None, variant: self.variant.of_digits() }
    }

    /// L-NUM-3: `1.000` and `12.345,67` in pt-BR, `1,000` and `12,345.67` in English: the locale's thousands
    /// separator followed by groups of exactly three digits separates thousands and is kept in the drawing.
    fn number_with_thousands(&mut self) -> Option<Node> {
        let thousands = self.locale.thousands_separator()?;
        let decimal = self.locale.decimal_separator();
        let mut i = self.pos;
        let digits = |i: &mut usize| -> String {
            let mut s = String::new();
            while let Some(c) = self.c.get(*i).filter(|c| c.is_ascii_digit()) {
                s.push(*c);
                *i += 1;
            }
            s
        };
        let first = digits(&mut i);
        if first.is_empty() || first.len() > 3 {
            return None;
        }
        let mut groups = vec![first];
        while self.c.get(i) == Some(&thousands) && self.c.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            let mut j = i + 1;
            let g = digits(&mut j);
            if g.len() != 3 {
                break;
            }
            groups.push(g);
            i = j;
        }
        if groups.len() < 2 {
            return None;
        }
        // after the last group no other digit may follow the thousands separator directly
        if self.c.get(i) == Some(&thousands) && self.c.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut value: String = groups.concat();
        if self.c.get(i) == Some(&decimal) && self.c.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            let mut j = i + 1;
            let f = digits(&mut j);
            value.push('.');
            value.push_str(&f);
            i = j;
        }
        let written: String = self.c[self.pos..i].iter().collect();
        self.pos = i;
        self.registry.note(rules::L_NUM_3, format!("'{written}': '{thousands}' separa milhares ({value})"));
        Some(Node::Number { value, group: false, thousands: Some(thousands), variant: self.variant.of_digits() })
    }

    /// Argument of a command or script: a group or a single token.
    fn argument(&mut self) -> R<Node> {
        self.skip_spaces();
        let col = self.column();
        match self.current() {
            None => {
                self.tolerate(rules::E_REC_7, col, "missing argument", "left empty")?;
                Ok(Node::row(vec![]))
            }
            Some('{') => Ok(self.group()?.simplify()),
            Some('}') => {
                self.tolerate(rules::E_REC_7, col, "missing argument", "left empty")?;
                Ok(Node::row(vec![]))
            }
            Some('\\') => {
                let mut items = Vec::new();
                if let Some(p) = self.structural_command(&mut items)? {
                    self.tolerate(rules::E_REC_7, col, format!("invalid argument: {p:?}"), "ignored")?;
                }
                Ok(Node::row(items).simplify())
            }
            Some(c) if c.is_ascii_digit() => {
                self.pos += 1;
                Ok(Node::Number { value: c.to_string(), group: false, thousands: None, variant: self.variant.of_digits() })
            }
            Some(_) => Ok(Node::row(self.atom()?).simplify()),
        }
    }

    /// Base of `\overset` and `\underset`: `{=}` stays a relation (the plain group would make it Ord).
    fn base_with_class(&mut self) -> R<Node> {
        self.skip_spaces();
        if self.current() == Some('{') {
            let raw = self.raw_text()?;
            return self.sub_reader(&raw, self.variant);
        }
        self.argument()
    }

    fn optional_argument(&mut self) -> R<Option<Node>> {
        match self.raw_optional() {
            Some(s) => Ok(Some(self.sub_reader(&s, self.variant)?)),
            None => Ok(None),
        }
    }

    /// `^`, `_` and `'` attach to the last atom.
    fn scripts(&mut self, items: &mut Vec<Node>) -> R<()> {
        self.nested(|this| this.scripts_unguarded(items))
    }

    /// The argument of the script `mark` (`^` or `_`) TeX refuses, before it is read:
    /// - `y__k`, `x^_2`: the argument is itself `^` or `_`. TeX stops ("Missing { inserted"); tolerant
    ///   mode draws the mark as the script, as before, with a warning (E-REC-3).
    /// - `^²`, `_₂`: a Unicode superscript or subscript, pasted from an editor, would be shrunk and
    ///   raised twice; tolerant mode reads it as its plain character (`^2`, `_2`; a run of them as a
    ///   group, `^⁻¹` as `^{-1}`), with a warning (E-REC-2).
    fn check_script_argument(&mut self, mark: char) -> R<()> {
        let saved = self.pos;
        self.skip_spaces();
        let col = self.column();
        let found = self.current();
        let at = self.pos;
        self.pos = saved;
        let which = if mark == '^' { "superscript" } else { "subscript" };
        match found {
            Some(second @ ('^' | '_')) => {
                self.tolerate(rules::E_REC_3, col, format!("'{second}' right after '{mark}': the {which} has no argument"), &format!("'{second}' drawn as the {which}"))?;
            }
            Some(glyph) if plain_script_char(glyph).is_some() => {
                // the whole run of the same kind is the script: ^⁻¹ is ^{-1}
                let sub = is_subscript_char(glyph);
                let run = self.c[at..].iter().take_while(|&&g| plain_script_char(g).is_some() && is_subscript_char(g) == sub).count();
                let glyphs: String = self.c[at..at + run].iter().collect();
                let plain: String = glyphs.chars().filter_map(plain_script_char).collect();
                let argument = if run == 1 { plain } else { format!("{{{plain}}}") };
                self.tolerate(
                    rules::E_REC_2,
                    col,
                    format!("'{glyphs}' after '{mark}' is already a {}", if sub { "subscript" } else { "superscript" }),
                    &format!("read as '{mark}{argument}'"),
                )?;
                self.c.splice(at..at + run, argument.chars());
            }
            Some(_) => {}
            None => {}
        }
        Ok(())
    }

    fn scripts_unguarded(&mut self, items: &mut Vec<Node>) -> R<()> {
        // x_1_2_3…: each double script starts over on an empty base, in a loop (not by recursion)
        'index: loop {
            let previous = items.pop().unwrap_or(Node::row(vec![]));
            // the brace of \overbrace{…}^{label} takes the script as its label
            if let Node::Brace { above, label: None, .. } = &previous {
                let mark = if *above { '^' } else { '_' };
                if self.current() == Some(mark) {
                    self.pos += 1;
                    let label = self.argument()?;
                    if let Node::Brace { base, above, bracket, .. } = previous {
                        items.push(Node::Brace { base, above, label: Some(Box::new(label)), bracket });
                    }
                    return Ok(());
                }
            }
            let (base, mut sup, mut sub) = match previous {
                Node::Scripts { base, sup, sub } => (*base, sup.map(|b| *b), sub.map(|b| *b)),
                other => (other, None, None),
            };
            let only_primes = |s: &Option<Node>| matches!(s, Some(Node::Operator { symbol: '′' | '″' | '‴', .. }));
            loop {
                self.skip_spaces();
                match self.current() {
                    Some('\'') => {
                        let mut n = 0;
                        while self.current() == Some('\'') {
                            n += 1;
                            self.pos += 1;
                        }
                        let prime = Node::Operator { symbol: ['′', '″', '‴'][n.min(3) - 1], class: Class::Ord };
                        sup = Some(match sup {
                            None => prime,
                            Some(s) => Node::row(std::iter::once(prime).chain(undo_line(s)).collect()),
                        });
                    }
                    Some('^') if sup.is_none() || only_primes(&sup) => {
                        self.pos += 1;
                        self.check_script_argument('^')?;
                        let arg = self.argument()?;
                        sup = Some(match sup {
                            None => arg,
                            Some(prime) => Node::row(vec![prime, arg]),
                        });
                    }
                    Some('_') if sub.is_none() => {
                        self.pos += 1;
                        self.check_script_argument('_')?;
                        sub = Some(self.argument()?);
                    }
                    Some(c @ ('^' | '_')) => {
                        let col = self.column();
                        let which = if c == '^' { "superscript" } else { "subscript" };
                        self.tolerate(rules::E_REC_3, col, format!("double {which}: use braces, as in x{c}{{a{c}b}}"), "the second went on an empty base")?;
                        // close the current script and start over with an empty base
                        items.push(Node::Scripts { base: Box::new(base), sup: sup.take().map(Box::new), sub: sub.take().map(Box::new) });
                        items.push(Node::row(vec![]));
                        continue 'index;
                    }
                    _ => break,
                }
            }
            // we only get here after reading at least one script
            items.push(Node::Scripts { base: Box::new(base), sup: sup.map(Box::new), sub: sub.map(Box::new) });
            return Ok(());
        }
    }
}

// ---------- commands ----------

impl Reader<'_> {
    /// Reads a command. Returns a stop when the command ends the list (`\\`, `\end`, `\right`…).
    fn structural_command(&mut self, items: &mut Vec<Node>) -> R<Option<Stop>> {
        self.nested(|this| this.command_unguarded(items))
    }

    fn command_unguarded(&mut self, items: &mut Vec<Node>) -> R<Option<Stop>> {
        let col = self.column();
        let start = self.pos;
        self.pos += 1;
        let name = self.read_command_name();
        if let Some(m) = self.macros.commands.get(&name).cloned() {
            self.expand(start, &m, col)?;
            return Ok(None);
        }
        let primitive = match name.as_str() {
            "bgroup" => Some("{"),
            "egroup" => Some("}"),
            "sp" => Some("^"),
            "sb" => Some("_"),
            "hfill" | "hss" | "quad" if name == "hfill" || name == "hss" => Some("\\quad "),
            "vrule" => Some("\\rule{0.04em}{1em}"),
            _ => None,
        };
        if let Some(text) = primitive {
            self.inject(start, text, col)?;
            return Ok(None);
        }
        if let Some(expansion) = self.package_expansion(&name, col)? {
            self.inject(start, &expansion, col)?;
            return Ok(None);
        }
        if self.definition(&name, col)? {
            return Ok(None);
        }
        let star = if self.current() == Some('*') && matches!(name.as_str(), "operatorname" | "tag" | "smash" | "resizebox") {
            self.pos += 1;
            true
        } else {
            false
        };
        // each group of commands lives in its own function: in debug builds, a single match with every
        // arm reserved ~80 KB of stack per level of nested \frac{…}.
        // Each group returns None when the name is not its own; the last one handles any name.
        #[allow(clippy::type_complexity)]
        let groups: [fn(&mut Self, &str, usize, usize, bool, &mut Vec<Node>) -> R<Option<Option<Stop>>>; 11] = [
            Self::structure_command,
            Self::space_command,
            Self::style_command,
            Self::fraction_command,
            Self::text_command,
            Self::accent_command,
            Self::unit_command,
            Self::unicode_and_table_command,
            Self::box_command,
            Self::verbatim_command,
            Self::mod_command,
        ];
        for group in groups {
            if let Some(stop) = group(self, &name, col, start, star, items)? {
                return Ok(stop);
            }
        }
        self.symbol_command(&name, col, start, items)
    }

    fn structure_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            "NC" => return Ok(Some(Some(Stop::E))),
            "NR" => {
                self.raw_optional();
                return Ok(Some(Some(Stop::NewRow)));
            }
            "\\" | "cr" | "newline" => {
                self.raw_optional();
                return Ok(Some(Some(Stop::NewRow)));
            }
            "end" => return Ok(Some(Some(Stop::End(self.raw_text()?.trim().to_string())))),
            "stopmatrix" | "stoppmatrix" | "stopbmatrix" | "stopBmatrix" | "stopvmatrix" | "stopVmatrix"
            | "stopsmallmatrix" | "stopcases" | "stopalign" | "stopaligned" | "stopgathered" | "stopsplit" | "stopmathalignment" | "stopformula" | "stopmath" => {
                return Ok(Some(Some(Stop::ContextEnd(name[4..].to_string()))));
            }
            "startmatrix" | "startpmatrix" | "startbmatrix" | "startBmatrix" | "startvmatrix" | "startVmatrix"
            | "startsmallmatrix" | "startcases" | "startalign" | "startaligned" | "startgathered" | "startsplit" | "startmathalignment" => {
                items.push(self.context_environment(&name[5..], col)?);
                return Ok(Some(None));
            }
            "startformula" | "startmath" => {
                items.push(self.context_environment(&name[5..], col)?);
                return Ok(Some(None));
            }
            "right" => return Ok(Some(Some(Stop::Right(self.delimiter()?)))),
            "middle" => return Ok(Some(Some(Stop::Middle(self.delimiter()?)))),
            "hline" | "midrule" | "toprule" | "bottomrule" => return Ok(Some(Some(Stop::Hline(false)))),
            // TeX's \noalign{…} between the rows of an alignment: vertical space there is read and left
            // out as the [length] of \\ is, a \hrule is a rule; anything else contextEquation does not draw
            "noalign" => {
                let content = self.raw_text()?;
                let spacing = |s: &str| {
                    let s = s.trim();
                    s.is_empty() || ["\\vspace", "\\vskip", "\\smallskip", "\\medskip", "\\bigskip", "\\addvspace"].iter().any(|c| s.starts_with(c))
                };
                let trimmed = content.trim();
                if trimmed == "\\hrule" || trimmed == "\\hline" {
                    return Ok(Some(Some(Stop::Hline(false))));
                }
                if !spacing(trimmed) {
                    self.tolerate(rules::E_REC_1, col, format!("\\noalign{{{trimmed}}}: only vertical space and rules are read"), "left out")?;
                }
                return Ok(Some(None));
            }
            // arydshln
            "hdashline" => {
                self.raw_optional();
                return Ok(Some(Some(Stop::Hline(true))));
            }
            "cline" | "cdashline" => {
                let range = self.raw_text()?;
                let (a, b) = range.split_once('-').unwrap_or((&range, &range));
                let n = |t: &str| t.trim().parse::<usize>().map(|v| v.saturating_sub(1));
                return match (n(a), n(b)) {
                    (Ok(a), Ok(b)) => Ok(Some(Some(Stop::Cline(a, b, name == "cdashline")))),
                    _ => self.error(col, format!("\\cline{{{range}}}: use the form \\cline{{1-2}}")),
                };
            }
            "left" => self.delimited(col)?,
            "begin" => {
                let saved = self.pos;
                let env_name = self.raw_text()?.trim().to_string();
                if let Some((open, close)) = self.macros.environments.get(&env_name).cloned() {
                    let content = self.environment_body(&env_name, col)?;
                    self.inject(start, &format!("{{{open}{content}{close}}}"), col)?;
                    return Ok(Some(None));
                }
                self.pos = saved;
                self.environment(col)?
            }
            "intertext" | "shortintertext" => return Ok(Some(Some(Stop::Intertext(Node::Text { parts: self.text_parts()? })))),
            // amsmath, for a line of multline: the line at the left or right margin (the optional
            // [length] away from it, which contextEquation, with no margin to measure from, leaves out)
            "shoveleft" | "shoveright" => {
                self.raw_optional();
                let alignment = if name == "shoveleft" { 'l' } else { 'r' };
                Node::Multicolumn { columns: 1, alignment, base: Box::new(self.argument()?) }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    /// TeX's "Extra alignment tab has been changed to \cr": a row of an `array` with more cells than its
    /// columns declare (E-REC-9; refused in strict mode). Out of line, so the frame of the environment
    /// reader, on the path of nested environments, does not grow.
    #[inline(never)]
    fn extra_cells(&mut self, rows: &[Vec<Node>], declared: usize, intertexts: &[usize], name: &str, col: usize) -> R<()> {
        let span = |cell: &Node| if let Node::Multicolumn { columns, .. } = cell { *columns } else { 1 };
        for (r, row) in rows.iter().enumerate() {
            let cells: usize = row.iter().map(span).sum();
            if cells > declared && !intertexts.contains(&r) {
                self.tolerate(rules::E_REC_9, col, format!("row {} of {name} has {cells} cells, the columns declare {declared}", r + 1), "the extra cells drawn centered")?;
            }
        }
        Ok(())
    }

    /// amsmath's "Misplaced alignment tab character &": a line of `gather`, `gathered` or `multline`,
    /// which have one column, split by `&` (E-REC-9; refused in strict mode). The cells are joined
    /// into the one cell of the line. Out of line, as `extra_cells`.
    #[inline(never)]
    fn misplaced_tabs(&mut self, rows: &mut [Vec<Node>], name: &str, col: usize) -> R<()> {
        for (r, row) in rows.iter_mut().enumerate() {
            if row.len() > 1 {
                self.tolerate(rules::E_REC_9, col, format!("'&' in line {} of {name}, which has one column", r + 1), "the cells joined")?;
                let mut joined = Vec::new();
                for cell in row.drain(..) {
                    match cell {
                        Node::Row { children } => joined.extend(children),
                        other => joined.push(other),
                    }
                }
                row.push(Node::row(joined).simplify());
            }
        }
        Ok(())
    }

    /// mathtools' `\vdotswithin` and `\shortvdotswithin`, and the accents package's `\underaccent` and
    /// `\accentset`, asked by `mod_command`: not in `structure_command`, which is on the path of every
    /// nested environment, nor in `accent_command`, on the path of every nested `\not`, where arms widen
    /// the stack frame.
    fn vdots_command(&mut self, name: &str, col: usize, start: usize, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        match name {
            "vdotswithin" => items.push(self.vdots_within()?),
            "shortvdotswithin" => self.short_vdots_within(start, col)?,
            "underaccent" | "accentset" => items.push(self.accents_package(name == "underaccent")?),
            _ => return Ok(None),
        }
        Ok(Some(None))
    }

    /// The accents package: `\underaccent{accent}{base}` puts the mark of an accent command under the
    /// base (`\underaccent{\tilde}{x}`), and anything else under it in scriptscript style;
    /// `\accentset{mark}{base}` sets the mark over the base in scriptscript style, as an accent (an
    /// ordinary atom, whatever the base).
    #[inline(never)]
    fn accents_package(&mut self, under: bool) -> R<Node> {
        let mark = self.raw_text()?;
        let base = Box::new(self.argument()?);
        let command = mark.trim().strip_prefix('\\').filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphabetic()));
        if let Some((accent, stretchy)) = command.filter(|_| under).and_then(symbols::accent) {
            return Ok(Node::Accent { base, accent, stretchy, below: true });
        }
        let mark = Some(Box::new(Node::Style { level: 3, base: Box::new(self.sub_reader(&mark, self.variant)?) }));
        Ok(if under { Node::Stacked { base, above: None, below: mark } } else { Node::Stacked { base, above: mark, below: None } })
    }

    /// mathtools' `\vdotswithin{=}`: ⋮ centered in a box as wide as `{}={}`, so that it sits under
    /// the symbol of the line above (a relation with its spaces). Short when `\shortvdotswithin`
    /// wrote it.
    fn vdots_within(&mut self) -> R<Node> {
        let short = std::mem::take(&mut self.macros.short_row);
        // read as a list, not as a group: `=` keeps its class, and its spaces
        let raw = self.raw_text()?;
        let symbol = self.sub_reader(&raw, self.variant)?;
        Ok(Node::VdotsWithin { symbol: Box::new(symbol), short })
    }

    /// mathtools' `\shortvdotswithin{=}`: a row of its own, `&\vdotswithin{=}\\` (`\vdotswithin{=}&\\`
    /// with the star, in the first column), set close to the rows around it.
    fn short_vdots_within(&mut self, start: usize, col: usize) -> R<()> {
        let starred = self.current() == Some('*');
        if starred {
            self.pos += 1;
        }
        let symbol = self.raw_text()?;
        self.macros.short_row = true;
        let row = if starred { format!("\\vdotswithin{{{symbol}}}&\\\\") } else { format!("&\\vdotswithin{{{symbol}}}\\\\") };
        self.inject(start, &row, col)
    }

    fn space_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let mu = |m: f32| Node::Space { em: m / 18.0 };
        let node: Node = match name {
            // spaces
            "," | "thinspace" => mu(3.0),
            ":" | ">" | "medspace" => mu(4.0),
            ";" | "thickspace" => mu(5.0),
            "!" | "negthinspace" => mu(-3.0),
            "negmedspace" => mu(-4.0),
            "negthickspace" => mu(-5.0),
            " " => Node::Space { em: 0.333 },
            "quad" => Node::Space { em: 1.0 },
            "qquad" => Node::Space { em: 2.0 },
            "enspace" | "enskip" => Node::Space { em: 0.5 },
            "hspace" | "mspace" | "kern" | "mkern" | "hskip" | "mskip" => {
                if self.current() == Some('*') {
                    self.pos += 1;
                }
                let measure = if self.c.get(self.pos).is_some_and(|c| *c == '{') || name.starts_with('h') && name != "hskip" { self.raw_text()? } else { self.raw_measure() };
                match self.space_measure(&measure, col)? {
                    Some(em) => Node::Space { em },
                    None => return Ok(Some(None)),
                }
            }
            // #206: plain TeX's math glue registers, which only an assignment names outside a skip
            "thinmuskip" | "medmuskip" | "thickmuskip" => {
                self.muskip_assignment(name, col)?;
                return Ok(Some(None));
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn style_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            // style and font for the rest of the group
            "displaystyle" | "textstyle" | "scriptstyle" | "scriptscriptstyle" => {
                let level = match name {
                    "displaystyle" => 0,
                    "textstyle" => 1,
                    "scriptstyle" => 2,
                    _ => 3,
                };
                let (remainder, p) = self.list()?;
                items.push(Node::Style { level, base: Box::new(Node::row(remainder)) });
                return Ok(Some(Some(p)));
            }
            "color" => {
                let color = self.color()?;
                let (remainder, p) = self.list()?;
                let base = Node::row(remainder);
                items.push(match color {
                    Some(color) => Node::Colored { color, base: Box::new(base) },
                    None => base,
                });
                return Ok(Some(Some(p)));
            }
            // generalized fractions, out of line (tex.rs): the rest of the group is their denominator
            "over" | "choose" | "atop" | "brack" | "brace" | "above" | "overwithdelims" | "atopwithdelims" | "abovewithdelims" => {
                return self.generalized_fraction(name, col, items).map(|p| Some(Some(p)));
            }
            "bf" | "rm" | "it" | "sf" | "tt" | "cal" | "mit" | "boldmath" | "unboldmath" | "normalfont" => {
                self.variant = match name {
                    "bf" | "boldmath" => Variant::Bold,
                    "rm" | "normalfont" => Variant::Upright,
                    "sf" => Variant::SansSerif,
                    "tt" => Variant::Monospace,
                    "cal" => Variant::Calligraphic,
                    _ => Variant::Italic,
                };
                return Ok(Some(None));
            }
            "tiny" | "scriptsize" | "footnotesize" | "small" | "normalsize" | "large" | "Large" | "LARGE" | "huge" | "Huge" => {
                self.registry.note(rules::L_ENT_3, format!("\\{name} ignored: the size comes from whoever uses the SVG"));
                return Ok(Some(None));
            }
            "limits" | "nolimits" | "displaylimits" => {
                let wanted = match name {
                    "limits" => Some(true),
                    "nolimits" => Some(false),
                    _ => None,
                };
                if let Some(last_item) = items.last_mut() {
                    set_limits(last_item, wanted);
                }
                return Ok(Some(None));
            }
            "label" => {
                let key = self.raw_text()?.trim().to_string();
                self.refs.line_labels.push(key);
                return Ok(Some(None));
            }
            "ref" | "eqref" => {
                let key = self.raw_text()?;
                Node::Text { parts: vec![TextPart::Text(self.label_number(name, &key, col)?)] }
            }
            "cite" => {
                let note = self.raw_optional();
                let keys = self.raw_text()?;
                Node::Text { parts: vec![TextPart::Text(self.citation(&keys, note.as_deref(), col)?)] }
            }
            // #255: \nobreak and \allowbreak (KaTeX's line break hints) are read and ignored: the tree has no
            // node for a break hint, so the line breaking of --max-width (L-BRK-1) does not see them
            "nonumber" | "notag" | "nolinebreak" | "nobreak" | "allowbreak" | "displaybreak" | "protect" | "relax" | "noindent" | "centering" | "ensuremath" => {
                return Ok(Some(None))
            }
            // \linebreak[n] is a break hint too, as \allowbreak; \qedhere places a proof's end mark, and an
            // equation converted alone is not a proof: both are read and draw nothing
            "linebreak" => {
                self.raw_optional();
                return Ok(Some(None));
            }
            "qedhere" => return Ok(Some(None)),
            // vertical space inside a formula (a row of cases, say) is left out, as the [length] of \\ is
            "vspace" => {
                if self.current() == Some('*') {
                    self.pos += 1;
                }
                self.raw_text()?;
                return Ok(Some(None));
            }
            "setlength" => return self.set_length(col).map(|()| Some(None)),
            "strut" | "mathstrut" => Node::Phantom { base: Box::new(Node::Operator { symbol: '(', class: Class::Open }), width: false, height: true },
            "tag" => {
                let label = self.raw_text()?;
                // \ref of a \label on this line gives the tag as written, \eqref the tag in parentheses
                self.refs.line_tag = Some(label.trim().to_string());
                let text = if star { label } else { format!("({label})") };
                items.push(Node::Space { em: 2.0 });
                Node::Text { parts: vec![TextPart::Text(text)] }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn fraction_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            // fractions, roots, binomials
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let numerator_align = if name == "cfrac" { self.cfrac_alignment() } else { None };
                let num = self.argument()?;
                let den = self.argument()?;
                let style = match name {
                    "dfrac" | "cfrac" => FractionStyle::Display,
                    "tfrac" => FractionStyle::Row,
                    _ => FractionStyle::Auto,
                };
                Node::Fraction { numerator_align, num: Box::new(num), den: Box::new(den), style, rule: true, thickness: None }
            }
            "binom" | "dbinom" | "tbinom" => {
                let num = self.argument()?;
                let den = self.argument()?;
                let style = match name {
                    "dbinom" => FractionStyle::Display,
                    "tbinom" => FractionStyle::Row,
                    _ => FractionStyle::Auto,
                };
                let f = Node::Fraction { numerator_align: None, num: Box::new(num), den: Box::new(den), style, rule: false, thickness: None };
                Node::Delimited { open: Some('('), close: Some(')'), parts: vec![f], middles: vec![] }
            }
            "sqrt" | "root" => {
                let index = self.optional_argument()?;
                let radicand = self.argument()?;
                Node::Root { radicand: Box::new(radicand), index: index.map(Box::new) }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn text_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            // text and fonts
            "text" | "textrm" | "textnormal" | "textup" | "mbox" | "hbox" | "textsf" | "textmd" | "textsl" | "fbox" | "framebox" => {
                let text = Node::Text { parts: self.text_parts()? };
                if matches!(name, "fbox" | "framebox") {
                    Node::Framed { base: Box::new(text), shape: Frame::Rectangle, background: None, border: None, padding: None, thickness: None }
                } else {
                    text
                }
            }
            "textsc" | "textbf" | "texttt" | "textit" | "emph" => {
                let (open, close) = match name {
                    "textsc" => (SMALL_CAPS_START, SMALL_CAPS_END),
                    "textbf" => (BOLD_START, BOLD_END),
                    "textit" | "emph" => (ITALIC_START, ITALIC_CORRECTED_END),
                    _ => (MONO_START, MONO_END),
                };
                // typewriter type has no dash ligatures: the text reader sees the variant
                let variant = self.variant;
                if name == "texttt" {
                    self.variant = Variant::Monospace;
                }
                let text = self.text_parts();
                self.variant = variant;
                Node::Text {
                    parts: text?
                        .into_iter()
                        .map(|p| match p {
                            TextPart::Text(t) => TextPart::Text(format!("{open}{t}{close}")),
                            math => math,
                        })
                        .collect(),
                }
            }
            // hyperref: the address as written, in typewriter type (_ % # ~ are its own characters)
            "url" => {
                let address = self.raw_text()?;
                let address: String = address.chars().filter(|c| !matches!(c, '\\')).collect();
                Node::Text { parts: vec![TextPart::Text(format!("{MONO_START}{address}{MONO_END}"))] }
            }
            // graphicx: \scalebox{-1}[1]{…}, its argument in text mode
            "reflectbox" => Node::Reflected { base: Box::new(Node::Text { parts: self.text_parts()? }) },
            "mathrm" | "mathup" | "mathit" | "mathbf" | "mathsf" | "mathtt" | "mathbb" | "mathcal" | "mathscr" | "mathfrak" | "boldsymbol" | "bm" | "pmb" | "mathbfit"
            | "mathbold" | "mathnormal" | "symbf" | "symrm" | "symit" | "mathds" | "mathbbm" | "Bbb" => {
                let variant = match name {
                    "mathrm" | "mathup" | "symrm" => Variant::Upright,
                    "mathit" | "mathnormal" | "symit" => Variant::Italic,
                    "mathbf" | "symbf" => Variant::Bold,
                    "boldsymbol" | "bm" | "pmb" | "mathbfit" | "mathbold" => Variant::BoldItalic,
                    "mathsf" => Variant::SansSerif,
                    "mathtt" => Variant::Monospace,
                    "mathbb" | "mathds" | "mathbbm" | "Bbb" => Variant::DoubleStruck,
                    "mathcal" | "mathscr" => Variant::Calligraphic,
                    _ => Variant::Fraktur,
                };
                let raw = self.raw_text()?;
                let node = self.sub_reader(&raw, variant)?;
                // \mathit's font is a text font (cmti, cmbxti): its letters make words, with no
                // italic correction between them (TeXbook, Appendix G, rule 17)
                if matches!(name, "mathit" | "mathbfit") {
                    words(node)
                } else {
                    node
                }
            }
            // #173: unicode-math's alphabets and its \sym… commands, and bbm's
            other if symbols::unicode_math_alphabet(other).is_some() => self.unicode_math_alphabet(other)?,
            "operatorname" => {
                let raw = self.raw_text()?;
                // in math mode TeX ignores spaces: \operatorname{ l i m } is "lim"
                // \, stays a space, as in \operatorname*{inj\,lim}
                let function_name: String = raw.replace("\\,", "\u{2009}").chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '\u{2009}')).collect();
                let function_name = function_name.replace('\u{2009}', " ");
                Node::Function { name: function_name.trim().to_string(), limits: star }
            }
            "mathop" | "mathrel" | "mathbin" | "mathord" | "mathopen" | "mathclose" | "mathpunct" | "mathinner" => {
                let class = match name {
                    "mathop" => Class::Op,
                    "mathrel" => Class::Rel,
                    "mathbin" => Class::Bin,
                    "mathopen" => Class::Open,
                    "mathclose" => Class::Close,
                    "mathpunct" => Class::Punct,
                    "mathinner" => Class::Inner,
                    _ => Class::Ord,
                };
                Node::ForcedClass { class, base: Box::new(self.argument()?) }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn accent_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            // accents, bars, braces
            "overline" | "underline" => Node::Bar { base: Box::new(self.argument()?), above: name == "overline" },
            "overbrace" | "underbrace" => Node::Brace { base: Box::new(self.argument()?), above: name == "overbrace", label: None, bracket: false },
            "overbracket" | "underbracket" => {
                // mathtools: [rule thickness][bracket height], which the font's bracket already has
                self.raw_optional();
                self.raw_optional();
                Node::Brace { base: Box::new(self.argument()?), above: name == "overbracket", label: None, bracket: true }
            }
            "underrightarrow" | "underleftarrow" | "underleftrightarrow" | "utilde" | "undertilde" | "underlinesegment" => {
                let accent = match name {
                    "underrightarrow" => '→',
                    "underleftarrow" => '←',
                    // undertilde (\utilde), accents' and mathabx's \undertilde
                    "utilde" | "undertilde" => '\u{0303}',
                    // KaTeX's line segment (see \overlinesegment in symbols.rs)
                    "underlinesegment" => '\u{20E9}',
                    _ => '↔',
                };
                Node::Accent { base: Box::new(self.argument()?), accent, stretchy: true, below: true }
            }
            // MathJax's \stackbin: \stackrel for a binary operator
            "overset" | "stackrel" | "stackbin" => {
                let above = self.argument()?;
                let base = self.base_with_class()?;
                Node::ForcedClass {
                    class: match name {
                        "stackrel" => Class::Rel,
                        "stackbin" => Class::Bin,
                        _ => natural_class(&base),
                    },
                    base: Box::new(Node::Stacked { base: Box::new(base), above: Some(Box::new(above)), below: None }),
                }
            }
            "underset" => {
                let below = self.argument()?;
                let base = self.base_with_class()?;
                let class = natural_class(&base);
                Node::ForcedClass { class, base: Box::new(Node::Stacked { base: Box::new(base), above: None, below: Some(Box::new(below)) }) }
            }
            "xrightarrow" | "xleftarrow" | "xRightarrow" | "xLeftarrow" | "xleftrightarrow" | "xLeftrightarrow" | "xrightleftharpoons" | "xleftrightharpoons" | "xmapsto"
            | "xlongequal" | "xhookrightarrow" | "xhookleftarrow" | "xtwoheadrightarrow" | "xtwoheadleftarrow" | "xrightharpoonup" | "xrightharpoondown" | "xleftharpoonup"
            | "xleftharpoondown" | "xtofrom" => {
                let symbol = match name {
                    "xrightarrow" => '→',
                    "xleftarrow" => '←',
                    "xRightarrow" => '⇒',
                    "xLeftarrow" => '⇐',
                    "xleftrightarrow" => '↔',
                    "xLeftrightarrow" => '⇔',
                    "xrightleftharpoons" => '⇌',
                    "xleftrightharpoons" => '⇋',
                    "xmapsto" => '↦',
                    "xhookrightarrow" => '↪',
                    // mathtools and extarrows
                    "xhookleftarrow" => '↩',
                    "xtwoheadrightarrow" => '↠',
                    "xtwoheadleftarrow" => '↞',
                    "xrightharpoonup" => '⇀',
                    "xrightharpoondown" => '⇁',
                    "xleftharpoonup" => '↼',
                    "xleftharpoondown" => '↽',
                    "xtofrom" => '⇄',
                    _ => '=',
                };
                let below = self.optional_argument()?;
                let above = self.argument()?;
                let above = if empty(&above) { None } else { Some(Box::new(above)) };
                Node::Arrow { symbol, above, below: below.filter(|b| !empty(b)).map(Box::new) }
            }
            // slashed.sty's \slashed, the Feynman slash, an overlay drawn like the cancels
            "cancel" | "bcancel" | "xcancel" | "sout" | "slashed" => {
                let kind = match name {
                    "bcancel" => Cancel::Backslash,
                    "xcancel" => Cancel::X,
                    "slashed" => Cancel::Slash,
                    _ => Cancel::Bar,
                };
                Node::Canceled { base: Box::new(self.argument()?), shape: kind }
            }
            "cancelto" => {
                let value = self.argument()?;
                let base = self.argument()?;
                Node::Scripts { base: Box::new(Node::Canceled { base: Box::new(base), shape: Cancel::Bar }), sup: Some(Box::new(value)), sub: None }
            }
            "boxed" => Node::Framed { base: Box::new(self.argument()?), shape: Frame::Rectangle, background: None, border: None, padding: None, thickness: None },
            // steinmetz: \phase[style]{angle}, the angle in text style unless [0] to [3] say otherwise
            // (display, text, script, scriptscript); any other option leaves the current style, as
            // the package's \ifcase does
            "phase" => {
                let level = self.raw_optional().map_or(Some(1), |o| o.trim().parse::<u8>().ok().filter(|l| *l <= 3));
                let angle = self.argument()?;
                let base = match level {
                    Some(level) => Node::Style { level, base: Box::new(angle) },
                    None => angle,
                };
                Node::Framed { base: Box::new(base), shape: Frame::Phase, background: None, border: None, padding: None, thickness: None }
            }
            // actuarialangle: \angl{n}, and \angln, \anglr, \anglk for the common durations
            "angl" | "angln" | "anglr" | "anglk" => {
                let base = match name {
                    "angl" => self.argument()?,
                    _ => Node::Identifier { name: name[4..].to_string(), variant: Variant::Italic },
                };
                Node::Framed { base: Box::new(base), shape: Frame::ActuarialAngle, background: None, border: None, padding: None, thickness: None }
            }
            "textcircled" | "circled" => Node::Framed { base: Box::new(self.argument()?), shape: Frame::Circle, background: None, border: None, padding: None, thickness: None },
            "phantom" | "hphantom" | "vphantom" => Node::Phantom { base: Box::new(self.argument()?), width: name != "vphantom", height: name != "hphantom" },
            // MathJax's \class, \cssId, \style and KaTeX's \htmlClass, \htmlId, \htmlStyle, \htmlData:
            // HTML attributes for the content's group in the SVG, kept only when safe (in strict mode too:
            // they are not wrong LaTeX, what is dropped is a warning)
            // (read in a function of its own: this match's frame is on the stack at every level of \not)
            "class" | "cssId" | "style" | "htmlClass" | "htmlId" | "htmlStyle" | "htmlData" => self.html_attributes_command(name, col)?,
            "textcolor" => {
                let color = self.color()?;
                let base = self.argument()?;
                match color {
                    Some(color) => Node::Colored { color, base: Box::new(base) },
                    None => base,
                }
            }
            // the color is the background, not the content's: the text keeps the page color
            "colorbox" => {
                let background = self.color()?;
                Node::Framed { base: Box::new(Node::Text { parts: self.text_parts()? }), shape: Frame::Fill, background, border: None, padding: None, thickness: None }
            }
            "fcolorbox" => {
                let border = self.color()?;
                let background = self.color()?;
                Node::Framed { base: Box::new(Node::Text { parts: self.text_parts()? }), shape: Frame::Rectangle, background, border, padding: None, thickness: None }
            }
            // MathJax: \bbox[background,padding,border:…]{math}, a \colorbox or \fcolorbox around math
            // (read in functions of their own: this match's frame is on the stack at every level of \not)
            "bbox" => self.bbox(col)?,
            // MathJax: \enclose{notation}[attributes]{math}, drawn as MathML's menclose
            "enclose" => self.enclose(col)?,
            // \centernot (centernot package) negates like \not; without a negated symbol, its slash is centered
            // (the reading stays in this arm: a function of its own costs a frame at every level of \not)
            "not" | "centernot" => {
                self.skip_spaces();
                let mut following = Vec::new();
                if self.current() == Some('\\') {
                    if let Some(p) = self.structural_command(&mut following)? {
                        return self.error(col, format!("\\not before {p:?}"));
                    }
                } else if self.current().is_some() {
                    following.extend(self.atom()?);
                }
                let shape = if name == "centernot" { Cancel::CenterNot } else { Cancel::Bar };
                match following.pop() {
                    Some(Node::Operator { symbol, class }) => match negate(symbol) {
                        Some(n) => Node::Operator { symbol: n, class },
                        None => Node::Canceled { base: Box::new(Node::Operator { symbol, class }), shape },
                    },
                    Some(other) => Node::Canceled { base: Box::new(other), shape },
                    None => return self.error(col, "\\not with no symbol after it"),
                }
            }
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "Bigl" | "biggl" | "Biggl" | "bigr" | "Bigr" | "biggr" | "Biggr" | "bigm" | "Bigm" | "biggm" | "Biggm" => {
                let size = match name.trim_end_matches(['l', 'r', 'm']) {
                    "big" => 1,
                    "Big" => 2,
                    "bigg" => 3,
                    _ => 4,
                };
                let Some(symbol) = self.delimiter()? else {
                    return Ok(Some(None));
                };
                let class = match name.chars().last() {
                    Some('l') => Class::Open,
                    Some('r') => Class::Close,
                    Some('m') => Class::Rel,
                    _ => symbols::character(symbol).map_or(Class::Ord, |s| s.class),
                };
                Node::FixedDelimiter { symbol, class, size }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn unit_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            // units, chemistry, arithmetic. siunitx options ([…]) come before the arguments and are ignored
            "SI" | "qty" => {
                self.raw_optional();
                let value = self.raw_text()?;
                // siunitx leaves out an explicit plus (`retain-explicit-plus` is off) and adds the
                // zero of a bare point (`add-integer-zero`)
                let value = if self.macros.siunitx.retain_plus { value } else { value.trim_start().strip_prefix('+').map_or(value.clone(), str::to_string) };
                let value = match value.trim_start().strip_prefix('-') {
                    Some(rest) if rest.starts_with('.') => format!("-0{rest}"),
                    _ if value.trim_start().starts_with('.') => format!("0{}", value.trim_start()),
                    _ => value,
                };
                let unit = self.raw_text()?;
                self.quantity(Some(value), &unit, col)?
            }
            "si" | "unit" => {
                self.raw_optional();
                let unit = self.raw_text()?;
                self.quantity(None, &unit, col)?
            }
            // siunitx v3's complex numbers, in siunitx.rs
            "complexnum" => {
                self.raw_optional();
                let value = self.raw_text()?;
                self.complex_number(&value, col)?.0
            }
            "complexqty" => {
                self.raw_optional();
                let value = self.raw_text()?;
                let unit = self.raw_text()?;
                self.complex_quantity(&value, &unit, col)?
            }
            // \tablenum: the number as \num (the alignment on the decimal marker of an S column is not done)
            "num" | "tablenum" => {
                self.raw_optional();
                let value = self.raw_text()?;
                self.raw_optional();
                self.number_value(&value, col)?
            }
            "ang" => {
                self.raw_optional();
                let value = self.raw_text()?;
                self.angle(&value, col)?
            }
            // L-UNI-2: ranges, lists and products repeat the unit on every value, as siunitx does by default
            "SIrange" | "qtyrange" | "numrange" | "angrange" => {
                self.raw_optional();
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                let unit = if matches!(name, "SIrange" | "qtyrange") { Some(self.raw_text()?) } else { None };
                let a = self.listed_value(name, &a, unit.as_deref(), col)?;
                let b = self.listed_value(name, &b, unit.as_deref(), col)?;
                // siunitx's range-phrase: the language's "to", when the embedded font can write it, as for lists
                let to = Some(self.locale.phrase("op.–")).filter(|w| w.chars().all(|c| self.font.get().glyph(c).is_some()));
                self.registry.note(rules::L_UNI_2, format!("\\{name}: range written with '{}'", to.unwrap_or("–")));
                let between = match to {
                    Some(to) => Node::Text { parts: vec![TextPart::Text(format!(" {to} "))] },
                    None => Node::Operator { symbol: '–', class: Class::Rel },
                };
                Node::row(vec![a, between, b])
            }
            "SIlist" | "qtylist" | "numlist" | "anglist" => {
                self.raw_optional();
                let values = self.raw_text()?;
                let unit = if matches!(name, "SIlist" | "qtylist") { Some(self.raw_text()?) } else { None };
                let values: Vec<&str> = values.split(';').map(str::trim).filter(|v| !v.is_empty()).collect();
                // the last separator is the language's "and", when the embedded font can write it (it has
                // no CJK, Devanagari or Arabic letters); otherwise a comma, like the others
                let and = Some(self.locale.phrase("list_and")).filter(|w| w.chars().all(|c| self.font.get().glyph(c).is_some()));
                let mut children = Vec::new();
                for (i, v) in values.iter().enumerate() {
                    match and {
                        Some(and) if i > 0 && i + 1 == values.len() => children.push(Node::Text { parts: vec![TextPart::Text(format!(" {and} "))] }),
                        _ if i > 0 => children.push(Node::Operator { symbol: ',', class: Class::Punct }),
                        _ => {}
                    }
                    children.push(self.listed_value(name, v, unit.as_deref(), col)?);
                }
                self.registry.note(rules::L_UNI_2, format!("\\{name}: list closed with '{}'", and.unwrap_or(",")));
                Node::row(children)
            }
            "SIproduct" | "qtyproduct" | "numproduct" => {
                self.raw_optional();
                let values = self.raw_text()?;
                let unit = if name == "numproduct" { None } else { Some(self.raw_text()?) };
                let mut children = Vec::new();
                for (i, v) in values.split('x').map(str::trim).enumerate() {
                    if i > 0 {
                        children.push(Node::Operator { symbol: '×', class: Class::Bin });
                    }
                    children.push(self.listed_value(name, v, unit.as_deref(), col)?);
                }
                Node::row(children)
            }
            "ce" => match self.chemistry_command(start, col)? {
                Some(node) => node,
                None => return Ok(Some(None)),
            },
            "pu" => {
                // mhchem's physical units: a number, then the unit with ^ exponents and . or * between units
                let raw = self.raw_text()?;
                let raw = raw.trim();
                let (value, unit) = match raw.split_once(char::is_whitespace) {
                    Some((v, u)) if siunitx_number(v, false).is_some() => (Some(v.to_string()), u.trim().to_string()),
                    _ if siunitx_number(raw, false).is_some() => (Some(raw.to_string()), String::new()),
                    _ => (None, raw.to_string()),
                };
                let unit_node = self.sub_reader(&mhchem_unit(&unit), Variant::Upright)?;
                self.quantity_node(value, unit_node, col)?
            }
            "xymatrix" => self.xymatrix(col)?,
            #[cfg(feature = "chem")]
            "chemfig" | "schemestart" | "chemname" | "subscheme" | "charge" | "Charge" => self.chem_construction(name, col)?,
            #[cfg(feature = "chem")]
            "chemleft" | "chemright" | "chemup" | "chemdown" | "chemskipalign" | "merge" => return self.chem_delimiters(name, start, col).map(|()| Some(None)),
            "divchave" => {
                let dividend = self.raw_text()?.trim().to_string();
                let divisor = self.raw_text()?.trim().to_string();
                if !is_integer(&dividend) || !is_integer(&divisor) || divisor.trim_start_matches('0').is_empty() {
                    let problem = "\\divchave{dividend}{divisor} takes integers, with a nonzero divisor";
                    self.tolerate(rules::E_REC_9, col, problem, "written as inline division")?;
                    self.sub_reader(&format!("{{{dividend}}} \\div {{{divisor}}}"), self.variant)?
                } else {
                    Node::LongDivision { dividend, divisor }
                }
            }
            "contaarmada" => {
                let op = self.raw_text()?.trim().to_string();
                let operation = match op.as_str() {
                    "+" => Some('+'),
                    "-" => Some('−'),
                    "x" | "\\times" | "×" | "*" => Some('×'),
                    _ => None,
                };
                let a = self.raw_text()?.trim().to_string();
                let b = self.raw_text()?.trim().to_string();
                match operation {
                    Some(operation) if is_integer(&a) && is_integer(&b) => Node::ColumnArithmetic { operation, terms: vec![a, b] },
                    _ => {
                        let problem = if operation.is_none() { "\\contaarmada{op}{a}{b}: op is +, - or \\times" } else { "\\contaarmada takes integers" };
                        self.tolerate(rules::E_REC_9, col, problem, "written as an inline calculation")?;
                        let symbol = operation.map_or(op, String::from);
                        self.sub_reader(&format!("{{{a}}} {symbol} {{{b}}}"), self.variant)?
                    }
                }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn unicode_and_table_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            "unicode" => {
                // MathJax: \unicode[height,depth][font]{…} or \unicode[font]{…}; the font is MathJax's
                // way to find a glyph its own fonts lack, and contextEquation has its own fallback fonts
                if self.raw_optional().is_some() {
                    self.raw_optional();
                }
                let raw = self.raw_text()?;
                match unicode_code_point(&raw) {
                    Some(c) => Node::Operator { symbol: c, class: Class::Ord },
                    None => return self.error(col, format!("\\unicode{{{raw}}}: invalid code point")),
                }
            }
            "LaTeX" | "TeX" => Node::Text { parts: vec![TextPart::Text(name.to_string())] },
            "substack" => self.argument_table(TableKind::Substack, col)?,
            "cases" => self.argument_table(TableKind::Cases, col)?,
            "matrix" => self.argument_table(TableKind::Matrix, col)?,
            "pmatrix" => self.argument_table(TableKind::PMatrix, col)?,
            "bordermatrix" => self.argument_table(TableKind::BorderMatrix, col)?,
            "eqalign" | "eqalignno" | "displaylines" => self.argument_table(TableKind::Aligned, col)?,
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn box_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            "genfrac" => {
                let open = self.raw_text()?;
                let close = self.raw_text()?;
                let thickness = self.raw_text()?;
                let style = self.raw_text()?;
                let num = self.argument()?;
                let den = self.argument()?;
                let style = match style.trim() {
                    "0" => FractionStyle::Display,
                    "" => FractionStyle::Auto,
                    _ => FractionStyle::Row,
                };
                let rule = !thickness.trim().trim_start_matches(['0', '.']).trim_end_matches(char::is_alphabetic).is_empty() || thickness.trim().is_empty();
                // an empty thickness is the font's; a given one is drawn as given
                let given = measure_em(&thickness).filter(|_| rule);
                let f = Node::Fraction { numerator_align: None, num: Box::new(num), den: Box::new(den), style, rule, thickness: given };
                let delim = |t: &str| -> Option<char> {
                    let t = t.trim();
                    match t.strip_prefix('\\') {
                        Some(n) => symbols::delimiter_command(n).flatten(),
                        None => t.chars().next().and_then(|c| symbols::delimiter_char(c).flatten()),
                    }
                };
                if open.trim().is_empty() && close.trim().is_empty() {
                    f
                } else {
                    Node::Delimited { open: delim(&open), close: delim(&close), parts: vec![f], middles: vec![] }
                }
            }
            "mathchoice" => {
                let mut branches = Vec::with_capacity(4);
                for _ in 0..4 {
                    branches.push(self.argument()?);
                }
                Node::Choice { branches }
            }
            "vcenter" => Node::VCenter { base: Box::new(self.argument()?) },
            "smash" => {
                let option = self.raw_optional().unwrap_or_default();
                let base = self.argument()?;
                Node::Smashed { base: Box::new(base), height: option != "b", depth: option != "t" }
            }
            // LaTeX's \lefteqn{…}: its argument in display style, with no width, overlapping to the right
            // (eqnarray's long left-hand side)
            "lefteqn" => {
                let base = self.argument()?;
                Node::Overlapped { base: Box::new(Node::Style { level: 0, base: Box::new(base) }), side: 'r' }
            }
            "mathllap" | "mathrlap" | "mathclap" | "llap" | "rlap" | "clap" => {
                let side = match name.trim_start_matches("math") {
                    "llap" => 'l',
                    "rlap" => 'r',
                    _ => 'c',
                };
                let base = if name.starts_with("math") { self.argument()? } else { Node::Text { parts: self.text_parts()? } };
                Node::Overlapped { base: Box::new(base), side }
            }
            // graphicx's boxes, their argument in text mode
            "raisebox" => {
                let measure = self.raw_text()?;
                let em = measure_em(&measure).ok_or(Error::Syntax { column: col, message: format!("unrecognized measure: {measure}") })?;
                let height = self.box_length(col)?;
                let depth = if height.is_some() { self.box_length(col)? } else { None };
                Node::Raised { em, height: height.flatten(), depth: depth.flatten(), base: Box::new(Node::Text { parts: self.text_parts()? }) }
            }
            "scalebox" => {
                let factor = |s: &str| s.trim().parse::<f32>().ok().filter(|f| f.is_finite()).map(|f| f.signum() * f.abs().clamp(0.05, 20.0));
                let x = factor(&self.raw_text()?).ok_or(Error::Syntax { column: col, message: "\\scalebox{factor}{…} takes a number".into() })?;
                let y = match self.raw_optional() {
                    Some(y) => Some(factor(&y).ok_or(Error::Syntax { column: col, message: "\\scalebox{h}[v]{…} takes numbers".into() })?),
                    None => None,
                };
                Node::Scaled { factor: x, vertical: y.filter(|y| *y != x), base: Box::new(Node::Text { parts: self.text_parts()? }) }
            }
            "resizebox" => {
                let mut size = Vec::with_capacity(2);
                for _ in 0..2 {
                    let raw = self.raw_text()?;
                    size.push(match raw.trim() {
                        "!" => None,
                        m => match box_length(m) {
                            Some(l) => Some(l),
                            None => return self.error(col, format!("\\resizebox: unrecognized measure: {m}")),
                        },
                    });
                }
                Node::Resized { width: size[0], height: size[1], total: star, base: Box::new(Node::Text { parts: self.text_parts()? }) }
            }
            "rotatebox" => {
                // of the options, origin=…; x=, y= and units= are ignored
                let options = self.raw_optional().unwrap_or_default();
                let origin =
                    options.split(',').find_map(|o| o.trim().strip_prefix("origin")).map(|o| o.trim_start().trim_start_matches('=').trim().to_string()).unwrap_or_default();
                let origin = if options.contains("origin") && origin.is_empty() { "c".to_string() } else { origin };
                let angle = self.raw_text()?;
                let degrees =
                    angle.trim().parse::<f32>().ok().filter(|a| a.is_finite()).ok_or(Error::Syntax { column: col, message: "\\rotatebox{angle}{…} takes a number".into() })?;
                Node::Rotated { degrees: degrees % 360.0, origin, base: Box::new(Node::Text { parts: self.text_parts()? }) }
            }
            "rule" => {
                let raise = self.raw_optional().and_then(|m| measure_em(&m)).unwrap_or(0.0);
                let width = self.raw_text()?;
                let height = self.raw_text()?;
                match (measure_em(&width), measure_em(&height)) {
                    (Some(width), Some(height)) => Node::Rule { width, height, raise },
                    _ => return self.error(col, "\\rule{width}{height}: unrecognized measures"),
                }
            }
            "sideset" => {
                let before = self.raw_text()?;
                let after = self.raw_text()?;
                let pre = self.sub_reader(&format!("{{}}{before}"), self.variant)?;
                let pos = self.sub_reader(&format!("{{}}{after}"), self.variant)?;
                let op = self.argument()?;
                let mut tmp = vec![op];
                self.skip_spaces();
                if matches!(self.current(), Some('^' | '_')) {
                    self.scripts(&mut tmp)?;
                }
                items.push(pre);
                items.extend(tmp);
                pos
            }
            "textsuperscript" | "textsubscript" => {
                let t = Node::Text { parts: self.text_parts()? };
                if name == "textsuperscript" {
                    Node::Scripts { base: Box::new(Node::row(vec![])), sup: Some(Box::new(t)), sub: None }
                } else {
                    Node::Scripts { base: Box::new(Node::row(vec![])), sup: None, sub: Some(Box::new(t)) }
                }
            }
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn verbatim_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        let node: Node = match name {
            "verb" => {
                if self.current() == Some('*') {
                    self.pos += 1;
                }
                let delim = self.current().ok_or(Error::Syntax { column: col, message: "\\verb without a delimiter".into() })?;
                self.pos += 1;
                let mut children = Vec::new();
                loop {
                    match self.current() {
                        None => return self.error(col, format!("\\verb{delim}… without the closing '{delim}'")),
                        Some(x) if x == delim => {
                            self.pos += 1;
                            break;
                        }
                        Some(x) => {
                            self.pos += 1;
                            children.push(if x.is_ascii_alphanumeric() {
                                Node::Identifier { name: x.to_string(), variant: Variant::Monospace }
                            } else if x == ' ' {
                                Node::Space { em: 0.525 }
                            } else {
                                Node::Operator { symbol: if x == '-' { '-' } else { x }, class: Class::Ord }
                            });
                        }
                    }
                }
                Node::row(children)
            }
            "multicolumn" => {
                let n: usize = self.raw_text()?.trim().parse().map_err(|_| Error::Syntax { column: col, message: "\\multicolumn{n}{…}{…} takes a number".into() })?;
                let spec = self.raw_text()?;
                let alignment = spec.chars().find(|c| matches!(c, 'l' | 'c' | 'r')).unwrap_or('c');
                Node::Multicolumn { columns: n.clamp(1, 50), alignment, base: Box::new(self.argument()?) }
            }
            "hdotsfor" | "dotsfor" => {
                self.raw_optional();
                let n: usize = self.raw_text()?.trim().parse().unwrap_or(1);
                let points = (0..n.clamp(1, 50) * 3).flat_map(|_| [Node::Operator { symbol: '.', class: Class::Ord }, Node::Space { em: 0.35 }]).collect();
                Node::Multicolumn { columns: n.clamp(1, 50), alignment: 'c', base: Box::new(Node::row(points)) }
            }
            _ => return self.tex_command(name, items),
        };
        items.push(node);
        Ok(Some(None))
    }

    fn mod_command(&mut self, name: &str, col: usize, start: usize, star: bool, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let _ = (col, start, star);
        if let Some(done) = self.vdots_command(name, col, start, items)? {
            return Ok(Some(done));
        }
        let mu = |m: f32| Node::Space { em: m / 18.0 };
        let node: Node = match name {
            // modular arithmetic, with amsmath's spacing
            "pmod" | "mod" | "pod" | "bmod" => {
                let text_mod = || Node::Text { parts: vec![TextPart::Text("mod".into())] };
                if name == "bmod" {
                    Node::ForcedClass { class: Class::Bin, base: Box::new(text_mod()) }
                } else {
                    let arg = self.argument()?;
                    let mut children = vec![mu(if name == "mod" { 12.0 } else { 18.0 })];
                    if name != "mod" {
                        children.push(Node::Operator { symbol: '(', class: Class::Open });
                    }
                    if name != "pod" {
                        children.push(text_mod());
                        children.push(mu(6.0));
                    }
                    children.push(arg);
                    if name != "mod" {
                        children.push(Node::Operator { symbol: ')', class: Class::Close });
                    }
                    Node::row(children)
                }
            }
            "imath" | "jmath" | "i" | "j" => Node::Identifier { name: if matches!(name, "imath" | "i") { "ı" } else { "ȷ" }.into(), variant: self.variant },
            "lt" => Node::Operator { symbol: '<', class: Class::Rel },
            "gt" => Node::Operator { symbol: '>', class: Class::Rel },
            _ => return Ok(None),
        };
        items.push(node);
        Ok(Some(None))
    }

    /// Functions, large operators, letters and symbols; whatever is left is an unknown command.
    fn symbol_command(&mut self, other: &str, col: usize, start: usize, items: &mut Vec<Node>) -> R<Option<Stop>> {
        let node = if let Some((canonical, limits)) = symbols::function(other) {
            if canonical != other {
                self.registry.note(rules::L_FUN_1, format!("\\{other} recognized as the function {canonical}"));
            }
            Node::Function { name: canonical.to_string(), limits }
        } else if let Some((symbol, _)) = symbols::large_operator(other) {
            let operator = Node::LargeOperator { symbol, limits: None };
            // plain TeX's \intop is \mathop without \nolimits: limits above and below in display
            if matches!(other, "intop" | "ointop") {
                Node::ForcedClass { class: Class::Op, base: Box::new(operator) }
            } else {
                operator
            }
        } else if let Some(letter) = other.strip_prefix("var").and_then(symbols::greek).filter(|g| g.is_uppercase()) {
            // amsmath's \varGamma…\varOmega: the capital Greek letters in italic
            Node::Identifier { name: letter.to_string(), variant: Variant::Italic }
        } else if let Some(letter) = symbols::greek(other) {
            let variant = if symbols::greek_upright(other) && self.variant == Variant::Italic { Variant::Upright } else { self.variant };
            Node::Identifier { name: letter.to_string(), variant }
        } else if let Some((accent, stretchy)) = symbols::accent(other) {
            // the "under" ones (⏝, ⏡) go below the base
            Node::Accent { base: Box::new(self.argument()?), accent, stretchy, below: matches!(accent, '⏝' | '⏡') }
        } else if matches!(other, "implies" | "impliedby" | "iff") {
            return self.long_implication(other, start, col, items);
        } else if let Some(Symbol { symbol, class }) = symbols::command(other) {
            // \boldsymbol{\nabla}, \bm{\nabla}: TeX's bold symbol font has an upright bold nabla, the 𝛁 of
            // physics' \grad (U+1D6C1)
            let symbol = if symbol == '∇' && self.variant == Variant::BoldItalic { '𝛁' } else { symbol };
            Node::Operator { symbol, class }
        } else {
            return self.unknown_command(other, start, col, items);
        };
        items.push(node);
        Ok(None)
    }

    /// #250: amsmath's `\implies`, `\impliedby` and `\iff`, defined as `\;\Longrightarrow\;`,
    /// `\;\Longleftarrow\;` and `\;\Longleftrightarrow\;`: the long arrow with a thick space on each
    /// side. The arrow is the last node read, so `\not\implies` still strikes the arrow; the space after
    /// it is read next, from the input. Out of line: `symbol_command` ends every chain of commands.
    #[inline(never)]
    fn long_implication(&mut self, name: &str, start: usize, col: usize, items: &mut Vec<Node>) -> R<Option<Stop>> {
        let symbol = symbols::command(name).map_or('⟹', |s| s.symbol);
        items.push(Node::Space { em: 5.0 / 18.0 });
        items.push(Node::Operator { symbol, class: Class::Rel });
        self.inject(start, "\\;", col)?;
        Ok(None)
    }

    /// #173: `\mathbfcal{L}`, `\symup{d}`: the argument in the alphabet the command names. Out of line,
    /// so that `text_command`'s frame, on the stack of every nested argument, does not grow.
    #[inline(never)]
    fn unicode_math_alphabet(&mut self, name: &str) -> R<Node> {
        let variant = symbols::unicode_math_alphabet(name).unwrap_or(Variant::Upright);
        let raw = self.raw_text()?;
        self.sub_reader(&raw, variant)
    }

    /// The optional `[length]` of a graphicx box: `None` without brackets; `Some(None)` for a length it
    /// cannot read, ignored in tolerant mode (E-REC-9).
    fn box_length(&mut self, col: usize) -> R<Option<Option<BoxLength>>> {
        let Some(raw) = self.raw_optional() else { return Ok(None) };
        match box_length(&raw) {
            Some(l) => Ok(Some(Some(l))),
            None => {
                self.tolerate(rules::E_REC_9, col, format!("unrecognized measure: {}", raw.trim()), "ignored")?;
                Ok(Some(None))
            }
        }
    }

    fn raw_measure(&mut self) -> String {
        self.skip_spaces();
        let mut s = String::new();
        // #206: a glue register, maybe with a factor: \thinmuskip, 2\medmuskip, -\thickmuskip
        let factor: String = self.c[self.pos..].iter().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')).collect();
        let at = self.pos + factor.chars().count();
        if self.c.get(at) == Some(&'\\') {
            let name: String = self.c[at + 1..].iter().take_while(|c| c.is_ascii_alphabetic()).collect();
            if !name.is_empty() {
                self.pos = at + 1 + name.chars().count();
                // #213: a box dimension takes its register, a number or a name (\wd0, \ht\mybox)
                if matches!(name.as_str(), "wd" | "ht" | "dp") {
                    let rest = &self.c[self.pos..];
                    let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
                    let len = if digits == 0 && rest.first() == Some(&'\\') { 1 + rest[1..].iter().take_while(|c| c.is_ascii_alphabetic()).count() } else { digits };
                    let register: String = rest[..len].iter().collect();
                    self.pos += len;
                    return format!("{factor}\\{name}{register}");
                }
                return format!("{factor}\\{name}");
            }
        }
        while let Some(c) = self.current() {
            if c.is_ascii_digit() || c == '.' || c == '-' || c == ',' || (c.is_ascii_alphabetic() && s.len() < 12) {
                s.push(c);
                self.pos += 1;
                if s.ends_with("em") || s.ends_with("ex") || s.ends_with("pt") || s.ends_with("mu") || s.ends_with("cm") || s.ends_with("mm") || s.ends_with("in") {
                    break;
                }
            } else {
                break;
            }
        }
        s
    }

    /// A valid color, or None in tolerant mode when it is unusable (the content comes out uncolored).
    fn color(&mut self) -> R<Option<String>> {
        let col = self.column();
        let model = self.raw_optional();
        let raw = self.raw_text()?.trim().to_string();
        let color = match model.as_deref().map(str::trim) {
            // the digits as written
            Some("HTML") => format!("#{raw}"),
            // a name the equation defined, or an xcolor mix (red!40!blue); any other name as written
            None | Some("named") => color::expression(&raw, &self.macros.colors).map_or(raw, |d| d.css),
            // a model's value as #rrggbb; a value that does not fit stays invalid, with its model
            Some(m) => color::model(m, &raw).map_or_else(|| format!("[{m}]{{{raw}}}"), color::hex),
        };
        self.checked_color(color, col)
    }

    /// A color named in place (`\bbox[brand]`, `\enclose{…}[mathcolor=brand]`): a color the equation
    /// defined or an xcolor mix becomes its CSS value, any other name stays as written; then checked.
    fn named_color(&mut self, name: &str, col: usize) -> R<Option<String>> {
        let css = color::expression(name, &self.macros.colors).map_or_else(|| name.to_string(), |d| d.css);
        self.checked_color(css, col)
    }

    /// `color` when it is a color contextEquation can write into the SVG; None in tolerant mode otherwise.
    fn checked_color(&mut self, color: String, col: usize) -> R<Option<String>> {
        let valid = color.chars().all(|c| c.is_ascii_alphanumeric() || "#(),. ".contains(c)) && !color.is_empty() && color.len() < 40;
        if valid {
            return Ok(Some(color));
        }
        self.tolerate(rules::E_REC_9, col, format!("invalid color: {color}"), "drawn without color")?;
        Ok(None)
    }

    /// `\bbox[…]` (MathJax): the parts, separated by commas, are a background color, a padding (`5px`)
    /// and CSS styles, of which contextEquation draws `border:` (width, style and color).
    fn bbox(&mut self, col: usize) -> R<Node> {
        let spec = self.raw_optional().unwrap_or_default();
        let base = self.argument()?;
        let (mut background, mut padding, mut border, mut thickness, mut framed) = (None, None, None, None, false);
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            if let Some(em) = measure_em(part).filter(|v| v.is_finite() && *v >= 0.0) {
                padding = Some(em);
            } else if let Some((property, value)) = part.split_once(':') {
                if !property.trim().eq_ignore_ascii_case("border") {
                    self.tolerate(rules::E_REC_9, col, format!("\\bbox style not drawn: {part}"), "ignored")?;
                    continue;
                }
                framed = true;
                for word in value.split_whitespace() {
                    if let Some(em) = measure_em(word).filter(|v| v.is_finite() && *v >= 0.0) {
                        thickness = Some(em);
                    } else if matches!(word, "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset") {
                        if word != "solid" {
                            self.tolerate(rules::E_REC_9, col, format!("\\bbox border style not drawn: {word}"), "drawn solid")?;
                        }
                    } else {
                        border = self.named_color(word, col)?;
                    }
                }
            } else if background.is_none() {
                background = self.named_color(part, col)?;
            } else {
                self.tolerate(rules::E_REC_9, col, format!("\\bbox part not understood: {part}"), "ignored")?;
            }
        }
        let shape = if framed { Frame::Rectangle } else { Frame::Fill };
        // MathJax adds no space of its own: without a padding the background fits the content
        Ok(Node::Framed { base: Box::new(base), shape, background, border, padding: Some(padding.unwrap_or(0.0)), thickness })
    }

    /// `\enclose{notation}[attributes]{…}` (MathJax): the same tree as MathML's `menclose`, with its
    /// `mathcolor` and `mathbackground` attributes.
    fn enclose(&mut self, col: usize) -> R<Node> {
        let notation = self.raw_text()?;
        let attributes = self.raw_optional();
        let base = self.argument()?;
        for n in notation.split(|c: char| c.is_whitespace() || c == ',').filter(|n| !n.is_empty()) {
            if !super::mathml::ENCLOSE_NOTATIONS.contains(&n) {
                self.tolerate(rules::E_REC_9, col, format!("\\enclose notation not drawn: {n}"), "ignored")?;
            }
        }
        let mut out = super::mathml::enclose(base, &notation);
        for attribute in attributes.as_deref().unwrap_or_default().split(',').map(str::trim).filter(|a| !a.is_empty()) {
            let (name, value) = attribute.split_once('=').unwrap_or((attribute, ""));
            let value = value.trim().trim_matches(['"', '\'']).to_string();
            match name.trim() {
                "mathcolor" => {
                    if let Some(color) = self.named_color(&value, col)? {
                        out = Node::Colored { color, base: Box::new(out) };
                    }
                }
                "mathbackground" => {
                    if let Some(background) = self.named_color(&value, col)? {
                        out = Node::Framed { base: Box::new(out), shape: Frame::Fill, background: Some(background), border: None, padding: None, thickness: None };
                    }
                }
                _ => self.tolerate(rules::E_REC_9, col, format!("\\enclose attribute not drawn: {attribute}"), "ignored")?,
            }
        }
        Ok(out)
    }

    fn delimiter(&mut self) -> R<Option<char>> {
        self.skip_spaces();
        let col = self.column();
        match self.current() {
            Some('\\') => {
                self.pos += 1;
                let name = self.read_command_name();
                match symbols::delimiter_command(&name) {
                    Some(d) => Ok(d),
                    None => {
                        self.tolerate(rules::E_REC_9, col, format!("\\{name} is not a delimiter"), "used the empty one")?;
                        Ok(None)
                    }
                }
            }
            Some(c) => {
                self.pos += 1;
                match symbols::delimiter_char(c) {
                    Some(d) => Ok(d),
                    None => {
                        self.tolerate(rules::E_REC_9, col, format!("'{c}' is not a delimiter"), "drawn as is")?;
                        Ok(Some(c))
                    }
                }
            }
            None => {
                self.tolerate(rules::E_REC_7, col, "missing delimiter", "used the empty one")?;
                Ok(None)
            }
        }
    }

    fn delimited(&mut self, col: usize) -> R<Node> {
        let open = self.delimiter()?;
        let variant = self.variant;
        let mut parts = Vec::new();
        let mut middles = Vec::new();
        loop {
            let (items, stop) = self.list()?;
            parts.push(Node::row(items));
            match stop {
                Stop::Right(close) => {
                    self.variant = variant;
                    return Ok(Node::Delimited { open, close, parts, middles });
                }
                Stop::Middle(m) => middles.push(m.unwrap_or('|')),
                Stop::Eof => {
                    self.tolerate(rules::E_REC_5, col, "\\left without \\right", "closed with an empty delimiter")?;
                    self.variant = variant;
                    return Ok(Node::Delimited { open, close: None, parts, middles });
                }
                other => {
                    self.tolerate(rules::E_REC_5, col, format!("\\left opened here ended in {other:?}"), "closed there")?;
                    self.variant = variant;
                    return Ok(Node::Delimited { open, close: None, parts, middles });
                }
            }
        }
    }

    fn environment(&mut self, col: usize) -> R<Node> {
        let name = self.raw_text()?.trim().to_string();
        self.environment_named(&name, false, col)
    }

    fn context_environment(&mut self, name: &str, col: usize) -> R<Node> {
        self.environment_named(name, true, col)
    }

    fn environment_named(&mut self, name: &str, context: bool, col: usize) -> R<Node> {
        if matches!(name, "cases*" | "dcases*" | "rcases*" | "drcases*") {
            return self.text_column_cases(name, col);
        }
        if context && matches!(name, "formula" | "math") {
            let (items, stop) = self.list()?;
            return match stop {
                Stop::ContextEnd(end) if end == name => Ok(Node::row(items)),
                _ => self.error(col, format!("\\start{name} without \\stop{name}")),
            };
        }
        let base = if context {
            match name {
                "mathalignment" => "align",
                "smallmatrix" | "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "cases" | "align" | "aligned" | "gathered" | "split" => name,
                _ => return self.error(col, format!("unsupported ConTeXt math environment: {name}")),
            }
        } else {
            name.trim_end_matches('*')
        };
        let mut columns: Vec<char> = Vec::new();
        let mut verticals = Vec::new();
        let mut dashed = Vec::new();
        let mut between = Vec::new();
        let kind = match base {
            "matrix" => TableKind::Matrix,
            "pmatrix" => TableKind::PMatrix,
            "bmatrix" => TableKind::BMatrix,
            "Bmatrix" => TableKind::BraceMatrix,
            "vmatrix" => TableKind::VMatrix,
            "Vmatrix" => TableKind::VVMatrix,
            "smallmatrix" => TableKind::SmallMatrix,
            "cases" | "dcases" | "numcases" => TableKind::Cases,
            "rcases" | "drcases" => TableKind::RCases,
            "aligned" | "align" | "alignat" | "alignedat" | "split" | "eqnarray" | "flalign" | "flalignat" | "mathalignment" => TableKind::Aligned,
            "gathered" | "gather" | "multline" => TableKind::Centered,
            "array" | "tabular" => TableKind::Array,
            "subarray" => TableKind::Substack,
            "CD" => {
                let raw = self.environment_body("CD", col)?;
                return self.cd_diagram(&raw, col);
            }
            "tikzcd" => {
                let raw = self.environment_body("tikzcd", col)?;
                return self.tikzcd(&raw, col);
            }
            "subequations" if !context => {
                let (items, stop) = self.list()?;
                return match stop {
                    Stop::End(end) if end == name => Ok(Node::row(items)),
                    _ => self.error(col, format!("\\begin{{{name}}} without \\end{{{name}}}")),
                };
            }
            "equation" | "displaymath" | "math" if !context => {
                let (items, stop) = self.list()?;
                if numbered_environment(&name) {
                    self.refs.end_line();
                }
                return match stop {
                    Stop::End(end) if end == name => Ok(Node::row(items)),
                    _ => self.error(col, format!("\\begin{{{name}}} without \\end{{{name}}}")),
                };
            }
            _ => {
                self.tolerate(rules::E_REC_6, col, format!("unsupported environment: {name}"), "read as a table")?;
                TableKind::Centered
            }
        };
        if matches!(base, "alignat" | "alignedat" | "flalignat") {
            self.raw_text()?;
        }
        if matches!(base, "pmatrix" | "bmatrix" | "matrix" | "vmatrix" | "Bmatrix" | "Vmatrix" | "smallmatrix") && name.ends_with('*') {
            if let Some(a) = self.raw_optional() {
                columns = vec![a.trim().chars().next().unwrap_or('c'); 1];
            }
        }
        if matches!(kind, TableKind::Array | TableKind::Substack) {
            self.raw_optional();
            let spec = self.raw_text()?;
            self.array_columns(&spec, &mut columns, &mut verticals, &mut dashed, &mut between, col)?;
        }

        let (mut rows, horizontals, intertexts) = self.table_body(Some(name), context, col)?;
        if matches!(base, "gather" | "gathered" | "multline") {
            self.misplaced_tabs(&mut rows, &name, col)?;
        }
        if base == "multline" {
            multline_alignments(&mut rows);
        }
        if kind == TableKind::Array {
            self.extra_cells(&rows, columns.len(), &intertexts, &name, col)?;
        }
        if !columns.is_empty() && kind != TableKind::Array {
            // pmatrix*[r]: the same alignment in every column
            let a = columns[0];
            let n = rows.iter().map(Vec::len).max().unwrap_or(1);
            columns = vec![a; n];
        }
        Ok(Node::Table {
            shape: kind,
            rows,
            columns,
            verticals,
            dashed,
            horizontals,
            between,
            intertexts,
            stretch: self.array_stretch(),
            colsep: self.macros.array_colsep.map_or(500, |em| (em * 1000.0).round() as u16),
        })
    }

    /// mathtools' `cases*`, `dcases*`, `rcases*` and `drcases*`: the second column is text, with `$…$` for
    /// math in it. The body is read as the unstarred environment with that column in `\text{…}`.
    #[inline(never)]
    fn text_column_cases(&mut self, name: &str, col: usize) -> R<Node> {
        let raw = self.environment_body(name, col)?;
        let base = name.trim_end_matches('*');
        self.sub_reader(&format!("\\begin{{{base}}}{}\\end{{{base}}}", text_second_column(&raw)), self.variant)
    }

    /// amsmath's `\cfrac[l]` and `\cfrac[r]`: where the numerator goes; `[c]` or no option centers it.
    #[inline(never)]
    fn cfrac_alignment(&mut self) -> Option<char> {
        self.raw_optional().and_then(|a| match a.trim() {
            "l" => Some('l'),
            "r" => Some('r'),
            _ => None,
        })
    }

    /// `\setlength{\register}{length}` (or `\setlength\register{…}`), `\setlength` just read.
    /// `\arraycolsep` sets half the space between columns of the arrays and matrices after it; any
    /// other length is one contextEquation does not use (E-REC-1, ignored).
    #[inline(never)]
    fn set_length(&mut self, col: usize) -> R<()> {
        self.skip_spaces();
        let register = if self.current() == Some('\\') {
            self.pos += 1;
            format!("\\{}", self.read_command_name())
        } else {
            self.raw_text()?.trim().to_string()
        };
        let value = self.raw_text()?;
        match (register.as_str(), measure_em(&value)) {
            ("\\arraycolsep", Some(em)) if em.is_finite() && em >= 0.0 => {
                self.macros.array_colsep = Some(em.min(10.0));
                Ok(())
            }
            ("\\arraycolsep", _) => self.tolerate(rules::E_REC_9, col, format!("\\setlength{{\\arraycolsep}}: unrecognized measure: {value}"), "the spacing left as it was"),
            _ => self.tolerate(rules::E_REC_1, col, format!("\\setlength{{{register}}}: a length contextEquation does not use"), "ignored"),
        }
    }

    /// `\arraystretch` as the input defined it (`\def`, `\renewcommand`), or 1.
    fn array_stretch(&self) -> f32 {
        self.macros.commands.get("arraystretch").and_then(|m| m.body.trim().parse::<f32>().ok()).filter(|v| v.is_finite() && *v > 0.0).map_or(1.0, |v| v.min(10.0))
    }

    /// Rows and cells up to `\end{name}` (or up to the end of the text, when `name` is None).
    #[allow(clippy::type_complexity)]
    fn table_body(&mut self, name: Option<&str>, context: bool, col: usize) -> R<(Vec<Vec<Node>>, Vec<TableRule>, Vec<usize>)> {
        let label = name.unwrap_or("tabela");
        let mut rows: Vec<Vec<Node>> = Vec::new();
        let mut row: Vec<Node> = Vec::new();
        let mut cell: Vec<Node> = Vec::new();
        let mut context_row_start = context;
        let mut horizontals = Vec::new();
        let mut intertexts = Vec::new();
        loop {
            let (items, stop) = self.list()?;
            cell.extend(items);
            let body_end = match (&stop, name) {
                (Stop::End(end), Some(n)) if end == n => true,
                (Stop::ContextEnd(end), Some(n)) if context && end == n => true,
                (Stop::Eof, None) => true,
                _ => false,
            };
            let numbered = name.is_some_and(numbered_environment);
            if body_end {
                if !cell.is_empty() || !row.is_empty() {
                    row.push(Node::row(std::mem::take(&mut cell)).simplify());
                    rows.push(row);
                }
                if numbered {
                    self.refs.end_line();
                }
                break;
            }
            match stop {
                Stop::E if context_row_start && row.is_empty() && cell.is_empty() => context_row_start = false,
                Stop::E => {
                    row.push(Node::row(std::mem::take(&mut cell)).simplify());
                    context_row_start = false;
                }
                Stop::NewRow => {
                    row.push(Node::row(std::mem::take(&mut cell)).simplify());
                    rows.push(std::mem::take(&mut row));
                    context_row_start = context;
                    if numbered {
                        self.refs.end_line();
                    }
                }
                Stop::Hline(dashed) => horizontals.push(TableRule { row: rows.len(), from: 0, to: usize::MAX, dashed }),
                Stop::Cline(from, to, dashed) => horizontals.push(TableRule { row: rows.len(), from, to, dashed }),
                Stop::Intertext(t) => {
                    if !cell.is_empty() || !row.is_empty() {
                        row.push(Node::row(std::mem::take(&mut cell)).simplify());
                        rows.push(std::mem::take(&mut row));
                    }
                    intertexts.push(rows.len());
                    rows.push(vec![t]);
                }
                Stop::End(other) => {
                    self.tolerate(rules::E_REC_5, col, format!("\\begin{{{label}}} closed with \\end{{{other}}}"), "closed there")?;
                    if !cell.is_empty() || !row.is_empty() {
                        row.push(Node::row(std::mem::take(&mut cell)).simplify());
                        rows.push(std::mem::take(&mut row));
                    }
                    break;
                }
                Stop::ContextEnd(other) => {
                    self.tolerate(rules::E_REC_5, col, format!("\\start{label} closed with \\stop{other}"), "closed there")?;
                    if !cell.is_empty() || !row.is_empty() {
                        row.push(Node::row(std::mem::take(&mut cell)).simplify());
                        rows.push(std::mem::take(&mut row));
                    }
                    break;
                }
                Stop::Eof => {
                    self.tolerate(rules::E_REC_5, col, format!("\\begin{{{label}}} without \\end{{{label}}}"), "closed at the end")?;
                    if !cell.is_empty() || !row.is_empty() {
                        row.push(Node::row(std::mem::take(&mut cell)).simplify());
                        rows.push(std::mem::take(&mut row));
                    }
                    break;
                }
                Stop::CloseBrace => self.tolerate(rules::E_REC_4, col, format!("stray '}}' inside {label}"), "ignored")?,
                other => self.tolerate(rules::E_REC_5, col, format!("{other:?} inside {label}"), "ignored")?,
            }
        }
        Ok((rows, horizontals, intertexts))
    }

    /// Table read from a braced argument (`\substack{…}`, plain TeX's `\cases{…}`).
    fn argument_table(&mut self, shape: TableKind, col: usize) -> R<Node> {
        let raw = self.raw_text()?;
        let mut sub = Reader {
            c: raw.chars().collect(),
            pos: 0,
            locale: self.locale,
            registry: self.registry,
            macros: self.macros,
            refs: self.refs,
            variant: self.variant,
            strict: self.strict,
            font: self.font,
        };
        let (rows, horizontals, intertexts) = sub.table_body(None, false, col)?;
        Ok(Node::Table { shape, rows, columns: vec![], verticals: vec![], dashed: vec![], horizontals, between: vec![], intertexts, stretch: 1.0, colsep: 500 })
    }

    /// amscd commutative diagram: `A @>f>> B \\ @VgVV @VVhV \\ C @>>k> D`.
    fn cd_diagram(&mut self, raw: &str, col: usize) -> R<Node> {
        let c: Vec<char> = raw.chars().collect();
        // split into rows at \\ outside braces
        let mut raw_rows: Vec<String> = vec![String::new()];
        let mut level = 0;
        let mut i = 0;
        while i < c.len() {
            match c[i] {
                '{' => level += 1,
                '}' => level -= 1,
                '\\' if level == 0 && c.get(i + 1) == Some(&'\\') => {
                    raw_rows.push(String::new());
                    i += 2;
                    continue;
                }
                _ => {}
            }
            raw_rows.last_mut().unwrap().push(c[i]);
            i += 1;
        }
        let to = |c: &[char], i: &mut usize, end: char| -> String {
            let mut level = 0;
            let mut s = String::new();
            while *i < c.len() {
                let ch = c[*i];
                *i += 1;
                match ch {
                    '{' => level += 1,
                    '}' => level -= 1,
                    x if x == end && level == 0 => return s,
                    _ => {}
                }
                s.push(ch);
            }
            s
        };
        let mut rows = Vec::new();
        for row in raw_rows.iter().filter(|l| !l.trim().is_empty()) {
            let c: Vec<char> = row.chars().collect();
            let mut objects: Vec<String> = vec![String::new()];
            let mut arrows: Vec<Node> = Vec::new();
            let mut vertical = false;
            let mut i = 0;
            while i < c.len() {
                if c[i] == '@' && i + 1 < c.len() {
                    let kind = c[i + 1];
                    i += 2;
                    let label = |s: String, this: &mut Self| -> R<Option<Box<Node>>> {
                        if s.trim().is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(Box::new(this.sub_reader(&s, Variant::Italic)?)))
                        }
                    };
                    let arrow = match kind {
                        '>' | '<' => {
                            let a = to(&c, &mut i, kind);
                            let b = to(&c, &mut i, kind);
                            Node::Arrow { symbol: if kind == '>' { '→' } else { '←' }, above: label(a, self)?, below: label(b, self)? }
                        }
                        'V' | 'A' => {
                            vertical = true;
                            let a = to(&c, &mut i, kind);
                            let b = to(&c, &mut i, kind);
                            Node::VerticalArrow { symbol: if kind == 'V' { '↓' } else { '↑' }, left: label(a, self)?, right: label(b, self)? }
                        }
                        '=' => Node::Arrow { symbol: '=', above: None, below: None },
                        '|' => {
                            vertical = true;
                            Node::VerticalArrow { symbol: '‖', left: None, right: None }
                        }
                        '.' => Node::row(vec![]),
                        other => return self.error(col, format!("unknown diagram arrow: @{other}")),
                    };
                    arrows.push(arrow);
                    objects.push(String::new());
                } else {
                    objects.last_mut().unwrap().push(c[i]);
                    i += 1;
                }
            }
            let mut cells = Vec::new();
            if vertical {
                // row of vertical arrows: one per object column, with empty columns between them
                for (k, s) in arrows.into_iter().enumerate() {
                    if k > 0 {
                        cells.push(Node::row(vec![]));
                    }
                    cells.push(s);
                }
            } else {
                let mut arrows = arrows.into_iter();
                for (k, o) in objects.iter().enumerate() {
                    if k > 0 {
                        cells.push(arrows.next().unwrap_or(Node::row(vec![])));
                    }
                    cells.push(self.sub_reader(o, Variant::Italic)?);
                }
            }
            rows.push(cells);
        }
        Ok(Node::Table {
            shape: TableKind::Diagram,
            rows,
            columns: vec![],
            verticals: vec![],
            dashed: vec![],
            horizontals: vec![],
            between: vec![],
            intertexts: vec![],
            stretch: 1.0,
            colsep: 500,
        })
    }

    fn array_columns(&mut self, spec: &str, columns: &mut Vec<char>, verticals: &mut Vec<usize>, dashed: &mut Vec<usize>, between: &mut Vec<(usize, Node)>, col: usize) -> R<()> {
        let c: Vec<char> = spec.chars().collect();
        let mut i = 0;
        let group = |i: &mut usize| -> String {
            // content of {…} starting at c[*i] == '{'
            let mut level = 0;
            let mut s = String::new();
            while *i < c.len() {
                let ch = c[*i];
                *i += 1;
                match ch {
                    '{' => {
                        level += 1;
                        if level == 1 {
                            continue;
                        }
                    }
                    '}' => {
                        level -= 1;
                        if level == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                s.push(ch);
            }
            s
        };
        while i < c.len() {
            match c[i] {
                'l' | 'c' | 'r' => {
                    columns.push(c[i]);
                    i += 1;
                }
                'p' | 'm' | 'b' | 'X' => {
                    i += 1;
                    if c.get(i) == Some(&'{') {
                        group(&mut i);
                    }
                    columns.push('l');
                }
                '|' => {
                    verticals.push(columns.len());
                    i += 1;
                }
                // arydshln: a dashed rule
                ':' => {
                    dashed.push(columns.len());
                    i += 1;
                }
                '@' | '!' => {
                    i += 1;
                    let content = group(&mut i);
                    between.push((columns.len(), self.sub_reader(&content, Variant::Italic)?));
                }
                '*' => {
                    i += 1;
                    let n: usize = group(&mut i).trim().parse().unwrap_or(1);
                    let repeat_n_times = group(&mut i);
                    // *{50}{*{50}{…}} multiplies: the total column count is capped too
                    for _ in 0..n.min(50) {
                        if columns.len() >= MAX_COLUMNS {
                            break;
                        }
                        self.nested(|this| this.array_columns(&repeat_n_times, columns, verticals, dashed, between, col))?;
                    }
                }
                ' ' => i += 1,
                // anything else, the array package's >{…} and <{…} among it (#177)
                _ => i = self.other_column(&c, i, col)?,
            }
        }
        Ok(())
    }

    /// Content of `\text{…}`: running text, old-style accents and `$…$` snippets, as the parts of its node.
    fn text_parts(&mut self) -> R<Vec<TextPart>> {
        let col = self.column();
        let raw = self.raw_text()?;
        let c: Vec<char> = raw.chars().collect();
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut i = 0;
        // bold and small capitals: \textbf{…} closes at the group's brace; \bf and \bfseries last until the
        // end of the current group. Each entry is the group level and the marker that closes it.
        let mut level = 0usize;
        let mut open_until: Vec<(usize, char)> = Vec::new();
        // L-TXT-2: \boldmath sets the math that follows in bold, until the end of its group (and
        // into what a bold reader reads, as the text of `\rotatebox`)
        let mut bold_math: Option<usize> = (self.variant == Variant::BoldItalic).then_some(0);
        while i < c.len() {
            let math_variant = if bold_math.is_some() { Variant::BoldItalic } else { Variant::Italic };
            match c[i] {
                '$' => {
                    let end = c[i + 1..].iter().position(|&x| x == '$').map(|p| p + i + 1);
                    let Some(end) = end else {
                        self.tolerate(rules::E_REC_9, col, "unpaired '$' inside \\text", "written as a dollar sign")?;
                        current.push('$');
                        i += 1;
                        continue;
                    };
                    let snippet: String = c[i + 1..end].iter().collect();
                    split_text(&mut parts, &mut current, &open_until);
                    parts.push(TextPart::Math(self.sub_reader(&snippet, math_variant)?));
                    i = end + 1;
                }
                '~' => {
                    current.push('\u{00A0}');
                    i += 1;
                }
                '{' => {
                    level += 1;
                    i += 1;
                }
                '}' => {
                    while let Some(&(_, end)) = open_until.last().filter(|(at, _)| *at == level) {
                        open_until.pop();
                        current.push(end);
                    }
                    if bold_math == Some(level) {
                        bold_math = None;
                    }
                    level = level.saturating_sub(1);
                    i += 1;
                }
                '\\' if matches!(c.get(i + 1), Some('(') | Some('[')) => {
                    // \(…\) and \[…\] inside text, like $…$
                    let close = if c[i + 1] == '(' { ')' } else { ']' };
                    let end = (i + 2..c.len().saturating_sub(1)).find(|&k| c[k] == '\\' && c[k + 1] == close);
                    let Some(end) = end else {
                        self.tolerate(rules::E_REC_9, col, format!("unpaired \\{} inside \\text", c[i + 1]), "written as is")?;
                        current.push(c[i + 1]);
                        i += 2;
                        continue;
                    };
                    let snippet: String = c[i + 2..end].iter().collect();
                    split_text(&mut parts, &mut current, &open_until);
                    parts.push(TextPart::Math(self.sub_reader(&snippet, math_variant)?));
                    i = end + 2;
                }
                '\\' => {
                    i += 1;
                    let mark = c.get(i).copied().unwrap_or(' ');
                    if "'`^~\"=.".contains(mark) || (matches!(mark, 'c' | 'u' | 'v' | 'H' | 'r' | 'k' | 'b' | 'd' | 't') && c.get(i + 1).is_some_and(|x| *x == '{' || *x == ' ')) {
                        i += 1;
                        let mut braced = false;
                        while c.get(i) == Some(&' ') || c.get(i) == Some(&'{') {
                            braced |= c[i] == '{';
                            i += 1;
                        }
                        let letter = c.get(i).copied().unwrap_or(' ');
                        i += 1;
                        // L-TXT-6: an empty argument (`\^{}`) is the accent alone, not an accent on its `}` (#306)
                        if braced && letter == '}' {
                            match symbols::text_spacing(mark) {
                                Some(alone) => current.push(alone),
                                None => current.extend(['\u{00A0}'].into_iter().chain(symbols::text_combining(mark))),
                            }
                            continue;
                        }
                        if c.get(i) == Some(&'}') {
                            i += 1;
                        }
                        match symbols::text_accent(mark, letter) {
                            Some(composed) => current.push(composed),
                            // no precomposed form: the letter followed by the combining accent
                            None => current.extend([letter].into_iter().chain(symbols::text_combining(mark))),
                        }
                        continue;
                    }
                    let mut name = String::new();
                    while let Some(x) = c.get(i).filter(|x| x.is_ascii_alphabetic()) {
                        name.push(*x);
                        i += 1;
                    }
                    if name.is_empty() {
                        i += 1;
                        match mark {
                            '%' | '$' | '&' | '#' | '_' | '{' | '}' => current.push(mark),
                            '|' => current.push('‖'),
                            ',' | ';' | ':' | ' ' | '!' => current.push(if mark == '!' { '\u{200B}' } else { ' ' }),
                            '\\' => current.push(' '),
                            other => {
                                self.tolerate(rules::E_REC_1, col, format!("unsupported text command: \\{other}"), "written as is")?;
                                current.push(other);
                            }
                        }
                        continue;
                    }
                    while c.get(i) == Some(&' ') {
                        i += 1;
                    }
                    match name.as_str() {
                        "textbf" | "mathbf" => {
                            current.push(BOLD_START);
                            open_until.push((level + 1, BOLD_END));
                        }
                        "bf" | "bfseries" => {
                            current.push(BOLD_START);
                            open_until.push((level, BOLD_END));
                        }
                        "textsc" => {
                            current.push(SMALL_CAPS_START);
                            open_until.push((level + 1, SMALL_CAPS_END));
                        }
                        "sc" | "scshape" => {
                            current.push(SMALL_CAPS_START);
                            open_until.push((level, SMALL_CAPS_END));
                        }
                        "texttt" => {
                            current.push(MONO_START);
                            open_until.push((level + 1, MONO_END));
                        }
                        "tt" | "ttfamily" => {
                            current.push(MONO_START);
                            open_until.push((level, MONO_END));
                        }
                        // L-TXT-5: italic text
                        "textit" | "emph" => {
                            current.push(ITALIC_START);
                            open_until.push((level + 1, ITALIC_CORRECTED_END));
                        }
                        "it" | "itshape" | "em" => {
                            current.push(ITALIC_START);
                            open_until.push((level, ITALIC_END));
                        }
                        "textrm" | "textsf" | "textsl" | "text" | "mbox" | "textup" | "textnormal" | "rm" | "sf" | "small" | "scriptsize" | "footnotesize" | "tiny" | "large"
                        | "normalsize" | "mdseries" | "selectfont" => {}
                        // L-ENT-3: the size comes from whoever uses the SVG
                        "fontsize" => {
                            text_arguments(&c, &mut i, "{{");
                        }
                        "boldmath" => bold_math = Some(bold_math.map_or(level, |at| at.min(level))),
                        "unboldmath" => bold_math = None,
                        // L-TXT-2: commands that TeX sets as math inside text
                        "textsubscript" | "textsuperscript" | "ensuremath" => {
                            let tex = format!("\\{name}{}", text_arguments(&c, &mut i, "{"));
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        // L-CHE-1: mhchem typesets its formulas and quantities in text as in math
                        "ce" | "pu" | "cf" => {
                            let tex = format!("\\{name}{}", text_arguments(&c, &mut i, "{"));
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        // L-TXT-4: \textcolor colors its argument, \color the rest of its group; the
                        // colored text keeps the bold, small capitals and monospace around it
                        "textcolor" | "color" => {
                            let color = text_arguments(&c, &mut i, "[{");
                            let body = if name == "textcolor" {
                                let arg = text_arguments(&c, &mut i, "{");
                                arg[1..arg.len() - 1].to_string()
                            } else {
                                let end = group_end(&c, i);
                                let rest: String = c[i..end].iter().collect();
                                i = end;
                                rest
                            };
                            let mut styles: String = open_until
                                .iter()
                                .map(|&(_, end)| match end {
                                    BOLD_END => "\\bfseries ",
                                    SMALL_CAPS_END => "\\scshape ",
                                    ITALIC_END | ITALIC_CORRECTED_END => "\\itshape ",
                                    _ => "\\ttfamily ",
                                })
                                .collect();
                            if bold_math.is_some() {
                                styles.push_str("\\boldmath ");
                            }
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&format!("\\textcolor{color}{{\\text{{{styles}{body}}}}}"), math_variant)?));
                        }
                        // L-TXT-3: a text is a box of its natural width, where the fills take no room; a
                        // line break, which one line of text cannot hold, is a space, as `\\` is
                        "hfill" | "hfil" | "hss" => {}
                        "newline" | "linebreak" => {
                            text_arguments(&c, &mut i, "[");
                            current.push(' ');
                        }
                        // L-TXT-2: an environment that starts math, read as math up to its \end
                        "begin" if math_environment(&environment_name(&c, i)).is_some() => {
                            let env = environment_name(&c, i);
                            if math_environment(&env) == Some(false) {
                                self.tolerate(rules::E_REC_1, col, format!("math environment in text: {env}"), "read as math")?;
                            }
                            let end = environment_end(&c, i, &env);
                            let mut tex = format!("\\begin{}", c[i..end.unwrap_or(c.len())].iter().collect::<String>());
                            if end.is_none() {
                                self.tolerate(rules::E_REC_5, col, format!("\\begin{{{env}}} without \\end{{{env}}}"), "closed at the end")?;
                                tex.push_str(&format!("\\end{{{env}}}"));
                            }
                            i = end.unwrap_or(c.len());
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        // L-UNI-4: siunitx typesets its quantities in text as in math; L-BOX-1: graphicx's boxes
                        other if siunitx_in_text(other).or(graphicx_box(other)).is_some() => {
                            let tex = format!("\\{other}{}", text_arguments(&c, &mut i, siunitx_in_text(other).or(graphicx_box(other)).unwrap_or_default()));
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        // E-REC-1: a math command in text is an error in TeX; read as math
                        other if math_in_text(other).is_some() => {
                            self.tolerate(rules::E_REC_1, col, format!("math command in text: \\{other}"), "read as math")?;
                            let tex = format!("\\{other}{}", text_arguments(&c, &mut i, math_in_text(other).unwrap_or_default()));
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        // cross-references read as in math: a number, as text
                        "label" | "ref" | "eqref" | "cite" => {
                            let tex = format!("\\{name}{}", text_arguments(&c, &mut i, if name == "cite" { "[{" } else { "{" }));
                            let node = self.sub_reader(&tex, math_variant)?;
                            if let Node::Text { parts: inner } = node {
                                current.extend(inner.into_iter().filter_map(|p| if let TextPart::Text(t) = p { Some(t) } else { None }));
                            }
                        }
                        // L-TXT-3: space inside text, set by the math reader, which knows lengths and phantoms
                        "hspace" => {
                            let tex = format!("\\hspace{}", text_arguments(&c, &mut i, "*{"));
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&tex, math_variant)?));
                        }
                        "phantom" | "hphantom" | "vphantom" | "sout" | "textcircled" => {
                            // the argument stays text: the room it takes is the text's (#177: also what \sout strikes and \textcircled circles)
                            let arg = text_arguments(&c, &mut i, "{");
                            let arg = if arg.starts_with('{') { arg } else { format!("{{{arg}}}") };
                            split_text(&mut parts, &mut current, &open_until);
                            parts.push(TextPart::Math(self.sub_reader(&format!("\\{name}{{\\text{arg}}}"), math_variant)?));
                        }
                        "quad" => current.push('\u{2003}'),
                        "qquad" => current.push_str("\u{2003}\u{2003}"),
                        "ldots" | "dots" => current.push('…'),
                        "textdegree" | "degree" => current.push('°'),
                        "o" => current.push('ø'),
                        "ss" => current.push('ß'),
                        "aa" => current.push('å'),
                        "AA" => current.push('Å'),
                        "ae" => current.push('æ'),
                        "AE" => current.push('Æ'),
                        "oe" => current.push('œ'),
                        "OE" => current.push('Œ'),
                        "i" => current.push('ı'),
                        "j" => current.push('ȷ'),
                        "L" => current.push('Ł'),
                        "textdollar" => current.push('$'),
                        "textasciicircum" => current.push('^'),
                        "textunderscore" => current.push('_'),
                        "textbar" => current.push('|'),
                        "textbardbl" => current.push('‖'),
                        "textbraceleft" => current.push('{'),
                        "textbraceright" => current.push('}'),
                        "textdagger" => current.push('†'),
                        "textdaggerdbl" => current.push('‡'),
                        "textellipsis" => current.push('…'),
                        "textbullet" => current.push('•'),
                        "textperiodcentered" => current.push('·'),
                        "textparagraph" => current.push('¶'),
                        "textsection" => current.push('§'),
                        "textquoteleft" => current.push('‘'),
                        "textquoteright" => current.push('’'),
                        "textquotedblleft" => current.push('“'),
                        "textquotedblright" => current.push('”'),
                        "textendash" => current.push('–'),
                        "textemdash" => current.push('—'),
                        "textasciitilde" => current.push('~'),
                        "textbackslash" => current.push('\\'),
                        "times" => current.push('×'),
                        "LaTeX" => current.push_str("LaTeX"),
                        "TeX" => current.push_str("TeX"),
                        other => {
                            if let Some(symbols::Symbol { symbol, .. }) = symbols::command(other) {
                                current.push(symbol);
                            } else if let Some(g) = symbols::greek(other) {
                                current.push(g);
                            } else {
                                self.tolerate(rules::E_REC_1, col, format!("unsupported text command: \\{other}"), "written as is")?;
                                current.push('\\');
                                current.push_str(other);
                            }
                        }
                    }
                }
                // L-TXT-1: TeX's dash ligatures in text: -- and --- become – and —, except in typewriter
                // type, whose font has none (KaTeX takes them apart in \texttt too)
                '-' if c.get(i + 1) == Some(&'-') && self.variant != Variant::Monospace && !open_until.iter().any(|&(_, end)| end == MONO_END) => {
                    let em_dash = c.get(i + 2) == Some(&'-');
                    let dash = if em_dash { '—' } else { '–' };
                    self.registry.note(rules::L_TXT_1, format!("{} written as {dash}", if em_dash { "---" } else { "--" }));
                    current.push(dash);
                    i += if em_dash { 3 } else { 2 };
                }
                // L-TXT-1: TeX's quote ligatures in text: `` '' ` ' become “ ” ‘ ’
                q @ ('`' | '\'') => {
                    let double = c.get(i + 1) == Some(&q);
                    let typographic = match (q, double) {
                        ('`', true) => '“',
                        ('`', false) => '‘',
                        (_, true) => '”',
                        _ => '’',
                    };
                    self.registry.note(rules::L_TXT_1, format!("{} written as {typographic}", if double { format!("{q}{q}") } else { q.to_string() }));
                    current.push(typographic);
                    i += if double { 2 } else { 1 };
                }
                // #212: a line break or a tab is a space, as in TeX; with the spaces around it (a line's
                // indentation, a blank line) it is one space
                '\n' | '\r' | '\t' => {
                    let end = current.trim_end_matches(' ').len();
                    current.truncate(end);
                    current.push(' ');
                    while c.get(i).is_some_and(|x| matches!(x, ' ' | '\n' | '\r' | '\t')) {
                        i += 1;
                    }
                }
                other => {
                    current.push(other);
                    i += 1;
                }
            }
        }
        for (_, end) in open_until.into_iter().rev() {
            current.push(end);
        }
        if !current.is_empty() {
            parts.push(TextPart::Text(current));
        }
        Ok(parts)
    }

    /// A siunitx number; in tolerant mode, something else is drawn as math, with a warning.
    fn number_value(&mut self, value: &str, col: usize) -> R<Node> {
        Ok(self.uncertain_value(value, col)?.0)
    }

    /// A siunitx number, perhaps with an uncertainty, drawn as written: `1.23(4)` (compact, siunitx's
    /// default `uncertainty-mode`) or `1.23 \pm 0.04` (separate, `true` in the answer: a unit after it puts
    /// the value in parentheses, siunitx's default `separate-uncertainty-units = bracket`).
    fn uncertain_value(&mut self, value: &str, col: usize) -> R<(Node, bool)> {
        if let Some(n) = siunitx_number(value, true) {
            return Ok((n, false));
        }
        if let Some(uncertain) = siunitx_uncertainty(value) {
            return Ok(uncertain);
        }
        self.tolerate(rules::E_REC_9, col, format!("{value} is not a number"), "drawn as written")?;
        Ok((self.sub_reader(value, Variant::Italic)?, false))
    }

    /// `\ang{30}` → 30°; `\ang{12;30;15}` → 12°30′15″ (degrees, minutes and seconds; any of them may be empty).
    fn angle(&mut self, value: &str, col: usize) -> R<Node> {
        let mut children = Vec::new();
        for (part, mark) in value.split(';').zip(['°', '′', '″']) {
            if part.trim().is_empty() {
                continue;
            }
            children.push(self.number_value(part, col)?);
            children.push(Node::Operator { symbol: mark, class: Class::Ord });
        }
        if value.split(';').count() > 3 {
            self.tolerate(rules::E_REC_9, col, format!("\\ang{{{value}}}: more than degrees, minutes and seconds"), "the rest ignored")?;
        }
        Ok(Node::row(children))
    }

    /// One value of a range, list or product: a number, an angle or a quantity, by the command.
    fn listed_value(&mut self, command: &str, value: &str, unit: Option<&str>, col: usize) -> R<Node> {
        match unit {
            Some(u) => self.quantity(Some(value.to_string()), u, col),
            None if command.starts_with("ang") => self.angle(value, col),
            None => self.number_value(value, col),
        }
    }

    /// L-UNI-1: `\SI{9,8}{\meter\per\second\squared}` → 9,8 m/s² in pt-BR. The units declared with
    /// `\DeclareSIUnit`, the ones based on constants and `\highlight` come from siunitx.rs; so does
    /// `per-mode`, which only `\sisetup` changes from the slash.
    fn quantity(&mut self, value: Option<String>, unit: &str, col: usize) -> R<Node> {
        let unit = self.declared_units_written_out(unit);
        let unit_node = if unit.contains('\\') {
            // (symbol, exponent, prefix names, unit name, highlight color)
            let mut numerator: Vec<UnitEntry> = Vec::new();
            let mut denominator: Vec<UnitEntry> = Vec::new();
            let mut prefix = String::new();
            let mut prefix_names = String::new();
            let mut per = false;
            let mut last_in_den = false;
            let mut highlight = None;
            let mut raised: Option<i32> = None;
            for token in unit.split('\\').map(str::trim).filter(|t| !t.is_empty()) {
                let (name, remainder) = token.split_at(token.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(token.len()));
                match name {
                    "per" => per = true,
                    "squared" | "cubed" => {
                        let target = if last_in_den { denominator.last_mut() } else { numerator.last_mut() };
                        if let Some(u) = target {
                            u.1 *= if name == "squared" { 2 } else { 3 };
                        }
                    }
                    "square" | "cubic" => prefix.push(if name == "square" { '²' } else { '³' }),
                    "tothe" => {
                        let n: i32 = remainder.trim_matches(['{', '}', ' ']).parse().unwrap_or(1);
                        let target = if last_in_den { denominator.last_mut() } else { numerator.last_mut() };
                        if let Some(u) = target {
                            u.1 *= n;
                        }
                    }
                    // \highlight{color}: the next unit's symbol in that color
                    "highlight" => highlight = self.named_color(remainder.trim().trim_matches(['{', '}']).trim(), col)?,
                    // \raiseto{4}\metre: the prefix form of \tothe, for the next unit (m⁴)
                    "raiseto" => raised = remainder.trim_matches(['{', '}', ' ']).parse().ok(),
                    _ => {
                        if let Some(p) = symbols::prefix(name).filter(|_| !self.macros.siunitx.units.contains_key(name)) {
                            // \square\kilo\meter: the pending exponent still applies (km²)
                            prefix.insert_str(0, p);
                            prefix_names.insert_str(0, symbols::canonical_prefix(name));
                            continue;
                        }
                        // a declared unit comes first, even over one of siunitx's own names
                        let other = if self.macros.siunitx.units.contains_key(name) || symbols::unit(name).is_none() { self.other_unit_symbol(name)? } else { None };
                        let symbol = match (other, symbols::unit(name)) {
                            (Some(symbol), _) => symbol,
                            (None, Some(symbol)) => UnitSymbol::Text(symbol.to_string()),
                            (None, None) => {
                                self.tolerate(rules::E_REC_1, col, format!("unsupported unit: \\{name}"), "written by its name")?;
                                UnitSymbol::Text(name.to_string())
                            }
                        };
                        let exponent = if let Some(n) = raised.take() {
                            n
                        } else if prefix.contains('²') {
                            2
                        } else if prefix.contains('³') {
                            3
                        } else {
                            1
                        };
                        let written = prefix.trim_end_matches(['²', '³']).to_string();
                        let base = match symbol {
                            UnitSymbol::Text(t) => Node::Text { parts: vec![TextPart::Text(format!("{written}{t}"))] },
                            UnitSymbol::Math(math) if !written.is_empty() => Node::row(vec![Node::Text { parts: vec![TextPart::Text(written)] }, math]),
                            UnitSymbol::Math(math) => math,
                        };
                        prefix.clear();
                        let names = (std::mem::take(&mut prefix_names), symbols::canonical_unit(name).to_string());
                        let entry = (base, exponent, names.0, names.1, highlight.take());
                        if per {
                            denominator.push(entry);
                            last_in_den = true;
                            per = false;
                        } else {
                            numerator.push(entry);
                            last_in_den = false;
                        }
                    }
                }
            }
            // a \raiseto with no unit after it (\second\raiseto{2}): the unit before it, as \tothe
            if let Some((n, u)) = raised.zip(if last_in_den { denominator.last_mut() } else { numerator.last_mut() }) {
                u.1 *= n;
            }
            let mode = self.macros.siunitx.per_mode;
            let inter = self.macros.siunitx.inter_unit_product.clone();
            let product = self.si_product(inter)?;
            let mut children = joined_units(&numerator, false, &product);
            if !denominator.is_empty() {
                let one = || Node::Number { value: "1".into(), group: false, thousands: None, variant: Variant::Upright };
                match mode {
                    PerMode::Power => {
                        if !children.is_empty() {
                            children.push(product.clone());
                        }
                        children.extend(joined_units(&denominator, true, &product));
                    }
                    PerMode::Fraction => {
                        let num = if children.is_empty() { one() } else { Node::row(std::mem::take(&mut children)) };
                        let den = Node::row(joined_units(&denominator, false, &product));
                        children.push(Node::Fraction { numerator_align: None, num: Box::new(num), den: Box::new(den), style: FractionStyle::Auto, rule: true, thickness: None });
                    }
                    PerMode::Repeated => {
                        if children.is_empty() {
                            children.push(one());
                        }
                        for u in &denominator {
                            children.push(Node::Operator { symbol: '/', class: Class::Ord });
                            children.push(unit_node(u, false));
                        }
                    }
                    PerMode::Symbol => {
                        if children.is_empty() {
                            children.push(one());
                        }
                        children.push(Node::Operator { symbol: '/', class: Class::Ord });
                        let several = denominator.len() > 1;
                        if several {
                            children.push(Node::Operator { symbol: '(', class: Class::Open });
                        }
                        for (i, u) in denominator.iter().enumerate() {
                            if i > 0 {
                                children.push(Node::Operator { symbol: '·', class: Class::Ord });
                            }
                            children.push(unit_node(u, false));
                        }
                        if several {
                            children.push(Node::Operator { symbol: ')', class: Class::Close });
                        }
                    }
                }
            }
            let how = match mode {
                PerMode::Symbol => "with a slash for negative exponents",
                PerMode::Repeated => "with a slash before each unit of the denominator (per-mode=repeated-symbol)",
                PerMode::Power => "with negative exponents (per-mode=power)",
                PerMode::Fraction => "as a fraction (per-mode=fraction)",
            };
            self.registry.note(rules::L_UNI_1, format!("unit {unit} written {how}"));
            let names: Vec<String> = numerator.iter().chain(&denominator).map(|u| format!("{}{}", u.2, u.3)).collect();
            self.registry.note(rules::L_UNI_3, format!("units kept by name for the speech: {}", names.join(", ")));
            Node::row(children)
        } else {
            self.sub_reader(&unit, Variant::Upright)?
        };
        self.quantity_node(value, unit_node, col)
    }

    /// Value, thin space and unit, as one quantity.
    fn quantity_node(&mut self, value: Option<String>, unit_node: Node, col: usize) -> R<Node> {
        let value = match value {
            // (1.23 ± 0.04) kg: the value with its uncertainty in parentheses before the unit
            Some(v) => Some(match self.uncertain_value(&v, col)? {
                (node, true) => Node::row(vec![Node::Operator { symbol: '(', class: Class::Open }, node, Node::Operator { symbol: ')', class: Class::Close }]),
                (node, false) => node,
            }),
            None => None,
        };
        let text_value = value.as_ref().map(|v| match v {
            Node::Number { value, .. } => value.clone(),
            _ => String::new(),
        });
        let mut children = Vec::new();
        if let Some(v) = value {
            children.push(v);
            let product = self.macros.siunitx.quantity_product.clone();
            children.push(self.si_product(product)?);
        }
        children.push(unit_node);
        Ok(Node::Quantity { value: text_value.filter(|v| !v.is_empty()), unit: Box::new(Node::row(children)) })
    }

    /// `\ce{…}`, its backslash at `start`. An `&` or `\\` in it belongs to the alignment around it, as
    /// mhchem passes them on (#270): `\ce{A &-> B \\ C}` is read again as `\ce{A }&\ce{-> B }\\\ce{C}`, and
    /// nothing is returned this time.
    #[inline(never)]
    fn chemistry_command(&mut self, start: usize, col: usize) -> R<Option<Node>> {
        let raw = self.raw_text()?;
        let pieces = chemical_alignment_pieces(&raw);
        if pieces.len() == 1 {
            return self.chemistry(&raw, col).map(Some);
        }
        let mut text = String::new();
        for (piece, separator) in pieces {
            text.push_str(&format!("\\ce{{{piece}}}{separator}"));
        }
        self.inject(start, &text, col)?;
        Ok(None)
    }

    /// L-CHE-1: mhchem subset. Formulas with automatic subscripts, charges, isotopes, hydrates, bonds and
    /// coefficients (integer, decimal, fraction or `$…$`), arrows with labels, `\frac` and math commands.
    fn chemistry(&mut self, raw: &str, col: usize) -> R<Node> {
        let c: Vec<char> = raw.chars().collect();
        let mut children: Vec<Node> = Vec::new();
        let mut i = 0;
        // whether the last token was a formula or a word, for the word space before the next one
        let mut previous_entity = false;
        loop {
            let before = i;
            while c.get(i).is_some_and(|x| x.is_whitespace()) {
                i += 1;
            }
            if i >= c.len() {
                break;
            }
            // a mark around a piece (\htmlData, \class, \htmlId…): the piece is read as chemistry and
            // the mark goes around it, so an editor knows where each species, coefficient, state or
            // arrow is, and the drawing is the one without the mark
            if let Some((command, value, inner, len)) = chemical_mark(&c[i..]) {
                let spaced = i > before;
                i += len;
                // whether the piece is a formula or a word, read from its text without the marks inside
                let inner_chars: Vec<char> = chemical_unmarked(&inner).chars().collect();
                let mut k = 0;
                while inner_chars.get(k).is_some_and(|x| x.is_whitespace()) {
                    k += 1;
                }
                let entity = chemical_arrow(&inner_chars[k..]).is_none() && chemical_entity(&chemical_token(&inner_chars, &mut k));
                if let Some(previous) = children.last() {
                    // the spaces between pieces are those of the text: none where pieces touch
                    let coefficient = matches!(previous.clone().simplify(), Node::Identifier { .. });
                    let em = if !spaced {
                        0.0
                    } else if coefficient {
                        2.0 / 18.0
                    } else if previous_entity && entity {
                        CHEMICAL_WORD_SPACE
                    } else {
                        0.0
                    };
                    children.push(Node::Space { em });
                }
                previous_entity = entity;
                let node = self.chemistry(&inner, col)?.simplify();
                let (node, dropped) = html_attributes(&command, &value, node);
                if !dropped.is_empty() {
                    self.registry.warn(rules::E_REC_9, format!("column {col}: \\{command} in \\ce: unsafe or unsupported {}; dropped", dropped.join(", ")));
                }
                children.push(node);
                continue;
            }
            let arrow = chemical_arrow(&c[i..]);
            let entity = arrow.is_none() && chemical_entity(&chemical_token(&c, &mut i.clone()));
            if let Some(previous) = children.last() {
                // a variable coefficient written apart ($x$ H2O, n H2O) gets the small space a number gets;
                // two formulas or words written apart get a word space, mhchem's entitySkip (sodium chloride)
                let coefficient = matches!(previous.clone().simplify(), Node::Identifier { .. });
                let em = if coefficient {
                    2.0 / 18.0
                } else if previous_entity && entity {
                    CHEMICAL_WORD_SPACE
                } else {
                    0.0
                };
                children.push(Node::Space { em });
            }
            previous_entity = entity;
            if let Some((symbol, len)) = arrow {
                i += len;
                let above = bracketed(&c, &mut i);
                let below = if above.is_some() { bracketed(&c, &mut i) } else { None };
                if above.is_none() {
                    children.push(Node::Operator { symbol, class: Class::Rel });
                    continue;
                }
                let label = |t: Option<String>, this: &mut Self| -> R<Option<Box<Node>>> {
                    match t.filter(|t| !t.trim().is_empty()) {
                        // a label between braces is text, spaces and all (#268): ->[{text above}]
                        Some(t) if braced_whole(t.trim()).is_some() => {
                            let text = braced_whole(t.trim()).unwrap_or_default();
                            let mut parts = Vec::new();
                            for (k, piece) in text.trim().split('$').enumerate() {
                                if k % 2 == 1 {
                                    parts.push(TextPart::Math(this.sub_reader(piece, Variant::Italic)?));
                                } else if !piece.is_empty() {
                                    parts.push(TextPart::Text(piece.to_string()));
                                }
                            }
                            Ok(Some(Box::new(Node::Text { parts })))
                        }
                        Some(t) => Ok(Some(Box::new(this.chemistry(&t, col)?.simplify()))),
                        None => Ok(None),
                    }
                };
                let above = label(above, self)?;
                let below = label(below, self)?;
                self.registry.note(rules::L_CHE_1, "arrow with a label drawn stretched over it");
                children.push(Node::Arrow { symbol, above, below });
                continue;
            }
            let token = chemical_token(&c, &mut i);
            match token.as_str() {
                "+" => children.push(Node::Operator { symbol: '+', class: Class::Bin }),
                "=" => children.push(Node::Operator { symbol: '=', class: Class::Rel }),
                "^" | "v" => children.push(Node::Operator { symbol: if token == "^" { '↑' } else { '↓' }, class: Class::Ord }),
                "→" | "⇌" | "←" => children.push(Node::Operator { symbol: token.chars().next().unwrap_or('→'), class: Class::Rel }),
                // a variable as coefficient (x, n): italic, like in math
                t if t.len() == 1 && t.chars().all(|x| x.is_ascii_lowercase()) => children.push(Node::Identifier { name: t.to_string(), variant: Variant::Italic }),
                // a word (heat, catalyst, Water, sodium): upright text, whole
                t if chemical_word(t) => children.push(Node::Text { parts: vec![TextPart::Text(t.to_string())] }),
                formula => children.push(self.chemical_formula(formula, col)?),
            }
        }
        Ok(Node::row(children))
    }

    fn chemical_formula(&mut self, f: &str, col: usize) -> R<Node> {
        let c: Vec<char> = f.chars().collect();
        let mut children: Vec<Node> = Vec::new();
        let mut i = 0;
        let upright = |t: String| Node::Text { parts: vec![TextPart::Text(t)] };
        // a coefficient may come at the start and after the dot of a hydrate (CuSO4.5H2O)
        let mut coefficient_allowed = true;
        while i < c.len() {
            let ch = c[i];
            if coefficient_allowed && ch.is_ascii_digit() {
                coefficient_allowed = false;
                let coefficient = chemical_coefficient(&c, &mut i);
                children.push(coefficient);
                children.push(Node::Space { em: 2.0 / 18.0 });
                continue;
            }
            coefficient_allowed = false;
            if ch == '$' {
                let end = c[i + 1..].iter().position(|&x| x == '$').map_or(c.len(), |p| p + i + 1);
                let snippet: String = c[i + 1..end].iter().collect();
                children.push(self.sub_reader(&snippet, Variant::Italic)?);
                i = (end + 1).min(c.len());
            } else if ch == '\\' {
                let node = self.chemical_command(&c, &mut i, col)?;
                children.push(node);
            } else if ch.is_ascii_uppercase() {
                let mut el = ch.to_string();
                i += 1;
                while i < c.len() && c[i].is_ascii_lowercase() {
                    el.push(c[i]);
                    i += 1;
                }
                children.push(upright(el));
            } else if ch.is_ascii_digit() {
                let mut d = String::new();
                while i < c.len() && c[i].is_ascii_digit() {
                    d.push(c[i]);
                    i += 1;
                }
                attach_script(&mut children, None, Some(Node::Number { value: d, group: false, thousands: None, variant: Variant::Upright }));
            } else if ch == '^' || ch == '_' {
                i += 1;
                let payload = if c.get(i) == Some(&'{') {
                    braced(&c, &mut i)
                } else {
                    let mut p = String::new();
                    while i < c.len() && (c[i].is_ascii_alphanumeric() || (ch == '^' && matches!(c[i], '+' | '-' | '.'))) {
                        p.push(c[i]);
                        i += 1;
                        if ch == '_' && !p.chars().all(|x| x.is_ascii_digit()) {
                            break;
                        }
                    }
                    p
                };
                let node = if payload.contains('$') {
                    // mhchem's escape to math (#269): _{$\frac{6}{2}$}, ^{$n+$}
                    self.sub_reader(&payload.replace('$', ""), Variant::Italic)?
                } else if ch != '^' {
                    self.chemical_subscript(&payload)?
                } else if payload.contains('\\') {
                    self.chemical_superscript(&payload)?
                } else {
                    charge(&payload)
                };
                if ch == '^' {
                    attach_script(&mut children, Some(node), None);
                } else {
                    attach_script(&mut children, None, Some(node));
                }
            } else if matches!(ch, '+' | '-') && !children.is_empty() && (i + 1 == c.len() || matches!(c[i + 1], ']' | ')')) {
                // a charge at the end of the formula or before a closing bracket: Ag+, [H+], [A-]
                attach_script(&mut children, Some(charge(&ch.to_string())), None);
                i += 1;
            } else if let Some(state) = CHEMICAL_STATES.into_iter().find(|e| c[i..].iter().take(e.len()).copied().eq(e.chars())) {
                children.push(upright(state.to_string()));
                i += state.chars().count();
            } else if let Some((state, len)) = chemical_state(&c[i..]) {
                // mhchem's state of aggregation with more inside: (aq,$\infty$)
                let mut parts = Vec::new();
                for (k, piece) in state.split('$').enumerate() {
                    if k % 2 == 1 {
                        parts.push(TextPart::Math(self.sub_reader(piece, Variant::Italic)?));
                    } else if !piece.is_empty() {
                        parts.push(TextPart::Text(piece.to_string()));
                    }
                }
                children.push(Node::Text { parts });
                i += len;
            } else if matches!(ch, '.' | '*' | '·') && i > 0 {
                // hydrate or adduct: the dot is centered and a coefficient may follow
                children.push(Node::Operator { symbol: '·', class: Class::Ord });
                coefficient_allowed = true;
                i += 1;
            } else if ch.is_alphabetic() && ch.is_lowercase() {
                // e- (electron), loose lowercase letters: upright, like the elements
                let mut t = String::new();
                while i < c.len() && c[i].is_alphabetic() && c[i].is_lowercase() {
                    t.push(c[i]);
                    i += 1;
                }
                children.push(upright(t));
            } else {
                let class = match ch {
                    '(' | '[' => Class::Open,
                    ')' | ']' => Class::Close,
                    _ => Class::Ord,
                };
                // bonds: - single, = double, # triple
                if let Some(line) = match ch {
                    '-' => Some('−'),
                    '=' => Some('='),
                    '#' => Some('≡'),
                    _ => None,
                } {
                    children.push(bond(line));
                    i += 1;
                    continue;
                }
                let symbol = ch;
                if ch.is_alphabetic() || ch.is_ascii_punctuation() {
                    children.push(Node::Operator { symbol, class });
                } else {
                    self.tolerate(rules::E_REC_9, col, format!("unsupported character in \\ce: '{ch}'"), "drawn as is")?;
                    children.push(Node::Operator { symbol, class });
                }
                i += 1;
            }
        }
        Ok(Node::row(children))
    }

    /// #252: a superscript with a command in it (an isotope's `^{234\mathrm{m}}`): read as math, upright as
    /// mhchem sets it, so an unknown command warns (E-REC-1) and strict mode refuses it. Digits and upright
    /// letters alone are joined as the superscript written without the command (`^{234m}`) is.
    #[inline(never)]
    fn chemical_superscript(&mut self, payload: &str) -> R<Node> {
        let node = self.sub_reader(payload, Variant::Upright)?;
        let children = match &node {
            Node::Row { children } => children.as_slice(),
            other => std::slice::from_ref(other),
        };
        let mut plain = String::new();
        for child in children {
            match child {
                Node::Number { value, .. } => plain.push_str(value),
                Node::Identifier { name, variant: Variant::Upright } => plain.push_str(name),
                Node::Text { parts } if parts.iter().all(|p| matches!(p, TextPart::Text(_))) => {
                    plain.extend(parts.iter().filter_map(|p| if let TextPart::Text(t) = p { Some(t.as_str()) } else { None }));
                }
                _ => return Ok(node),
            }
        }
        Ok(charge(&plain))
    }

    /// Subscript in a formula: a number, a state (`_{(aq)}`) as text, anything else as math (`NO_x`).
    fn chemical_subscript(&mut self, payload: &str) -> R<Node> {
        if !payload.is_empty() && payload.chars().all(|x| x.is_ascii_digit()) {
            return Ok(Node::Number { value: payload.to_string(), group: false, thousands: None, variant: Variant::Upright });
        }
        if payload.starts_with('(') {
            return Ok(Node::Text { parts: vec![TextPart::Text(payload.to_string())] });
        }
        self.sub_reader(payload, Variant::Italic)
    }

    /// A command inside `\ce`: `\bond{…}`, `\frac{…}{…}` with formulas inside, or any math command with
    /// the braced arguments that follow it (`\Delta`, `\mu`, `\text{…}`).
    fn chemical_command(&mut self, c: &[char], i: &mut usize, col: usize) -> R<Node> {
        let start = *i;
        *i += 1;
        let mut name = String::new();
        while *i < c.len() && c[*i].is_ascii_alphabetic() {
            name.push(c[*i]);
            *i += 1;
        }
        if name.is_empty() && *i < c.len() {
            name.push(c[*i]);
            *i += 1;
        }
        let mut arguments = Vec::new();
        while c.get(*i) == Some(&'{') {
            arguments.push(braced(c, i));
        }
        match (name.as_str(), arguments.as_slice()) {
            ("bond", [b, ..]) => {
                // #267: the dashed bond, the bonds with more than one kind of line and of four dots are drawn by composition
                let composite = match b.trim() {
                    "~" => Some(crate::composite::BOND_DASHED),
                    "~-" => Some(crate::composite::BOND_DASHED_OVER_SINGLE),
                    "~=" | "~--" => Some(crate::composite::BOND_DASHED_OVER_DOUBLE),
                    "-~-" => Some(crate::composite::BOND_DASHED_BETWEEN),
                    "...." => Some(crate::composite::BOND_FOUR_DOTS),
                    _ => None,
                };
                if let Some(symbol) = composite {
                    return Ok(Node::Operator { symbol, class: Class::Ord });
                }
                let symbol = match b.trim() {
                    "-" | "1" => '−',
                    "=" | "2" => '=',
                    "#" | "3" => '≡',
                    "..." => '⋯',
                    "->" => '→',
                    "<-" => '←',
                    other => {
                        self.tolerate(rules::E_REC_9, col, format!("unknown bond: \\bond{{{other}}}"), "drawn as a single bond")?;
                        '−'
                    }
                };
                Ok(bond(symbol))
            }
            ("frac", [a, b, ..]) => {
                let num = self.chemistry(a, col)?.simplify();
                let den = self.chemistry(b, col)?.simplify();
                Ok(Node::Fraction { numerator_align: None, num: Box::new(num), den: Box::new(den), style: FractionStyle::Auto, rule: true, thickness: None })
            }
            _ => {
                let raw: String = c[start..*i].iter().collect();
                self.sub_reader(&raw, Variant::Italic)
            }
        }
    }
}

/// The pieces of a `\ce` argument between the `&` and `\\` that belong to the alignment around it (#270),
/// each with the separator after it (the last one has none): outside braces and `$…$`; `\&` is a
/// character. Without them, one piece, the whole argument.
fn chemical_alignment_pieces(raw: &str) -> Vec<(String, String)> {
    let c: Vec<char> = raw.chars().collect();
    let mut pieces = Vec::new();
    let mut piece = String::new();
    let (mut level, mut math) = (0i32, false);
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let top = level == 0 && !math;
        match ch {
            '\\' if top && c.get(i + 1) == Some(&'\\') => {
                pieces.push((std::mem::take(&mut piece), "\\\\".to_string()));
                i += 2;
                continue;
            }
            '\\' => {
                // a control sequence or an escaped character stays whole: \&, \{, \bond
                piece.push(ch);
                if let Some(&next) = c.get(i + 1) {
                    piece.push(next);
                    i += 1;
                }
            }
            '&' if top => pieces.push((std::mem::take(&mut piece), "&".to_string())),
            '{' => {
                level += 1;
                piece.push(ch);
            }
            '}' => {
                level -= 1;
                piece.push(ch);
            }
            '$' => {
                math = !math;
                piece.push(ch);
            }
            _ => piece.push(ch),
        }
        i += 1;
    }
    pieces.push((piece, String::new()));
    pieces
}

/// The space mhchem puts between two formulas or words written apart: its `entitySkip`, a `~`.
const CHEMICAL_WORD_SPACE: f32 = 0.333;

/// The states of aggregation `\ce` reads as upright text wherever they are.
const CHEMICAL_STATES: [&str; 6] = ["(aq)", "(s)", "(l)", "(g)", "(sol)", "(cr)"];

/// A token of `\ce` that is a word, kept whole as upright text: letters only, at least two, and either
/// all lowercase (heat, sodium) or a capital followed by lowercase letters that is not an element
/// symbol (Water, Hydrogen; Na and Co stay elements). mhchem draws any run of letters upright as
/// written, so the drawing is the same; this decides how the tree keeps it (spec/chemistry.md).
fn chemical_word(t: &str) -> bool {
    let mut letters = t.chars();
    let Some(first) = letters.next() else { return false };
    let rest: Vec<char> = letters.collect();
    if rest.is_empty() || !first.is_alphabetic() || !rest.iter().all(|x| x.is_alphabetic() && x.is_lowercase()) {
        return false;
    }
    first.is_lowercase() || !symbols::ELEMENTS.contains(&t)
}

/// A token of `\ce` that is a formula or a word, which mhchem separates from the next one by a word
/// space when they are written apart: it has a letter, and it is not an operator, a bond (`\bond{…}`),
/// a coefficient (`n`, `$x$`) or a state of aggregation standing alone (`(g)`).
fn chemical_entity(t: &str) -> bool {
    if t.starts_with("\\bond") {
        return false;
    }
    let c: Vec<char> = t.chars().collect();
    let coefficient = (c.len() == 1 && c[0].is_ascii_lowercase()) || (c.len() > 1 && t.starts_with('$') && t.ends_with('$'));
    let state = CHEMICAL_STATES.contains(&t) || chemical_state(&c).is_some_and(|(_, len)| len == c.len());
    c.iter().any(|x| x.is_alphabetic()) && !coefficient && !state
}

/// mhchem's state of aggregation with more inside, `(aq,$\infty$)`: a parenthesis, one to three
/// lowercase letters and a comma, up to the closing parenthesis, at the end of the formula or before a
/// space or `,;)]}` (mhchem's pattern `state of aggregation $`). Its text and its length.
fn chemical_state(c: &[char]) -> Option<(String, usize)> {
    if c.first() != Some(&'(') {
        return None;
    }
    let letters = c[1..].iter().take_while(|x| x.is_ascii_lowercase()).count();
    if !(1..=3).contains(&letters) || c.get(1 + letters) != Some(&',') {
        return None;
    }
    let close = c.iter().position(|&x| x == ')')?;
    if c.get(close + 1).is_some_and(|x| !x.is_whitespace() && !",;)]}".contains(*x)) {
        return None;
    }
    Some((c[..=close].iter().collect(), close + 1))
}

/// A chemical bond: drawn as its line, but it is not a minus, an equals or a congruence.
fn bond(symbol: char) -> Node {
    Node::Text { parts: vec![TextPart::Text(symbol.to_string())] }
}

/// mhchem arrows, the longest first: (drawn symbol, length in the input).
fn chemical_arrow(c: &[char]) -> Option<(char, usize)> {
    const ARROWS: [(&str, char); 7] = [("<=>>", '⥂'), ("<<=>", '⥄'), ("<-->", '⇄'), ("<=>", '⇌'), ("<->", '↔'), ("->", '→'), ("<-", '←')];
    ARROWS.iter().find(|(a, _)| c.iter().take(a.len()).copied().eq(a.chars())).map(|(a, s)| (*s, a.len()))
}

/// `[…]` right after an arrow (its label), honoring braces and `$…$`.
fn bracketed(c: &[char], i: &mut usize) -> Option<String> {
    if c.get(*i) != Some(&'[') {
        return None;
    }
    let mut level = 0;
    let mut math = false;
    let mut s = String::new();
    let mut k = *i + 1;
    while k < c.len() {
        match c[k] {
            '$' => math = !math,
            '{' => level += 1,
            '}' => level -= 1,
            ']' if level == 0 && !math => {
                *i = k + 1;
                return Some(s);
            }
            _ => {}
        }
        s.push(c[k]);
        k += 1;
    }
    None
}

/// The content of `t` when one pair of braces holds all of it (`{text above}`), else `None`.
fn braced_whole(t: &str) -> Option<String> {
    let c: Vec<char> = t.chars().collect();
    if c.first() != Some(&'{') {
        return None;
    }
    let mut i = 0;
    let inner = braced(&c, &mut i);
    (i == c.len() && c.last() == Some(&'}')).then_some(inner)
}

/// The text of a piece of `\ce` without the marks in it (`\htmlData{m=a}{Zn}(s)` → `Zn(s)`).
fn chemical_unmarked(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if let Some((_, _, inner, len)) = chemical_mark(&c[i..]) {
            out.push_str(&chemical_unmarked(&inner));
            i += len;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

/// A mark at the start of `c` inside `\ce`: `\htmlData{value}{inner}` (or `\class`, `\htmlClass`,
/// `\htmlId`, `\cssId`): the command, the value, the inner text and how many characters it all takes.
fn chemical_mark(c: &[char]) -> Option<(String, String, String, usize)> {
    if c.first() != Some(&'\\') {
        return None;
    }
    let name: String = c[1..].iter().take_while(|x| x.is_ascii_alphabetic()).collect();
    if !matches!(name.as_str(), "htmlData" | "class" | "htmlClass" | "htmlId" | "cssId") {
        return None;
    }
    let mut i = 1 + name.len();
    if c.get(i) != Some(&'{') {
        return None;
    }
    let value = braced(c, &mut i);
    if c.get(i) != Some(&'{') {
        return None;
    }
    let inner = braced(c, &mut i);
    Some((name, value, inner, i))
}

/// Content of `{…}` starting at `c[*i] == '{'`, without the outer braces.
fn braced(c: &[char], i: &mut usize) -> String {
    let mut level = 0;
    let mut s = String::new();
    while *i < c.len() {
        let ch = c[*i];
        *i += 1;
        match ch {
            '{' => {
                level += 1;
                if level == 1 {
                    continue;
                }
            }
            '}' => {
                level -= 1;
                if level == 0 {
                    break;
                }
            }
            _ => {}
        }
        s.push(ch);
    }
    s
}

/// A token of `\ce`: up to a space outside braces and `$…$`.
fn chemical_token(c: &[char], i: &mut usize) -> String {
    let mut level = 0;
    let mut math = false;
    let mut s = String::new();
    while *i < c.len() {
        let ch = c[*i];
        if ch.is_whitespace() && level == 0 && !math {
            break;
        }
        match ch {
            '$' => math = !math,
            '{' => level += 1,
            '}' => level -= 1,
            _ => {}
        }
        s.push(ch);
        *i += 1;
    }
    s
}

/// Stoichiometric coefficient: 2, 0.5, 1/2 (as a small fraction).
fn chemical_coefficient(c: &[char], i: &mut usize) -> Node {
    let digits = |i: &mut usize| -> String {
        let mut d = String::new();
        while *i < c.len() && c[*i].is_ascii_digit() {
            d.push(c[*i]);
            *i += 1;
        }
        d
    };
    let mut value = digits(i);
    if c.get(*i) == Some(&'.') && c.get(*i + 1).is_some_and(char::is_ascii_digit) {
        *i += 1;
        value.push('.');
        value.push_str(&digits(i));
    } else if c.get(*i) == Some(&'/') && c.get(*i + 1).is_some_and(char::is_ascii_digit) {
        *i += 1;
        let den = digits(i);
        let number = |v: String| Box::new(Node::Number { value: v, group: false, thousands: None, variant: Variant::Upright });
        return Node::Fraction { numerator_align: None, num: number(value), den: number(den), style: FractionStyle::Row, rule: true, thickness: None };
    }
    Node::Number { value, group: false, thousands: None, variant: Variant::Upright }
}

/// Charge or oxidation state: `2-` → 2−, `.-` → •− (radical), `II` → II.
fn charge(payload: &str) -> Node {
    if !payload.is_empty() && payload.chars().all(|x| x.is_ascii_digit()) {
        return Node::Number { value: payload.to_string(), group: false, thousands: None, variant: Variant::Upright };
    }
    let text: String = payload
        .chars()
        .map(|x| match x {
            '-' => '−',
            '.' | '*' => '•',
            other => other,
        })
        .collect();
    Node::Text { parts: vec![TextPart::Text(text)] }
}

/// Attaches a superscript and/or subscript to the last node of the formula; at the start of the formula
/// (an isotope's `^{14}_{6}C`) they go on an empty base.
fn attach_script(children: &mut Vec<Node>, sup: Option<Node>, sub: Option<Node>) {
    // after a coefficient (3^{1}_{0}n) the scripts start a new base, not the space
    if matches!(children.last(), Some(Node::Space { .. })) {
        children.push(Node::row(vec![]));
    }
    let last_item = children.pop().unwrap_or(Node::row(vec![]));
    children.push(match last_item {
        Node::Scripts { base, sup: None, sub: s } if sup.is_some() => Node::Scripts { base, sup: sup.map(Box::new), sub: s.or(sub.map(Box::new)) },
        Node::Scripts { base, sup: s, sub: None } if sub.is_some() => Node::Scripts { base, sup: s, sub: sub.map(Box::new) },
        other => Node::Scripts { base: Box::new(other), sup: sup.map(Box::new), sub: sub.map(Box::new) },
    });
}

/// tensor package indices: `^\mu_\nu^\rho` → `^{\mu}{}_{\nu}{}^{\rho}`, each change of side on a new empty
/// base so the indices stagger. Before the base (`pre`), the empty base comes first.
fn staggered_indices(raw: &str, pre: bool) -> String {
    let c: Vec<char> = raw.chars().collect();
    let mut groups: Vec<(char, String)> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let side = c[i];
        i += 1;
        if !matches!(side, '^' | '_') {
            continue;
        }
        while c.get(i) == Some(&' ') {
            i += 1;
        }
        let index = match c.get(i) {
            Some('{') => braced(&c, &mut i),
            Some('\\') => {
                let mut s = String::from('\\');
                i += 1;
                while i < c.len() && c[i].is_ascii_alphabetic() {
                    s.push(c[i]);
                    i += 1;
                }
                if s.len() == 1 && i < c.len() {
                    s.push(c[i]);
                    i += 1;
                }
                s
            }
            Some(x) => {
                i += 1;
                x.to_string()
            }
            None => String::new(),
        };
        match groups.last_mut() {
            Some((last, content)) if *last == side => {
                content.push(' ');
                content.push_str(&index);
            }
            _ => groups.push((side, index)),
        }
    }
    let scripts: Vec<String> = groups.iter().map(|(side, content)| format!("{side}{{{content}}}")).collect();
    let joined = scripts.join("{}");
    if pre {
        format!("{{}}{joined}")
    } else {
        joined
    }
}

/// mhchem's `\pu` unit to math: `J mol^-1` → `J\,mol^{-1}`, `.` and `*` between units → `\cdot`.
/// The commands of `physics_expansion`, besides the long names of `physics_long_name`.
const PHYSICS: &[&str] = &[
    "mqty",
    "matrixquantity",
    "smqty",
    "smallmatrixquantity",
    "pmqty",
    "Pmqty",
    "bmqty",
    "vmqty",
    "mdet",
    "matrixdeterminant",
    "spmqty",
    "sPmqty",
    "sbmqty",
    "svmqty",
    "smdet",
    "smallmatrixdeterminant",
    "imat",
    "identitymatrix",
    "xmat",
    "xmatrix",
    "zmat",
    "zeromatrix",
    "paulimatrix",
    "pmat",
    "paulixmatrix",
    "pauliymatrix",
    "paulizmatrix",
    "dmat",
    "diagonalmatrix",
    "admat",
    "antidiagonalmatrix",
    "qq",
    "qqtext",
    "qc",
    "qcomma",
    "qcc",
    "qif",
    "qthen",
    "qelse",
    "qotherwise",
    "qunless",
    "qgiven",
    "qusing",
    "qassume",
    "qsince",
    "qlet",
    "qfor",
    "qall",
    "qeven",
    "qodd",
    "qinteger",
    "qand",
    "qor",
    "qas",
    "qin",
    "var",
    "variation",
    "pv",
    "principalvalue",
    "PV",
    "op",
    "vev",
    "flatfrac",
    "Trace",
    "Residue",
    "asin",
    "acos",
    "atan",
    "acsc",
    "asec",
    "acot",
    "asine",
    "acosine",
    "atangent",
    "acosecant",
    "asecant",
    "acotangent",
    "vnabla",
    "gradientnabla",
    "diffd",
    "real",
    "imaginary",
];

/// physics' Pauli matrices σx, σy, σz, as the package writes their contents.
const PAULI: [&str; 3] = ["0&1\\\\1&0", "0&-i\\\\i&0", "1&0\\\\0&-1"];

/// A physics matrix: `shape` is `(`, `[`, `|`, `P` (\lgroup…\rgroup) or `{` (none); `small` for
/// `\smqty` and its family, a `smallmatrix` between `\left…\right` delimiters, as the package's
/// `\spmqty` is `\pqty{\begin{smallmatrix}…}`.
fn physics_matrix(small: bool, shape: char, body: &str) -> String {
    let (env, open, close) = match (small, shape) {
        (false, '(') => ("pmatrix", "", ""),
        (false, '[') => ("bmatrix", "", ""),
        (false, '|') => ("vmatrix", "", ""),
        (false, 'P') => ("matrix", "\\lgroup", "\\rgroup"),
        (false, _) => ("matrix", "", ""),
        (true, '(') => ("smallmatrix", "(", ")"),
        (true, '[') => ("smallmatrix", "[", "]"),
        (true, '|') => ("smallmatrix", "|", "|"),
        (true, 'P') => ("smallmatrix", "\\lgroup", "\\rgroup"),
        (true, _) => ("smallmatrix", "", ""),
    };
    let table = format!("\\begin{{{env}}}{body}\\end{{{env}}}");
    if open.is_empty() {
        table
    } else {
        format!("\\left{open} {table} \\right{close}")
    }
}

/// The contents of a generated matrix (`\imat`, `\xmat`, `\dmat`…): `rows` × `columns` entries, by
/// row and column from 0, in `&` and `\\`.
fn generated_matrix(rows: usize, columns: usize, entry: impl Fn(usize, usize) -> String) -> String {
    (0..rows).map(|i| (0..columns).map(|j| entry(i, j)).collect::<Vec<_>>().join("&")).collect::<Vec<_>>().join("\\\\")
}

/// `raw` split at each `separator` outside braces, each part trimmed.
fn split_top_level(raw: &str, separator: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut level = 0usize;
    for c in raw.chars() {
        match c {
            '{' => level += 1,
            '}' => level = level.saturating_sub(1),
            c if c == separator && level == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        parts.last_mut().unwrap().push(c);
    }
    parts.into_iter().map(|p| p.trim().to_string()).collect()
}

/// physics' long names of the elementary functions (`\sine` is `\sin`), which the package keeps when
/// it redefines the short ones.
fn physics_long_name(long: &str) -> Option<&'static str> {
    Some(match long {
        "sine" => "sin",
        "cosine" => "cos",
        "tangent" => "tan",
        "cosecant" => "csc",
        "secant" => "sec",
        "cotangent" => "cot",
        "hypsine" => "sinh",
        "hypcosine" => "cosh",
        "hyptangent" => "tanh",
        "hypcosecant" => "csch",
        "hypsecant" => "sech",
        "hypcotangent" => "coth",
        "arcsine" => "arcsin",
        "arccosine" => "arccos",
        "arctangent" => "arctan",
        "arccosecant" => "arccsc",
        "arcsecant" => "arcsec",
        "arccotangent" => "arccot",
        "exponential" => "exp",
        "logarithm" => "log",
        "naturallogarithm" => "ln",
        "determinant" => "det",
        "Probability" => "Pr",
        _ => return None,
    })
}

fn mhchem_unit(unit: &str) -> String {
    // `//` is a fraction bar: mhchem.js' `pu` state reads the unit before it and after it apart and
    // writes them as a `pu-frac`, \frac in text style (\mathchoice{\textstyle\frac{…}{…}}…)
    if let Some((numerator, denominator)) = unit.split_once("//") {
        return format!("\\tfrac{{{}}}{{{}}}", mhchem_unit(numerator.trim()), mhchem_unit(denominator.trim()));
    }
    let c: Vec<char> = unit.chars().collect();
    let mut s = String::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '^' => {
                let mut exponent = String::new();
                i += 1;
                while i < c.len() && (c[i].is_ascii_digit() || (c[i] == '-' && exponent.is_empty())) {
                    exponent.push(c[i]);
                    i += 1;
                }
                s.push_str(&format!("^{{{exponent}}}"));
                continue;
            }
            '.' | '*' => s.push_str("\\cdot "),
            ' ' => s.push_str("\\,"),
            other => s.push(other),
        }
        i += 1;
    }
    s
}

fn set_limits(node: &mut Node, wanted: Option<bool>) {
    match node {
        Node::LargeOperator { limits, .. } => *limits = wanted,
        Node::Function { limits, .. } => *limits = wanted.unwrap_or(*limits),
        Node::Scripts { base, .. } => set_limits(base, wanted),
        Node::ForcedClass { base, .. } => set_limits(base, wanted),
        _ => {}
    }
}

fn natural_class(node: &Node) -> Class {
    match node {
        Node::Operator { class, .. } => *class,
        Node::LargeOperator { .. } | Node::Function { .. } => Class::Op,
        // \overset{a}{\underset{b}{=}}: the class the inner command gave
        Node::ForcedClass { class, .. } => *class,
        _ => Class::Ord,
    }
}

fn negate(c: char) -> Option<char> {
    Some(match c {
        '=' => '≠',
        '∈' => '∉',
        '⊂' => '⊄',
        '⊃' => '⊅',
        '⊆' => '⊈',
        '⊇' => '⊉',
        '≡' => '≢',
        '<' => '≮',
        '>' => '≯',
        '≤' => '≰',
        '≥' => '≱',
        '∼' => '≁',
        '≈' => '≉',
        '∃' => '∄',
        '∣' => '∤',
        '∥' => '∦',
        '→' => '↛',
        _ => return None,
    })
}

/// The text so far as a part of its own, before math inside it: each part is set apart, so the bold,
/// small capitals and monospace still open (`open`, by their closing markers) are closed at its end
/// and opened again in the text that follows (`\textbf{a $x$ b}`: b is bold too). Text of markers
/// alone stays, to go on after the math.
fn split_text(parts: &mut Vec<TextPart>, current: &mut String, open: &[(usize, char)]) {
    if current.chars().all(|c| MARKERS.contains(&c)) {
        return;
    }
    let mut done = std::mem::take(current);
    done.extend(open.iter().rev().map(|&(_, end)| end));
    parts.push(TextPart::Text(done));
    current.extend(open.iter().map(|&(_, end)| match end {
        BOLD_END => BOLD_START,
        SMALL_CAPS_END => SMALL_CAPS_START,
        ITALIC_END | ITALIC_CORRECTED_END => ITALIC_START,
        _ => MONO_START,
    }));
}

/// The arguments of a command inside text, as written, to read them again with the command: `spec` has
/// The body of a `cases*` with its second column in `\text{…}`: the text after the first `&` of each row, up
/// to the row's `\\` (or a further `&`), outside braces and nested environments. Spaces at the ends of the
/// text go, as TeX skips them in an alignment cell.
fn text_second_column(raw: &str) -> String {
    let c: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let (mut braces, mut environments, mut in_text, mut i) = (0usize, 0usize, false, 0);
    let close = |out: &mut String, in_text: &mut bool| {
        if *in_text {
            out.truncate(out.trim_end().len());
            out.push('}');
            *in_text = false;
        }
    };
    while i < c.len() {
        let top = braces == 0 && environments == 0;
        match c[i] {
            '\\' if top && c.get(i + 1) == Some(&'\\') => {
                close(&mut out, &mut in_text);
                out.push_str("\\\\");
                i += 2;
                continue;
            }
            '\\' => {
                let name: String = c[i + 1..].iter().take_while(|c| c.is_ascii_alphabetic()).collect();
                match name.as_str() {
                    "begin" => environments += 1,
                    "end" => environments = environments.saturating_sub(1),
                    "cr" if top => close(&mut out, &mut in_text),
                    _ => {}
                }
                // the command, or the character after the backslash (\{, \&)
                let n = name.chars().count().max(1);
                out.extend(&c[i..(i + 1 + n).min(c.len())]);
                i += 1 + n;
                continue;
            }
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            '&' if top && in_text => close(&mut out, &mut in_text),
            '&' if top => {
                out.push_str("&\\text{");
                in_text = true;
                i += 1;
                while c.get(i).is_some_and(|c| c.is_whitespace()) {
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        out.push(c[i]);
        i += 1;
    }
    close(&mut out, &mut in_text);
    out
}

/// `{` for each mandatory argument, `[` for each optional one and `*` for an optional star, in order. A
/// mandatory argument without braces is one character or one command, as in TeX.
fn text_arguments(c: &[char], i: &mut usize, spec: &str) -> String {
    let mut out = String::new();
    for kind in spec.chars() {
        let mut k = *i;
        while c.get(k) == Some(&' ') {
            k += 1;
        }
        match kind {
            '*' => {
                if c.get(k) == Some(&'*') {
                    out.push('*');
                    *i = k + 1;
                }
            }
            '[' => {
                if c.get(k) != Some(&'[') {
                    continue;
                }
                let mut depth = 0usize;
                let end = (k + 1..c.len()).find(|&j| match c[j] {
                    '{' => {
                        depth += 1;
                        false
                    }
                    '}' => {
                        depth = depth.saturating_sub(1);
                        false
                    }
                    ']' => depth == 0,
                    _ => false,
                });
                if let Some(end) = end {
                    out.extend(&c[k..=end]);
                    *i = end + 1;
                }
            }
            _ => match c.get(k) {
                Some('{') => {
                    let mut depth = 0usize;
                    let mut j = k;
                    while j < c.len() {
                        match c[j] {
                            '\\' => j += 1,
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    let end = j.min(c.len().saturating_sub(1));
                    out.extend(&c[k..=end]);
                    // an unclosed group is closed at the end, as raw_text does
                    if j >= c.len() {
                        out.push('}');
                    }
                    *i = end + 1;
                }
                Some('\\') => {
                    let mut j = k + 1;
                    while c.get(j).is_some_and(char::is_ascii_alphabetic) {
                        j += 1;
                    }
                    let end = j.max(k + 2).min(c.len());
                    out.push('{');
                    out.extend(&c[k..end]);
                    out.push('}');
                    *i = end;
                }
                Some(&x) => {
                    out.push('{');
                    out.push(x);
                    out.push('}');
                    *i = k + 1;
                }
                None => {}
            },
        }
    }
    out
}

/// The siunitx commands, which work in text as in math, with their arguments (as for `text_arguments`).
fn siunitx_in_text(name: &str) -> Option<&'static str> {
    Some(match name {
        "SI" | "qty" | "SIlist" | "qtylist" | "SIproduct" | "qtyproduct" => "[{{",
        "si" | "unit" | "ang" | "numlist" | "anglist" | "numproduct" => "[{",
        "num" => "[{[",
        "complexnum" => "[{",
        "complexqty" => "[{{",
        "tablenum" => "[{",
        "SIrange" | "qtyrange" => "[{{{",
        "numrange" | "angrange" => "[{{",
        _ => return None,
    })
}

/// graphicx's box commands, with their arguments (as for `text_arguments`); their content is text.
fn graphicx_box(name: &str) -> Option<&'static str> {
    Some(match name {
        "scalebox" => "{[{",
        "reflectbox" => "{",
        "resizebox" => "*{{{",
        "rotatebox" => "[{{",
        "raisebox" => "{[[{",
        _ => return None,
    })
}

/// Math commands that authors write in text, with their arguments (as for `text_arguments`).
fn math_in_text(name: &str) -> Option<&'static str> {
    Some(match name {
        "mathrm" | "mathit" | "mathsf" | "mathtt" | "mathcal" | "mathbb" | "mathfrak" | "mathscr" | "mathbfit" | "mathnormal" | "boldsymbol" | "bm" | "mathbold" => "{",
        "frac" | "dfrac" | "tfrac" => "{{",
        "sqrt" => "[{",
        "operatorname" => "*{",
        _ => return None,
    })
}

/// amsmath's `multline`: the first line at the left, the last at the right, the ones between
/// centered, except a line in `\shoveleft{…}` or `\shoveright{…}` (already a one-column
/// `Multicolumn` with its alignment). One line alone stays as it is.
fn multline_alignments(rows: &mut [Vec<Node>]) {
    let last = rows.len().saturating_sub(1);
    if last == 0 {
        return;
    }
    // a line is one cell by now: `misplaced_tabs` has joined the cells of a line split by `&`
    for (i, alignment) in [(0, 'l'), (last, 'r')] {
        for cell in rows[i].iter_mut().filter(|cell| !matches!(cell, Node::Multicolumn { .. })) {
            let base = std::mem::replace(cell, Node::row(vec![]));
            *cell = Node::Multicolumn { columns: 1, alignment, base: Box::new(base) };
        }
    }
}

/// Environments that start math, read in text: `Some(true)` for the displays, which TeX allows in a
/// paragraph, `Some(false)` for those TeX allows only in math.
fn math_environment(name: &str) -> Option<bool> {
    match name.trim_end_matches('*') {
        "equation" | "align" | "alignat" | "gather" | "multline" | "flalign" | "eqnarray" | "displaymath" | "math" => Some(true),
        "array" | "matrix" | "pmatrix" | "bmatrix" | "Bmatrix" | "vmatrix" | "Vmatrix" | "smallmatrix" | "cases" | "dcases" | "rcases" | "aligned" | "alignedat" | "gathered"
        | "split" | "subarray" => Some(false),
        _ => None,
    }
}

/// The name in `{name}` at `c[i]` (after spaces), or nothing.
fn environment_name(c: &[char], i: usize) -> String {
    let rest = c[i..].iter().skip_while(|&&x| x == ' ');
    let mut rest = rest.peekable();
    if rest.next() != Some(&'{') {
        return String::new();
    }
    rest.take_while(|&&x| x != '}').collect()
}

/// Where the environment `name` that starts at `c[i]` ends: after its `\end{name}`, counting the ones
/// nested in it; nothing if it does not end.
fn environment_end(c: &[char], i: usize, name: &str) -> Option<usize> {
    let open: Vec<char> = format!("\\begin{{{name}}}").chars().collect();
    let close: Vec<char> = format!("\\end{{{name}}}").chars().collect();
    let mut level = 0;
    let mut k = i;
    while k < c.len() {
        if c[k..].starts_with(&open) {
            level += 1;
            k += open.len();
        } else if c[k..].starts_with(&close) {
            if level == 0 {
                return Some(k + close.len());
            }
            level -= 1;
            k += close.len();
        } else {
            k += 1;
        }
    }
    None
}

/// Where the group that `c[i]` is in closes: its `}`, or the end of the text.
fn group_end(c: &[char], i: usize) -> usize {
    let mut depth = 0usize;
    let mut k = i;
    while k < c.len() {
        match c[k] {
            '\\' => k += 1,
            '{' => depth += 1,
            '}' if depth == 0 => return k,
            '}' => depth -= 1,
            _ => {}
        }
        k += 1;
    }
    c.len()
}

fn is_integer(s: &str) -> bool {
    !s.is_empty() && s.len() <= 30 && s.chars().all(|c| c.is_ascii_digit())
}

/// Converts TeX measures to em (1em = 10pt in the 10pt font).
/// A graphicx length: a measure (`1cm`, `0pt`) or a multiple of the box's `\width`, `\height`,
/// `\depth` or `\totalheight` (`2\height`, `-.5\depth`).
fn box_length(m: &str) -> Option<BoxLength> {
    let m = m.trim();
    for (name, l) in [
        ("\\totalheight", BoxLength { height: 1.0, depth: 1.0, ..BoxLength::default() }),
        ("\\height", BoxLength { height: 1.0, ..BoxLength::default() }),
        ("\\depth", BoxLength { depth: 1.0, ..BoxLength::default() }),
        ("\\width", BoxLength { width: 1.0, ..BoxLength::default() }),
    ] {
        if let Some(factor) = m.strip_suffix(name) {
            let factor = match factor.trim() {
                "" | "+" => 1.0,
                "-" => -1.0,
                f => f.replace(',', ".").parse::<f32>().ok().filter(|f| f.is_finite())?,
            };
            return Some(BoxLength { em: 0.0, width: l.width * factor, height: l.height * factor, depth: l.depth * factor });
        }
    }
    measure_em(m).filter(|v| v.is_finite()).map(|em| BoxLength { em, ..BoxLength::default() })
}

fn measure_em(m: &str) -> Option<f32> {
    let m = m.trim().replace(',', ".");
    if let Some((factor, register)) = m.split_once('\\') {
        return muskip_em(factor.trim(), register.trim());
    }
    let units = [("em", 1.0), ("ex", 0.45), ("pt", 0.1), ("mu", 1.0 / 18.0), ("cm", 2.845), ("mm", 0.2845), ("in", 7.227), ("px", 0.075)];
    for (u, f) in units {
        if let Some(v) = m.strip_suffix(u) {
            return v.trim().parse::<f32>().ok().map(|v| v * f);
        }
    }
    None
}

/// #206: plain TeX's math glue (plain.tex: `\thinmuskip=3mu`, `\medmuskip=4mu plus 2mu minus 4mu`,
/// `\thickmuskip=5mu plus 5mu`; the stretch and shrink are ignored), times a factor (`2\thinmuskip`).
fn muskip_em(factor: &str, register: &str) -> Option<f32> {
    let mu = match register {
        "thinmuskip" => 3.0,
        "medmuskip" => 4.0,
        "thickmuskip" => 5.0,
        _ => return None,
    };
    let factor = match factor {
        "" | "+" => 1.0,
        "-" => -1.0,
        f => f.parse::<f32>().ok().filter(|f| f.is_finite())?,
    };
    Some(factor * mu / 18.0)
}

impl Reader<'_> {
    /// #206: the width of `\mskip`, `\hskip`, `\mspace`, `\kern`…; a measure contextEquation cannot read is no space
    /// in tolerant mode (E-REC-9) and an error in strict mode.
    #[inline(never)]
    fn space_measure(&mut self, measure: &str, col: usize) -> R<Option<f32>> {
        match measure_em(measure) {
            Some(em) => Ok(Some(em)),
            None => {
                self.tolerate(rules::E_REC_9, col, format!("unrecognized measure: {}", measure.trim()), "no space")?;
                Ok(None)
            }
        }
    }

    /// #206: `\thinmuskip`, `\medmuskip` or `\thickmuskip` outside a skip: an assignment (`\thinmuskip=2mu`),
    /// which contextEquation does not honor, read and ignored in tolerant mode (E-REC-9); an error in strict mode.
    #[inline(never)]
    fn muskip_assignment(&mut self, name: &str, col: usize) -> R<()> {
        self.skip_spaces();
        let value = if self.current() == Some('=') {
            self.pos += 1;
            let mut value = self.raw_measure();
            // the stretch and shrink of the glue: `plus 2mu minus 4mu`
            for word in ["plus", "minus"] {
                self.skip_spaces();
                let w: Vec<char> = word.chars().collect();
                if self.c[self.pos..].starts_with(&w) {
                    self.pos += w.len();
                    value = format!("{value} {word} {}", self.raw_measure());
                }
            }
            format!("={value}")
        } else {
            String::new()
        };
        self.tolerate(rules::E_REC_9, col, format!("\\{name}{value}: math glue is not changed"), "ignored")
    }

    /// `\class{…}{…}` and its kin: the content with the attributes [`html_attributes`] keeps.
    fn html_attributes_command(&mut self, name: &str, col: usize) -> R<Node> {
        let value = self.raw_text()?;
        let base = self.argument()?;
        let (node, dropped) = html_attributes(name, &value, base);
        if !dropped.is_empty() {
            self.registry.warn(rules::E_REC_9, format!("column {col}: \\{name}: unsafe or unsupported {}; dropped", dropped.join(", ")));
        }
        Ok(node)
    }

    /// #177: colortbl's colors, which contextEquation's tables have no place for, and the array package's
    /// `\newcolumntype`, which contextEquation does not expand: read with their arguments and ignored (E-REC-9).
    /// MathJax's `\Newextarrow{\cs}{lspace,rspace}{code}` defines `\cs[below]{above}`: an extensible
    /// arrow contextEquation knows (`\xrightarrow` and family) when the code is its arrow, else the character
    /// with the labels over and under it, not stretched (E-REC-9). What is left in place of the command.
    #[inline(never)]
    fn table_and_arrow_definitions(&mut self, name: &str, col: usize) -> R<String> {
        match name {
            "newcolumntype" => {
                let letter = self.raw_text()?;
                self.raw_optional();
                self.raw_text()?;
                self.tolerate(rules::E_REC_9, col, format!("\\newcolumntype{{{}}}: new column types are not expanded", letter.trim()), "ignored")?;
            }
            "Newextarrow" => {
                let target = self.raw_text()?.trim().trim_start_matches('\\').to_string();
                self.raw_text()?;
                let raw = self.raw_text()?;
                let code = raw.trim();
                let symbol = match code.strip_prefix("0x").or_else(|| code.strip_prefix("0X")) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => code.parse::<u32>().ok(),
                }
                .and_then(char::from_u32)
                .ok_or(Error::Syntax { column: col, message: format!("\\Newextarrow: {code} is not a character code") })?;
                let known = match symbol {
                    '→' => "xrightarrow",
                    '←' => "xleftarrow",
                    '⇒' => "xRightarrow",
                    '⇐' => "xLeftarrow",
                    '↔' => "xleftrightarrow",
                    '⇔' => "xLeftrightarrow",
                    '⇌' => "xrightleftharpoons",
                    '⇋' => "xleftrightharpoons",
                    '↦' => "xmapsto",
                    '↪' => "xhookrightarrow",
                    '↩' => "xhookleftarrow",
                    '↠' => "xtwoheadrightarrow",
                    '↞' => "xtwoheadleftarrow",
                    '⇀' => "xrightharpoonup",
                    '⇁' => "xrightharpoondown",
                    '↼' => "xleftharpoonup",
                    '↽' => "xleftharpoondown",
                    '⇄' => "xtofrom",
                    '=' => "xlongequal",
                    _ => "",
                };
                let body = if known.is_empty() {
                    self.tolerate(
                        rules::E_REC_9,
                        col,
                        format!("\\Newextarrow{{\\{target}}}: U+{:04X} has no extensible form in contextEquation", symbol as u32),
                        "drawn with its labels over and under it, not stretched",
                    )?;
                    format!("\\mathrel{{\\overset{{#2}}{{\\underset{{#1}}{{\\unicode{{x{:X}}}}}}}}}", symbol as u32)
                } else {
                    format!("\\{known}[#1]{{#2}}")
                };
                self.registry.note(rules::L_MAC_1, format!("macro \\{target} defined with 2 argument(s)"));
                self.macros.commands.insert(target, Macro { arguments: 2, default_arg: Some(String::new()), body, paired: None });
            }
            _ => {
                self.raw_optional();
                let color = self.raw_text()?;
                if matches!(name, "rowcolor" | "columncolor") {
                    self.raw_optional();
                    self.raw_optional();
                }
                self.tolerate(rules::E_REC_9, col, format!("\\{name}{{{}}}: contextEquation's tables have no colors", color.trim()), "ignored")?;
            }
        }
        Ok(String::new())
    }

    /// A character of an array's column specification that is no column contextEquation knows, at `spec[at]`:
    /// ignored (E-REC-9); the position after it. #177: the `>{…}` or `<{…}` of the array package,
    /// declarations put before or after each cell of the column, which contextEquation does not apply, are
    /// skipped with their group. Out of line, so the frame of `array_columns`, which `*{n}{…}` nests,
    /// does not grow.
    #[inline(never)]
    fn other_column(&mut self, spec: &[char], at: usize, col: usize) -> R<usize> {
        if !(matches!(spec[at], '>' | '<') && spec.get(at + 1) == Some(&'{')) {
            self.tolerate(rules::E_REC_9, col, format!("unsupported array column: '{}'", spec[at]), "ignored")?;
            return Ok(at + 1);
        }
        let mut level = 0;
        let mut end = at + 1;
        while end < spec.len() {
            match spec[end] {
                '{' => level += 1,
                '}' => level -= 1,
                _ => {}
            }
            end += 1;
            if level == 0 {
                break;
            }
        }
        let group: String = spec[at + 1..end].iter().collect();
        self.tolerate(rules::E_REC_9, col, format!("{}{group} of an array column: declarations are not applied to the cells", spec[at]), "ignored")?;
        Ok(end)
    }
}

/// The CSS properties `\style` and `\htmlStyle` keep: they change how the content looks, and cannot
/// load anything or move it out of its place.
const STYLE_PROPERTIES: [&str; 4] = ["color", "background", "background-color", "opacity"];

/// `\class`, `\cssId`, `\style`, `\htmlClass`, `\htmlId`, `\htmlStyle` or `\htmlData` with its first
/// argument `value`, around `base`; and what was dropped as unsafe or unsupported. Classes, ids and
/// data keys keep only `[A-Za-z0-9_-]`; a style keeps only [`STYLE_PROPERTIES`] with plain values
/// (no `url(`, quotes, `;`, `<` or `>`). A style's color also colors the content, as `\color` does,
/// so it reaches the outputs that have no CSS.
fn html_attributes(command: &str, value: &str, base: Node) -> (Node, Vec<String>) {
    let mut dropped = Vec::new();
    let mut attributes: Vec<(String, String)> = Vec::new();
    let mut color = None;
    let safe_name = |raw: &str, dropped: &mut Vec<String>| -> String {
        let kept: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
        if kept.len() != raw.len() {
            dropped.push(format!("characters of {raw:?}"));
        }
        kept
    };
    match command {
        "class" | "htmlClass" => {
            let classes: Vec<String> = value.split_whitespace().map(|c| safe_name(c, &mut dropped)).filter(|c| !c.is_empty()).collect();
            if !classes.is_empty() {
                attributes.push(("class".into(), classes.join(" ")));
            }
        }
        "cssId" | "htmlId" => {
            let id = safe_name(value.trim(), &mut dropped);
            if !id.is_empty() {
                attributes.push(("id".into(), id));
            }
        }
        "style" | "htmlStyle" => {
            let kept;
            (kept, color) = style_declarations(value, &mut dropped);
            if !kept.is_empty() {
                attributes.push(("style".into(), kept.join(";")));
            }
        }
        _ => {
            for pair in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                let (key, v) = pair.split_once('=').unwrap_or((pair, ""));
                // HTML lowercases attribute names; XML keeps them as written
                let key = safe_name(key.trim(), &mut dropped).to_ascii_lowercase();
                if key.is_empty() {
                    dropped.push(format!("data {pair:?}"));
                    continue;
                }
                attributes.push((format!("data-{key}"), v.trim().to_string()));
            }
        }
    }
    let mut out = base;
    if let Some(color) = color {
        out = Node::Colored { color, base: Box::new(out) };
    }
    if !attributes.is_empty() {
        out = Node::Attributed { attributes, base: Box::new(out) };
    }
    (out, dropped)
}

/// The declarations of a style that [`html_attributes`] keeps, as it writes them (`property:value`),
/// and the color among them; the others go to `dropped`.
fn style_declarations(value: &str, dropped: &mut Vec<String>) -> (Vec<String>, Option<String>) {
    let mut kept = Vec::new();
    let mut color = None;
    for declaration in value.split(';').map(str::trim).filter(|d| !d.is_empty()) {
        let Some((property, v)) = declaration.split_once(':') else {
            dropped.push(format!("style {declaration:?}"));
            continue;
        };
        let (property, v) = (property.trim().to_ascii_lowercase(), v.trim());
        let lower = v.to_ascii_lowercase();
        let plain = !v.is_empty() && v.len() < 60 && v.chars().all(|c| c.is_ascii_alphanumeric() || "#(),.% -".contains(c));
        let is_color = v.chars().all(|c| c.is_ascii_alphanumeric() || "#(),. ".contains(c));
        if !STYLE_PROPERTIES.contains(&property.as_str()) || !plain || lower.contains("url(") || lower.contains("expression(") || (property == "color" && !is_color) {
            dropped.push(format!("style {declaration:?}"));
            continue;
        }
        if property == "color" {
            color = Some(v.to_string());
        }
        kept.push(format!("{property}:{v}"));
    }
    (kept, color)
}

/// A style attribute with only the declarations [`html_attributes`] keeps, as it writes them; `None`
/// when none is left. For the MathML reader, which reads back the `style` contextEquation writes.
pub(crate) fn safe_style(value: &str) -> Option<String> {
    let (kept, _) = style_declarations(value, &mut Vec::new());
    (!kept.is_empty()).then(|| kept.join(";"))
}

/// A siunitx number with an uncertainty, as written: compact, `1.23(4)` (and `1.23(4)e3`), or separate,
/// `1.23 \pm 0.04` (and `+-`), which answers `true`.
fn siunitx_uncertainty(raw: &str) -> Option<(Node, bool)> {
    let raw = raw.replace("+-", "\\pm");
    if let Some((value, uncertainty)) = raw.split_once("\\pm") {
        let value = siunitx_number(value, true)?;
        let uncertainty = siunitx_number(uncertainty, true)?;
        return Some((Node::row(vec![value, Node::Operator { symbol: '±', class: Class::Bin }, uncertainty]), true));
    }
    let (value, rest) = raw.split_once('(')?;
    let (uncertainty, exponent) = rest.split_once(')')?;
    let uncertainty = uncertainty.trim();
    if uncertainty.is_empty() || !uncertainty.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',')) {
        return None;
    }
    // the parenthesized digits right after the number, before any exponent
    let mut children = match siunitx_number(&format!("{value}{exponent}"), true)? {
        Node::Row { children } => children,
        number => vec![number],
    };
    let at = children.iter().position(|n| matches!(n, Node::Number { .. }))? + 1;
    let compact = [
        Node::Operator { symbol: '(', class: Class::Open },
        Node::Number { value: uncertainty.replace(',', "."), group: false, thousands: None, variant: Variant::Upright },
        Node::Operator { symbol: ')', class: Class::Close },
    ];
    children.splice(at..at, compact);
    Some((Node::row(children), false))
}

/// A number in siunitx format: decimal comma or dot, spaces, exponent with `e`.
fn siunitx_number(raw: &str, group: bool) -> Option<Node> {
    let clean: String = raw.replace("\\,", "").replace(['{', '}', ' ', '~'], "");
    let (mantissa, exponent) = match clean.split_once(['e', 'E']) {
        Some((m, e)) => (m.to_string(), Some(e.to_string())),
        None => (clean, None),
    };
    let (sign, mantissa) = match mantissa.strip_prefix(['-', '+']) {
        Some(remainder) => (mantissa.chars().next(), remainder.to_string()),
        None => (None, mantissa),
    };
    let value = mantissa.replace(',', ".");
    if value.is_empty() || value.matches('.').count() > 1 || !value.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let mut children = Vec::new();
    if let Some(s) = sign {
        children.push(Node::Operator { symbol: if s == '-' { '−' } else { '+' }, class: Class::Ord });
    }
    children.push(Node::Number { value, group, thousands: None, variant: Variant::Upright });
    if let Some(e) = exponent {
        let (sign_e, digits) = match e.strip_prefix(['-', '+']) {
            Some(d) => (e.starts_with('-'), d.to_string()),
            None => (false, e.clone()),
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut exp = Vec::new();
        if sign_e {
            exp.push(Node::Operator { symbol: '−', class: Class::Ord });
        }
        exp.push(Node::Number { value: digits, group: false, thousands: None, variant: Variant::Upright });
        children.push(Node::Operator { symbol: '×', class: Class::Bin });
        children.push(Node::Scripts {
            base: Box::new(Node::Number { value: "10".into(), group: false, thousands: None, variant: Variant::Upright }),
            sup: Some(Box::new(Node::row(exp).simplify())),
            sub: None,
        });
    }
    Some(Node::row(children).simplify())
}

// ---------- macros ----------

impl Reader<'_> {
    /// Replaces `self.c[start..self.pos]` with `text` and resumes reading at `start`.
    /// In tolerant mode, an expansion beyond the limit disappears from the text with a warning.
    fn inject(&mut self, start: usize, text: &str, col: usize) -> R<()> {
        let replacement: Vec<char> = text.chars().collect();
        let end = self.pos;
        // what was already read is not read again: a text that fits in the replaced span goes to its
        // end, without moving the rest of the input (\bgroup repeated a thousand times does not go quadratic)
        if replacement.len() <= end - start {
            let start = end - replacement.len();
            self.c[start..end].copy_from_slice(&replacement);
            self.pos = start;
            return Ok(());
        }
        // growing moves the rest of the input: expansions that grow count against the limit too
        self.macros.expansions += 1;
        let excess = self.macros.expansions > EXPANSION_LIMIT || self.c.len() + replacement.len() > self.macros.max_size;
        let replacement = if excess {
            if !self.macros.warned_excess {
                self.tolerate(rules::E_REC_1, col, "too many macros: a macro probably calls itself", "expansions beyond the limit were discarded")?;
                self.macros.warned_excess = true;
            }
            Vec::new()
        } else {
            replacement
        };
        self.c.splice(start..end, replacement);
        self.pos = start;
        Ok(())
    }

    fn expand(&mut self, start: usize, m: &Macro, col: usize) -> R<()> {
        self.macros.expansions += 1;
        let mut star = false;
        let mut size = None;
        if m.paired.is_some() {
            // \abs{x} → \mathopen{|}x\mathclose{|};  \abs*{x} → \left|x\right|;  \abs[\big]{x} → \bigl|x\bigr|
            star = self.current() == Some('*');
            if star {
                self.pos += 1;
            }
            size = self.raw_optional();
        }
        let mut args = Vec::new();
        if let Some(default_arg) = &m.default_arg {
            args.push(self.raw_optional().unwrap_or_else(|| default_arg.clone()));
        }
        while args.len() < m.arguments {
            args.push(self.raw_text()?);
        }
        let mut body = String::new();
        let c: Vec<char> = m.body.chars().collect();
        let mut i = 0;
        while i < c.len() {
            if c[i] == '#' && i + 1 < c.len() {
                if let Some(d) = c[i + 1].to_digit(10) {
                    // #0 does not exist in TeX: it disappears, like a missing argument
                    // a paired delimiter's argument goes in as written, as TeX puts it in
                    if let Some(a) = (d as usize).checked_sub(1).and_then(|k| args.get(k)) {
                        let braces = m.paired.is_none();
                        if braces {
                            body.push('{');
                        }
                        body.push_str(a);
                        if braces {
                            body.push('}');
                        }
                    }
                    i += 2;
                    continue;
                }
            }
            body.push(c[i]);
            i += 1;
        }
        if let Some(p) = &m.paired {
            // mathtools' \delimsize: the size the delimiters got, \middle with the star
            let delimsize = if star { "\\middle" } else { size.as_deref().unwrap_or("") };
            let x = body.replace("\\delimsize", delimsize);
            let (open, close, pre, post) = (&p.open, &p.close, &p.pre, &p.post);
            body = match (star, size) {
                (true, _) => format!("{pre}\\left{open} {x} \\right{close}{post}"),
                (false, Some(t)) => format!("{pre}{t}l{open} {x} {t}r{close}{post}"),
                (false, None) => format!("{pre}\\mathopen{{{open}}} {x} \\mathclose{{{close}}}{post}"),
            };
        }
        // a space keeps the macro from sticking to what follows (\foo x)
        body.push(' ');
        self.inject(start, &body, col)
    }

    /// Name being defined: `{\foo}` or `\foo`.
    fn defined_name(&mut self, col: usize) -> R<String> {
        self.skip_spaces();
        let raw = if self.current() == Some('{') {
            self.raw_text()?
        } else if self.current() == Some('\\') {
            self.pos += 1;
            format!("\\{}", self.read_command_name())
        } else {
            return self.error(col, "missing macro name");
        };
        let name = raw.trim().trim_start_matches('\\').to_string();
        if name.is_empty() {
            return self.error(col, "empty macro name");
        }
        Ok(name)
    }

    /// Commands that define macros. Returns `true` if it handled the command.
    fn definition(&mut self, name: &str, col: usize) -> R<bool> {
        match name {
            "newcommand" | "renewcommand" | "providecommand" | "DeclareRobustCommand" => {
                if self.current() == Some('*') {
                    self.pos += 1;
                }
                let target = self.defined_name(col)?;
                let arguments = match self.raw_optional() {
                    Some(n) => n.trim().parse::<usize>().map_err(|_| Error::Syntax { column: col, message: format!("invalid number of arguments: {n}") })?,
                    None => 0,
                };
                let default_arg = self.raw_optional();
                let body = self.raw_text()?;
                if name == "providecommand" && self.macros.commands.contains_key(&target) {
                    return Ok(true);
                }
                self.registry.note(rules::L_MAC_1, format!("macro \\{target} defined with {arguments} argument(s)"));
                self.macros.commands.insert(target, Macro { arguments, default_arg, body, paired: None });
                Ok(true)
            }
            "def" | "gdef" | "edef" | "xdef" => {
                let target = self.defined_name(col)?;
                let mut arguments = 0;
                while self.current() != Some('{') {
                    match (self.current(), self.em(1)) {
                        (Some('#'), Some(d)) if d.is_ascii_digit() => {
                            arguments += 1;
                            self.pos += 2;
                        }
                        (Some(c), _) if c.is_whitespace() => self.pos += 1,
                        (None, _) => return self.error(col, format!("\\def\\{target} without a body")),
                        _ => return self.error(col, "\\def with delimited parameters is not supported (only #1#2…)"),
                    }
                }
                let body = self.raw_text()?;
                self.registry.note(rules::L_MAC_1, format!("macro \\{target} defined with {arguments} argument(s)"));
                self.macros.commands.insert(target, Macro { arguments, default_arg: None, body, paired: None });
                Ok(true)
            }
            "let" => {
                let target = self.defined_name(col)?;
                self.skip_spaces();
                if self.current() == Some('=') {
                    self.pos += 1;
                    self.skip_spaces();
                }
                let body = if self.current() == Some('\\') {
                    self.pos += 1;
                    let n = self.read_command_name();
                    match self.macros.commands.get(&n).cloned() {
                        Some(m) => {
                            self.macros.commands.insert(target, m);
                            return Ok(true);
                        }
                        None => format!("\\{n}"),
                    }
                } else {
                    let c = self.current().ok_or(Error::Syntax { column: col, message: "\\let without a target".into() })?;
                    self.pos += 1;
                    c.to_string()
                };
                self.macros.commands.insert(target, Macro { arguments: 0, default_arg: None, body, paired: None });
                Ok(true)
            }
            "DeclareMathOperator" => {
                let star = self.current() == Some('*');
                if star {
                    self.pos += 1;
                }
                let target = self.defined_name(col)?;
                let text = self.raw_text()?;
                let body = format!("\\operatorname{}{{{text}}}", if star { "*" } else { "" });
                self.registry.note(rules::L_MAC_1, format!("operator \\{target} defined as '{text}'"));
                self.macros.commands.insert(target, Macro { arguments: 0, default_arg: None, body, paired: None });
                Ok(true)
            }
            // mathtools: \DeclarePairedDelimiter{\abs}{\lvert}{\rvert}; the X form takes [n] arguments and
            // a body, where \delimsize is the size of the delimiters; XPP adds what goes before and after
            "DeclarePairedDelimiter" | "DeclarePairedDelimiterX" | "DeclarePairedDelimiterXPP" => {
                let target = self.defined_name(col)?;
                let arguments = match self.raw_optional() {
                    Some(n) => n.trim().parse::<usize>().map_err(|_| Error::Syntax { column: col, message: format!("invalid number of arguments: {n}") })?,
                    None => 1,
                };
                let pre = if name == "DeclarePairedDelimiterXPP" { self.raw_text()? } else { String::new() };
                let open = self.raw_text()?.trim().to_string();
                let close = self.raw_text()?.trim().to_string();
                let post = if name == "DeclarePairedDelimiterXPP" { self.raw_text()? } else { String::new() };
                let body = if name == "DeclarePairedDelimiter" { "#1".to_string() } else { self.raw_text()? };
                let paired = Some(Box::new(Paired { open, close, pre, post }));
                self.macros.commands.insert(target, Macro { arguments, default_arg: None, body, paired });
                Ok(true)
            }
            "newenvironment" | "renewenvironment" => {
                let target = self.raw_text()?.trim().to_string();
                if let Some(n) = self.raw_optional() {
                    if n.trim() != "0" {
                        return self.error(col, "\\newenvironment with arguments is not supported");
                    }
                }
                let open = self.raw_text()?;
                let close = self.raw_text()?;
                self.macros.environments.insert(target, (open, close));
                Ok(true)
            }
            // chemfig, manual section 9: a submolecule `!{name}` stands for in the structures after it
            #[cfg(feature = "chem")]
            "definesubmol" | "redefinesubmol" => {
                self.skip_spaces();
                let target = if self.current() == Some('\\') {
                    self.pos += 1;
                    format!("\\{}", self.read_command_name())
                } else {
                    self.raw_text()?.trim().to_string()
                };
                self.skip_spaces();
                let arguments = match self.current().and_then(|d| d.to_digit(10)) {
                    Some(n) => {
                        self.pos += 1;
                        n as usize
                    }
                    None => 0,
                };
                let right = self.raw_optional();
                let code = self.raw_text()?;
                self.macros.submols.insert(target, crate::chem::model::Submol { arguments, right, code });
                Ok(true)
            }
            // xcolor: a color for the rest of the input, as \newcommand defines a macro
            "definecolor" | "providecolor" => {
                self.raw_optional();
                let target = self.raw_text()?.trim().to_string();
                let model = self.raw_text()?;
                let value = self.raw_text()?;
                match color::model(&model, &value) {
                    Some(rgb) if name == "definecolor" || !self.macros.colors.contains_key(&target) => {
                        self.macros.colors.insert(target, color::Defined { css: color::hex(rgb), rgb: Some(rgb) });
                    }
                    Some(_) => {}
                    None => self.tolerate(rules::E_REC_9, col, format!("invalid color: {{{}}}{{{}}}", model.trim(), value.trim()), "not defined")?,
                }
                Ok(true)
            }
            "colorlet" => {
                self.raw_optional();
                let target = self.raw_text()?.trim().to_string();
                self.raw_optional();
                let expression = self.raw_text()?.trim().to_string();
                match color::expression(&expression, &self.macros.colors).or_else(|| color::name(&expression)) {
                    Some(defined) => {
                        self.macros.colors.insert(target, defined);
                    }
                    None => self.tolerate(rules::E_REC_9, col, format!("invalid color: {expression}"), "not defined")?,
                }
                Ok(true)
            }
            "usepackage" | "RequirePackage" | "require" => {
                self.raw_optional();
                let package = self.raw_text()?;
                self.registry.note(rules::L_ENT_3, format!("\\{name}{{{package}}} ignored: the supported packages are already loaded"));
                Ok(true)
            }
            "makeatletter" | "makeatother" | "ignorespaces" | "leavevmode" | "sc" | "scshape" | "upshape" | "itshape" | "bfseries" | "mdseries" | "rmfamily" | "sffamily"
            | "ttfamily" => Ok(true),
            "aftergroup" => {
                // TeX primitive: defers the next token; here it is just discarded
                self.skip_spaces();
                if self.current() == Some('\\') {
                    self.pos += 1;
                    self.read_command_name();
                } else if self.current().is_some() {
                    self.pos += 1;
                }
                Ok(true)
            }
            _ => self.siunitx_definition(name, col),
        }
    }

    /// Raw content up to the matching `\end{name}` (which is consumed).
    fn environment_body(&mut self, name: &str, col: usize) -> R<String> {
        let open: Vec<char> = format!("\\begin{{{name}}}").chars().collect();
        let close: Vec<char> = format!("\\end{{{name}}}").chars().collect();
        let start = self.pos;
        let mut level = 1;
        let mut i = start;
        while i < self.c.len() {
            if self.c[i..].starts_with(&open) {
                level += 1;
                i += open.len();
            } else if self.c[i..].starts_with(&close) {
                level -= 1;
                if level == 0 {
                    self.pos = i + close.len();
                    return Ok(self.c[start..i].iter().collect());
                }
                i += close.len();
            } else {
                i += 1;
            }
        }
        self.tolerate(rules::E_REC_5, col, format!("\\begin{{{name}}} without \\end{{{name}}}"), "closed at the end")?;
        self.pos = self.c.len();
        Ok(self.c[start..].iter().collect())
    }

    /// A braced argument, if one comes next (after spaces).
    fn optional_braced(&mut self) -> R<Option<String>> {
        let saved = self.pos;
        self.skip_spaces();
        if self.current() == Some('{') {
            return Ok(Some(self.raw_text()?));
        }
        self.pos = saved;
        Ok(None)
    }

    /// Content between `open` and its matching `close` (the opening one is the current character).
    fn raw_delimited(&mut self, open: char, close: char) -> String {
        self.pos += 1;
        let mut level = 0;
        let mut s = String::new();
        while let Some(c) = self.current() {
            self.pos += 1;
            if c == close && level == 0 {
                return s;
            }
            if c == open && open != close {
                level += 1;
            } else if c == close {
                level -= 1;
            }
            s.push(c);
        }
        s
    }

    /// Is the `\\qty` here physics' (brackets) and not siunitx's (`{value}{unit}`)?
    fn physics_qty_follows(&mut self) -> bool {
        let saved = self.pos;
        self.skip_spaces();
        let physics = match self.current() {
            Some('(' | '|' | '<') => true,
            // \qty[options]{value}{unit} is siunitx's; physics's \qty[…] has no braces after it
            Some('[') => {
                let _ = self.raw_optional();
                self.skip_spaces();
                self.current() != Some('{') || {
                    let _ = self.raw_text();
                    self.skip_spaces();
                    self.current() != Some('{')
                }
            }
            Some('{') => {
                let _ = self.raw_text();
                self.skip_spaces();
                self.current() != Some('{')
            }
            _ => false,
        };
        self.pos = saved;
        physics
    }

    /// physics' automatic brackets: `(…)`, `[…]`, `|…|`, `{…}` or `<…>` sized by `\left…\right`.
    fn physics_brackets(&mut self) -> R<Option<String>> {
        self.skip_spaces();
        let (open, close, left, right) = match self.current() {
            Some('(') => ('(', ')', "(", ")"),
            Some('[') => ('[', ']', "[", "]"),
            Some('|') => ('|', '|', "|", "|"),
            Some('<') => ('<', '>', "\\langle", "\\rangle"),
            Some('{') => {
                let x = self.raw_text()?;
                return Ok(Some(format!("\\left\\{{ {x} \\right\\}}")));
            }
            _ => return Ok(None),
        };
        let x = self.raw_delimited(open, close);
        Ok(Some(format!("\\left{left} {x} \\right{right}")))
    }

    /// physics' `\sin[n](x)`: `\sin^{n}` and `x` in parentheses that resize; `None`, with nothing read, when the brackets are
    /// not followed by parentheses.
    #[inline(never)]
    fn physics_function_power(&mut self, name: &str) -> Option<String> {
        let saved = self.pos;
        if let Some(power) = self.raw_optional() {
            self.skip_spaces();
            if self.current() == Some('(') {
                let x = self.raw_delimited('(', ')');
                // physics' \argopen: an empty \mathopen keeps the space after the function out
                return Some(format!("\\{name}^{{{power}}}\\mathopen{{}}\\mathclose{{\\left( {x} \\right)}}"));
            }
        }
        self.pos = saved;
        None
    }

    /// Package commands defined through other commands (physics, braket, xfrac, units, tensor, mathtools).
    #[inline(never)]
    fn package_expansion(&mut self, name: &str, col: usize) -> R<Option<String>> {
        let d = |p: bool| if p { "\\partial" } else { "\\mathrm{d}" };
        let star = |this: &mut Self| {
            let s = this.current() == Some('*');
            if s {
                this.pos += 1;
            }
            s
        };
        if let Some(text) = self.tex_expansion(name, col)? {
            return Ok(Some(text));
        }
        Ok(Some(match name {
            "fdv" | "functionalderivative" | "fderivative" => {
                let flat = star(self);
                let power = self.raw_optional().map(|n| format!("^{{{n}}}")).unwrap_or_default();
                let f = self.raw_text()?;
                match self.optional_braced()? {
                    Some(x) => physics_fraction(flat, format!("\\delta{power} {f}"), format!("\\delta {x}{power}")),
                    None => physics_fraction(flat, format!("\\delta{power}"), format!("\\delta {f}{power}")),
                }
            }
            "dv" | "odv" | "pdv" | "derivative" | "partialderivative" | "pderivative" | "pd" => {
                let partial = name.starts_with('p');
                // physics' \dv* and \pdv*: the derivative as a flat fraction, df/dx
                let flat = matches!(name, "dv" | "pdv" | "derivative" | "partialderivative") && star(self);
                let order = self.raw_optional();
                let f = self.raw_text()?;
                self.skip_spaces();
                let x = if self.current() == Some('{') { Some(self.raw_text()?) } else { None };
                self.skip_spaces();
                let y = if partial && x.is_some() && self.current() == Some('{') { Some(self.raw_text()?) } else { None };
                let dd = d(partial);
                let (numerator, denominator) = match (x, y, order) {
                    (None, _, None) => (dd.to_string(), format!("{dd} {f}")),
                    (None, _, Some(n)) => (format!("{dd}^{{{n}}}"), format!("{dd} {f}^{{{n}}}")),
                    (Some(x), None, None) => (format!("{dd} {f}"), format!("{dd} {x}")),
                    (Some(x), None, Some(n)) => (format!("{dd}^{{{n}}} {f}"), format!("{dd} {x}^{{{n}}}")),
                    (Some(x), Some(y), _) => (format!("{dd}^{{2}} {f}"), format!("{dd} {x} \\, {dd} {y}")),
                };
                physics_fraction(flat, numerator, denominator)
            }
            // physics' \ket* and \bra*: brackets that do not resize (braket's \Ket and \Bra have no star)
            "ket" | "Ket" => {
                let s = name == "ket" && star(self);
                physics_fence(s, "|", "|", "\\rangle", &[&self.raw_text()?])
            }
            "bra" | "Bra" => {
                let s = name == "bra" && star(self);
                physics_fence(s, "\\langle", "|", "|", &[&self.raw_text()?])
            }
            // mathtools (its names since 2022) and colonequals: a colon on the axis beside another
            // relation, set as one; \coloneq, \coloneqq, \eqcolon and \eqqcolon are single symbols
            "Coloneq" | "coloncolonequals" => "\\Coloneqq ".into(),
            "Eqcolon" | "Eqqcolon" | "equalscoloncolon" => "\\mathrel{\\mathord{=}\\mkern-1.2mu\\mathord{\\dblcolon}}".into(),
            "colondash" | "colonminus" => "\\mathrel{\\mathord{\\vcentcolon}\\mkern-1.2mu\\mathord{-}}".into(),
            "Colondash" | "coloncolonminus" => "\\mathrel{\\mathord{\\dblcolon}\\mkern-1.2mu\\mathord{-}}".into(),
            "dashcolon" | "minuscolon" => "\\mathrel{\\mathord{-}\\mkern-1.2mu\\mathord{\\vcentcolon}}".into(),
            "Dashcolon" | "minuscoloncolon" => "\\mathrel{\\mathord{-}\\mkern-1.2mu\\mathord{\\dblcolon}}".into(),
            "colonapprox" => "\\mathrel{\\mathord{\\vcentcolon}\\mkern-1.2mu\\mathord{\\approx}}".into(),
            "Colonapprox" | "coloncolonapprox" => "\\mathrel{\\mathord{\\dblcolon}\\mkern-1.2mu\\mathord{\\approx}}".into(),
            "colonsim" => "\\mathrel{\\mathord{\\vcentcolon}\\mkern-1.2mu\\mathord{\\sim}}".into(),
            "Colonsim" | "coloncolonsim" => "\\mathrel{\\mathord{\\dblcolon}\\mkern-1.2mu\\mathord{\\sim}}".into(),
            "approxcolon" => "\\mathrel{\\mathord{\\approx}\\mkern-1.2mu\\mathord{\\vcentcolon}}".into(),
            "Approxcolon" | "approxcoloncolon" => "\\mathrel{\\mathord{\\approx}\\mkern-1.2mu\\mathord{\\dblcolon}}".into(),
            "simcolon" => "\\mathrel{\\mathord{\\sim}\\mkern-1.2mu\\mathord{\\vcentcolon}}".into(),
            "Simcolon" | "simcoloncolon" => "\\mathrel{\\mathord{\\sim}\\mkern-1.2mu\\mathord{\\dblcolon}}".into(),
            // amsmath: italic capital Greek, and the limits of category theory and of sets
            "injlim" => "\\operatorname*{inj\\,lim}".into(),
            "projlim" => "\\operatorname*{proj\\,lim}".into(),
            "varinjlim" => "\\mathop{\\underrightarrow{\\mathrm{lim}}}".into(),
            "varprojlim" => "\\mathop{\\underleftarrow{\\mathrm{lim}}}".into(),
            "varliminf" => "\\mathop{\\underline{\\mathrm{lim}}}".into(),
            "varlimsup" => "\\mathop{\\overline{\\mathrm{lim}}}".into(),
            "operatornamewithlimits" => format!("\\operatorname*{{{}}}", self.raw_text()?),
            // mathtools: the operator with its limits; the limits keep their width (partial support)
            "smashoperator" => {
                self.raw_optional();
                format!("{{{}}}", self.raw_text()?)
            }
            // slashed.sty's \declareslashed{class}{slash}{right shift}{up shift}{symbol}: another slash or
            // offset for one symbol; read, and not applied (each \slashed is drawn centered)
            "declareslashed" => {
                for _ in 0..5 {
                    self.raw_text()?;
                }
                String::new()
            }
            // the LaTeX kernel
            "underbar" => format!("\\underline{{{}}}", self.raw_text()?),
            // definitions already last for the whole equation: \global changes nothing
            "global" => String::new(),
            "space" => "\\ ".into(),
            "nobreakspace" => "~".into(),
            // hyperref: the link's text is what the equation shows
            "href" => {
                self.raw_text()?;
                format!("{{{}}}", self.raw_text()?)
            }
            // TeX's \char: a character by its code, decimal, "hex, 'octal or `character
            "char" => {
                self.skip_spaces();
                if self.current() == Some('`') {
                    // `a, or `\a: the character itself
                    self.pos += 1;
                    if self.current() == Some('\\') {
                        self.pos += 1;
                    }
                    let c = self.current().ok_or(Error::Syntax { column: col, message: "\\char` without a character".into() })?;
                    self.pos += 1;
                    return Ok(Some(format!("\\unicode{{x{:X}}}", c as u32)));
                }
                let (radix, digits): (u32, fn(char) -> bool) = match self.current() {
                    Some('"') => (16, |c| c.is_ascii_hexdigit()),
                    Some('\'') => (8, |c| ('0'..='7').contains(&c)),
                    _ => (10, |c| c.is_ascii_digit()),
                };
                if radix != 10 {
                    self.pos += 1;
                }
                let code: String = self.c[self.pos..].iter().take_while(|c| digits(**c)).collect();
                self.pos += code.chars().count();
                match u32::from_str_radix(&code, radix).ok().and_then(char::from_u32).filter(|c| !c.is_control()) {
                    Some(c) => format!("\\unicode{{x{:X}}}", c as u32),
                    None => return self.error(col, format!("\\char needs a character code, not \"{code}\"")),
                }
            }
            "braket" | "Braket" | "innerproduct" | "ip" => {
                let s = name != "Braket" && star(self);
                let x = self.raw_text()?;
                match self.optional_braced()? {
                    // physics: \braket{a}{b}
                    Some(y) => physics_fence(s, "\\langle", "|", "\\rangle", &[&x, &y]),
                    // braket: \braket{a|b}
                    None => physics_fence(s, "\\langle", "|", "\\rangle", &x.splitn(3, '|').collect::<Vec<_>>()),
                }
            }
            "ketbra" | "outerproduct" | "dyad" => {
                let s = star(self);
                let a = self.raw_text()?;
                let b = self.optional_braced()?.unwrap_or_else(|| a.clone());
                format!("{}\\!{}", physics_fence(s, "|", "|", "\\rangle", &[&a]), physics_fence(s, "\\langle", "|", "|", &[&b]))
            }
            // #177: the \Ketbra braket.sty suggests (its comments: each side as tall as both)
            "Ketbra" => {
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                format!("\\left|{{{a}}}\\vphantom{{{b}}}\\right\\rangle\\left\\langle{{{b}}}\\vphantom{{{a}}}\\right|")
            }
            // bm's \hm: heavy bold where a heavy math version exists, \bm otherwise (`\let\hm\bm`)
            "hm" => format!("\\bm{{{}}}", self.raw_text()?),
            // MathJax's \data{key=value}{math}, its html extension: KaTeX's \htmlData
            "data" => "\\htmlData".into(),
            // mhchem's deprecated \cf, its old name for \ce
            "cf" => format!("\\ce{{{}}}", self.raw_text()?),
            // colortbl, the array package's \newcolumntype and MathJax's \Newextarrow
            "cellcolor" | "rowcolor" | "columncolor" | "arrayrulecolor" | "newcolumntype" | "Newextarrow" => self.table_and_arrow_definitions(name, col)?,
            // starred, the brackets do not resize; doubly starred (\expval**, \mel**), they do
            "expval" | "ev" | "expectationvalue" => {
                let s = star(self) && !star(self);
                let a = self.raw_text()?;
                match self.optional_braced()? {
                    Some(psi) => physics_fence(s, "\\langle", "|", "\\rangle", &[&psi, &a, &psi]),
                    None => physics_fence(s, "\\langle", "|", "\\rangle", &[&a]),
                }
            }
            "mel" | "matrixel" | "matrixelement" => {
                let s = star(self) && !star(self);
                let a = self.raw_text()?;
                let op = self.raw_text()?;
                let b = self.raw_text()?;
                physics_fence(s, "\\langle", "|", "\\rangle", &[&a, &op, &b])
            }
            "comm" | "commutator" | "acomm" | "acommutator" | "anticommutator" | "pb" | "poissonbracket" => {
                let s = star(self);
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                let ab = format!("{a}, {b}");
                if matches!(name, "comm" | "commutator") {
                    physics_fence(s, "[", "|", "]", &[&ab])
                } else {
                    physics_fence(s, "\\{", "|", "\\}", &[&ab])
                }
            }
            // physics' vector calculus: the nabla in bold (U+1D6C1), like the package
            "grad" | "gradient" => "\\unicode{x1D6C1} ".into(),
            "divergence" => "\\unicode{x1D6C1} \\cdot ".into(),
            "curl" | "rot" => "\\unicode{x1D6C1} \\times ".into(),
            "laplacian" => "\\nabla^{2} ".into(),
            "cross" | "crossproduct" | "cp" => "\\times ".into(),
            "vdot" | "dotproduct" => "\\cdot ".into(),
            "vb" | "vectorbold" => {
                let bold = if star(self) { "\\boldsymbol" } else { "\\mathbf" };
                format!("{bold}{{{}}}", self.raw_text()?)
            }
            "vu" | "vectorunit" => {
                let bold = if star(self) { "\\boldsymbol" } else { "\\mathbf" };
                format!("\\hat{{{bold}{{{}}}}}", self.raw_text()?)
            }
            "va" | "vectorarrow" => {
                star(self);
                format!("\\vec{{{}}}", self.raw_text()?)
            }
            "order" => match self.physics_brackets()? {
                Some(inner) if inner.starts_with("\\left\\{") => format!("\\mathcal{{O}}{}", inner.replacen("\\left\\{", "\\left(", 1).replacen("\\right\\}", "\\right)", 1)),
                Some(inner) => format!("\\mathcal{{O}}{inner}"),
                None => "\\mathcal{O}".into(),
            },
            "Tr" | "tr" | "rank" | "Res" | "erf" | "Rank" | "trace" => {
                let shown = if name == "trace" { "tr" } else { name };
                format!("\\operatorname{{{shown}}}")
            }
            "dd" | "differential" => {
                let order = self.raw_optional();
                let power = order.map(|n| format!("^{{{n}}}")).unwrap_or_default();
                let x = self.optional_braced()?.unwrap_or_default();
                format!("\\mathop{{}}\\!\\mathrm{{d}}{power}{x} ")
            }
            "eval" | "evaluated" => {
                self.skip_spaces();
                match self.current() {
                    Some(o @ ('(' | '[')) => {
                        let close = if o == '(' { ')' } else { ']' };
                        let x = self.raw_delimited(o, close);
                        self.skip_spaces();
                        if self.current() == Some('|') {
                            self.pos += 1;
                        }
                        format!("\\left{o} {x} \\right|")
                    }
                    _ => format!("\\left. {} \\right|", self.raw_text()?),
                }
            }
            // physics' \qty(…), \qty[…], \qty|…|, \qty{x}; siunitx's \qty{3}{m} (two groups) goes to the units
            "qty" | "quantity" if name == "quantity" || self.physics_qty_follows() => self.physics_brackets()?.unwrap_or_default(),
            "pqty" | "bqty" | "vqty" | "Bqty" => {
                let s = star(self);
                let x = self.raw_text()?;
                let (open, close) = match name {
                    "pqty" => ("(", ")"),
                    "bqty" => ("[", "]"),
                    "vqty" => ("|", "|"),
                    _ => ("\\{", "\\}"),
                };
                physics_fence(s, open, "|", close, &[&x])
            }
            // esint's \dotsint is amsmath's \idotsint
            "idotsint" | "dotsint" => "\\int\\!\\cdots\\!\\int ".into(),
            // mathtools: \prescript{sup}{sub}{base}
            "prescript" => {
                let sup = self.raw_text()?;
                let sub = self.raw_text()?;
                let base = self.raw_text()?;
                format!("{{}}^{{{sup}}}_{{{sub}}}{{{base}}}")
            }
            // tensor package: \tensor[pre]{T}{^\mu_\nu}, \indices{^\mu_\nu}
            "tensor" | "indices" => {
                star(self);
                let pre = self.raw_optional();
                let base = if name == "tensor" { self.raw_text()? } else { String::new() };
                let indices = self.raw_text()?;
                format!("{}{{{base}}}{}", pre.map(|p| staggered_indices(&p, true)).unwrap_or_default(), staggered_indices(&indices, false))
            }
            // esvect: \vv*{x}{i} is \vv{x}_{i}, the index pulled 1 mu closer (\vv alone is in the accents)
            "vv" if self.current() == Some('*') => {
                self.pos += 1;
                let x = self.raw_text()?;
                let index = self.raw_text()?;
                format!("\\overrightarrow{{{x}}}_{{\\mkern-1mu {index}}}")
            }
            // mathabx: \widering, built on \wideparen as the package builds it, a ring at the top of the arc
            "widering" => format!("\\mathring{{\\wideparen{{{}}}}}", self.raw_text()?),
            "Set" | "set" => {
                let x = self.raw_text()?;
                match x.split_once('|') {
                    Some((a, b)) => format!("\\left\\{{ {a} \\;\\middle|\\; {b} \\right\\}}"),
                    None => format!("\\left\\{{ {x} \\right\\}}"),
                }
            }
            // physics: \abs*{…} and \norm*{…} keep the delimiters at the text size
            "abs" | "absolutevalue" => {
                let s = star(self);
                physics_fence(s, "|", "|", "|", &[&self.raw_text()?])
            }
            "norm" => {
                let s = star(self);
                physics_fence(s, "\\|", "|", "\\|", &[&self.raw_text()?])
            }
            // physics' \sin[2](x): the power on the function, the argument in parentheses that resize
            // (without the parentheses, physics leaves [2] as written, as contextEquation does)
            "sin" | "cos" | "tan" | "csc" | "sec" | "cot" | "arcsin" | "arccos" | "arctan" | "arccsc" | "arcsec" | "arccot" | "sinh" | "cosh" | "tanh" | "csch" | "sech"
            | "coth" | "log" | "ln"
                if self.current() == Some('[') =>
            {
                match self.physics_function_power(name) {
                    Some(expansion) => expansion,
                    None => return Ok(None),
                }
            }
            "sfrac" | "nicefrac" => {
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                format!("{{}}^{{{a}}}\\!/{{}}_{{{b}}}")
            }
            "unitfrac" => {
                let v = self.raw_optional();
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                format!("{}\\,{{}}^{{\\mathrm{{{a}}}}}\\!/{{}}_{{\\mathrm{{{b}}}}}", v.unwrap_or_default())
            }
            "unit" if self.current() == Some('[') => {
                // \unit[3]{m} from the units package
                let v = self.raw_optional().unwrap_or_default();
                let u = self.raw_text()?;
                format!("{v}\\,\\mathrm{{{u}}}")
            }
            // (asked by name: its frame stays off the stack of the commands that nest, \frac or \sqrt)
            _ if PHYSICS.contains(&name) || physics_long_name(name).is_some() => return self.physics_expansion(name, col),
            // #172: KaTeX's texvc aliases (\R, \cnums, \bold…)
            _ if symbols::texvc_macro(name).is_some() => return Ok(symbols::texvc_macro(name).map(str::to_string)),
            // #169's mathtools, asked by name too
            _ if mathtools::is_mathtools(name, &self.c, self.pos) => return self.mathtools_expansion(name, col),
            _ => return Ok(None),
        }))
    }

    /// physics' matrices, spaced text and the long names of its operators (physics.sty 1.3; MathJax's
    /// physics extension draws the same), the names in `PHYSICS`. Out of line, so `package_expansion`,
    /// which runs for every command, keeps its size.
    #[inline(never)]
    fn physics_expansion(&mut self, name: &str, col: usize) -> R<Option<String>> {
        let starred = |this: &mut Self| {
            let s = this.current() == Some('*');
            if s {
                this.pos += 1;
            }
            s
        };
        Ok(Some(match name {
            // \mqty(…), \mqty*(…) (\lgroup…\rgroup), \mqty[…], \mqty|…|, \mqty{…} (no delimiters); \mqty
            // alone is "()", as in the package. What follows the first form is read as written
            "mqty" | "matrixquantity" | "smqty" | "smallmatrixquantity" => {
                let small = name.starts_with('s');
                let star = starred(self);
                self.skip_spaces();
                let (shape, body) = match self.current() {
                    Some('(') => (if star { 'P' } else { '(' }, self.raw_delimited('(', ')')),
                    Some('[') => ('[', self.raw_delimited('[', ']')),
                    Some('|') => ('|', self.raw_delimited('|', '|')),
                    Some('{') => ('{', self.raw_text()?),
                    _ => return Ok(Some("()".into())),
                };
                physics_matrix(small, shape, &body)
            }
            "pmqty" | "Pmqty" | "bmqty" | "vmqty" | "mdet" | "matrixdeterminant" | "spmqty" | "sPmqty" | "sbmqty" | "svmqty" | "smdet" | "smallmatrixdeterminant" => {
                let small = name.starts_with("sm") || (name.len() == 6 && name.starts_with('s'));
                let shape = match name.trim_start_matches('s').chars().next() {
                    Some('p') => '(',
                    Some('P') => 'P',
                    Some('b') => '[',
                    _ => '|',
                };
                physics_matrix(small, shape, &self.raw_text()?)
            }
            // the generated contents, to be put in a matrix: \pmqty{\imat{2}}
            "imat" | "identitymatrix" => {
                let n = self.matrix_size(name, col)?;
                generated_matrix(n, n, |i, j| if i == j { "1".into() } else { "0".into() })
            }
            "xmat" | "xmatrix" | "zmat" | "zeromatrix" => {
                let indexed = name.starts_with('x') && starred(self);
                let x = if name.starts_with('x') { self.raw_text()? } else { "0".into() };
                let rows = self.matrix_size(name, col)?;
                // \zeromatrix{m}: square
                let columns = match name.starts_with('z') {
                    true => match self.optional_braced()? {
                        Some(raw) => self.size_of(name, &raw, col)?,
                        None => rows,
                    },
                    false => self.matrix_size(name, col)?,
                };
                generated_matrix(rows, columns, |i, j| match indexed {
                    true => format!("{x}_{{{}{}}}", if rows > 1 { (i + 1).to_string() } else { String::new() }, if columns > 1 { (j + 1).to_string() } else { String::new() }),
                    false => x.clone(),
                })
            }
            "paulimatrix" | "pmat" => {
                let which = self.raw_text()?;
                match which.trim() {
                    "0" => generated_matrix(2, 2, |i, j| if i == j { "1".into() } else { "0".into() }),
                    "1" | "x" => PAULI[0].into(),
                    "2" | "y" => PAULI[1].into(),
                    "3" | "z" => PAULI[2].into(),
                    other => {
                        self.tolerate(rules::E_REC_9, col, format!("\\{name}{{{other}}}: no Pauli matrix by that name (0 to 3, x, y or z)"), "left empty")?;
                        String::new()
                    }
                }
            }
            "paulixmatrix" => PAULI[0].into(),
            "pauliymatrix" => PAULI[1].into(),
            "paulizmatrix" => PAULI[2].into(),
            // \dmat[filler]{a,b,c}: the entries on the diagonal (\admat: the antidiagonal), the filler
            // elsewhere; an entry with its own rows is a matrix, as the package's \mqty{…} makes it
            "dmat" | "diagonalmatrix" | "admat" | "antidiagonalmatrix" => {
                let filler = self.raw_optional().unwrap_or_default();
                let entries = split_top_level(&self.raw_text()?, ',');
                let n = entries.len();
                let anti = name.starts_with('a');
                generated_matrix(n, n, |i, j| match (anti && j == n - 1 - i) || (!anti && i == j) {
                    true if entries[i].contains('&') || entries[i].contains("\\\\") => format!("\\begin{{matrix}}{}\\end{{matrix}}", entries[i]),
                    true => entries[i].clone(),
                    false => filler.clone(),
                })
            }
            // spaced text: \qq{text} is \quad\text{text}\quad, without the first \quad when starred
            "qq" | "qqtext" => {
                let lead = if starred(self) { "" } else { "\\quad" };
                format!("{lead}\\text{{{}}}\\quad ", self.raw_text()?)
            }
            "qc" | "qcomma" => ",\\quad ".into(),
            "qcc" | "qif" | "qthen" | "qelse" | "qotherwise" | "qunless" | "qgiven" | "qusing" | "qassume" | "qsince" | "qlet" | "qfor" | "qall" | "qeven" | "qodd"
            | "qinteger" | "qand" | "qor" | "qas" | "qin" => {
                let lead = if starred(self) { "" } else { "\\quad" };
                let word = if name == "qcc" { "c.c." } else { &name[1..] };
                format!("{lead}\\text{{{word}}}\\quad ")
            }
            // the variation δ, written like the differential \dd
            "var" | "variation" => {
                let order = self.raw_optional();
                let power = order.map(|n| format!("^{{{n}}}")).unwrap_or_default();
                let x = self.optional_braced()?.unwrap_or_default();
                format!("\\mathop{{}}\\!\\delta{power} {x} ")
            }
            "pv" | "principalvalue" | "PV" => {
                let symbol = if name == "PV" { "\\mathrm{P.V.}" } else { "\\mathcal{P}" };
                let x = self.optional_braced()?.map(|x| format!("{{{x}}}")).unwrap_or_default();
                format!("{symbol}{x}")
            }
            "op" => "\\dyad".into(),
            // the vacuum expectation value ⟨0|x|0⟩
            "vev" => format!("\\expval{{{}}}{{0}}", self.raw_text()?),
            // the same flat fraction as \dv* (#157)
            "flatfrac" => {
                let a = self.raw_text()?;
                let b = self.raw_text()?;
                physics_fraction(true, a, b)
            }
            "Trace" => "\\operatorname{Tr}".into(),
            "Residue" => "\\operatorname{Res}".into(),
            "asin" | "acos" | "atan" | "acsc" | "asec" | "acot" => format!("\\operatorname{{{name}}}"),
            "asine" | "acosine" | "atangent" | "acosecant" | "asecant" | "acotangent" => format!("\\operatorname{{a{}}}", physics_long_name(&name[1..]).unwrap_or_default()),
            // physics' bold nabla (its default, bolddel), as \grad draws it
            "vnabla" | "gradientnabla" => "\\unicode{x1D6C1} ".into(),
            // the upright d of the package's default (uprightdiff)
            "diffd" => "\\mathrm{d}".into(),
            // physics' \real and \imaginary are LaTeX's \Re and \Im (ℜ, ℑ), kept by the package when it
            // redefines those two as the operators Re and Im, which contextEquation does not do
            "real" => "\\Re ".into(),
            "imaginary" => "\\Im ".into(),
            // a long name (\sine): what is left once the names in `PHYSICS` have their arms
            long => return Ok(physics_long_name(long).map(|short| format!("\\{short} "))),
        }))
    }

    /// The number of rows or columns of a generated physics matrix.
    fn matrix_size(&mut self, name: &str, col: usize) -> R<usize> {
        let raw = self.raw_text()?;
        self.size_of(name, &raw, col)
    }

    fn size_of(&self, name: &str, raw: &str, col: usize) -> R<usize> {
        match raw.trim().parse::<usize>() {
            Ok(n) if (1..=64).contains(&n) => Ok(n),
            _ => self.error(col, format!("\\{name} needs a size from 1 to 64, not {:?}", raw.trim())),
        }
    }
}

impl Reader<'_> {
    /// A command contextEquation does not know. In tolerant mode: a `\n` that came from JSON becomes a line break,
    /// `²` becomes the character, `\5` becomes the digit, and the rest shows up in red.
    fn unknown_command(&mut self, name: &str, start: usize, col: usize, items: &mut Vec<Node>) -> R<Option<Stop>> {
        if self.strict {
            return self.error(col, format!("unsupported command: \\{name}"));
        }
        if let Some((escape, remainder)) = self.json_escape(name) {
            let read = if escape == 't' { "a space" } else { "a line break" };
            self.tolerate(rules::E_REC_2, col, format!("\\{name}"), &format!("JSON '\\{escape}' read as {read}"))?;
            self.inject(start, &format!(" {remainder}"), col)?;
            return Ok(None);
        }
        if name == "u" {
            let hex: String = self.c[self.pos..].iter().take(4).collect();
            if let Some(c) = u32::from_str_radix(&hex, 16).ok().filter(|_| hex.len() == 4).and_then(char::from_u32) {
                self.tolerate(rules::E_REC_2, col, format!("\\u{hex} is a JSON escape"), &format!("read as '{c}'"))?;
                self.pos += 4;
                self.inject(start, &c.to_string(), col)?;
                return Ok(None);
            }
        }
        if let Some(accent) = self.text_accent_in_math(name) {
            self.tolerate(rules::E_REC_1, col, format!("text accent '\\{name}' in math"), &format!("read as \\{accent}"))?;
            self.inject(start, &format!("\\{accent} "), col)?;
            return Ok(None);
        }
        if name.chars().count() == 1 && !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            self.tolerate(rules::E_REC_1, col, format!("'\\{name}'"), "backslash before a symbol ignored")?;
            self.inject(start, name, col)?;
            if matches!(name, "\"") {
                self.pos += 1;
            }
            return Ok(None);
        }
        self.tolerate(rules::E_REC_1, col, format!("unsupported command: \\{name}"), "drawn in red")?;
        items.push(Node::Colored { color: "#c0392b".into(), base: Box::new(Node::Text { parts: vec![TextPart::Text(format!("\\{name}"))] }) });
        Ok(None)
    }
}

/// physics' brackets around `parts`, separated by `\middle{middle}`. Unstarred, `\left…\right` sizes them
/// to the content; starred, they keep the text size: the parts are smashed and a `\vphantom` keeps their
/// height, as the package's `\braces` does.
fn physics_fence(star: bool, open: &str, middle: &str, close: &str, parts: &[&str]) -> String {
    let separator = format!(" \\middle{middle} ");
    if star {
        let smashed: Vec<String> = parts.iter().map(|p| format!("\\smash{{{p}}}")).collect();
        format!("\\vphantom{{{}}}\\left{open} {} \\right{close}", parts.join(" "), smashed.join(&separator))
    } else {
        format!("\\left{open} {} \\right{close}", parts.join(&separator))
    }
}

/// A physics derivative: `\frac`, or starred, physics' `\flatfrac`, `\left.a\middle/b\right.`: a slash at
/// the text size between the two (TeX keeps the slash at its size around letters and their exponents).
fn physics_fraction(flat: bool, numerator: String, denominator: String) -> String {
    if flat {
        format!("{{{numerator}}}/{{{denominator}}}")
    } else {
        format!("\\frac{{{numerator}}}{{{denominator}}}")
    }
}

impl Reader<'_> {
    /// #213: a text accent (`\'`, `` \` ``, `\^`, `\"`, `\~`, `\=`, `\.`) before a letter in math, an error
    /// in TeX: the math accent KaTeX reads it as (`\'e` is `\acute e`).
    #[inline(never)]
    fn text_accent_in_math(&self, name: &str) -> Option<&'static str> {
        let accent = match name {
            "'" => "acute",
            "`" => "grave",
            "^" => "hat",
            "\"" => "ddot",
            "~" => "tilde",
            "=" => "bar",
            "." => "dot",
            _ => return None,
        };
        let next = self.c[self.pos..].iter().find(|c| !matches!(c, ' ' | '{'))?;
        next.is_alphabetic().then_some(accent)
    }
}

/// LaTeX text commands of one or two letters after an n, r or t: never a JSON escape (`\th` is þ).
const SHORT_TEXT_COMMANDS: [&str; 3] = ["th", "ng", "rq"];

impl Reader<'_> {
    /// E-REC-2: an unknown command `\n…`, `\r…` or `\t…` that is really a JSON escape left in the text (the
    /// escape and the letters after it), as `spec/tolerance.md` defines it: the letter alone (`x\n2`), never
    /// a command; letters after it (`x\nb`, `a\tx`, `\nwhere`) only when the input shows another sign of
    /// JSON (#253, [`json_evidence`]), and not when the name is a LaTeX text command (`\th`). Otherwise the
    /// name is a command contextEquation does not know (`\nm`, `\real`, `\rowcolor`): E-REC-1.
    #[inline(never)]
    fn json_escape<'n>(&self, name: &'n str) -> Option<(char, &'n str)> {
        let escape = name.chars().next().filter(|c| matches!(c, 'n' | 'r' | 't'))?;
        let remainder = &name[1..];
        let signs = self.macros.json_text?;
        let json = match remainder.chars().count() {
            0 => true,
            1 | 2 => signs && !SHORT_TEXT_COMMANDS.contains(&name),
            _ => signs,
        };
        json.then_some((escape, remainder))
    }
}

/// Does the input (`original`, as it came) show a sign of JSON ([`json_evidence`])? `None` when the text
/// read (`c`) has a real tab or line break: a JSON string never has one, so the text is not JSON and shows
/// no escape (#208).
fn json_signs(original: &str, c: &[char]) -> Option<bool> {
    if c.iter().any(|x| matches!(x, '\t' | '\r' | '\n')) {
        return None;
    }
    Some(json_evidence(&original.chars().collect::<Vec<_>>()))
}

/// #253: does the text show a sign of JSON that hand-written LaTeX never has? A control character JSON
/// makes of a backslash and a letter that no one types (form feed from `\frac`, backspace from `\beta`,
/// vertical tab from `\vec`, bell from `\alpha`), `\"`, or `\u` and four hexadecimal digits. A tab or a
/// line break is not one (an author types them), nor is `\n`, `\r` or `\t` itself: a lost backslash is
/// rebuilt from them only when the input shows one of these.
fn json_evidence(c: &[char]) -> bool {
    if c.iter().any(|x| matches!(x, '\u{0C}' | '\u{08}' | '\u{0B}' | '\u{07}')) {
        return true;
    }
    let mut i = 0;
    while i + 1 < c.len() {
        if c[i] != '\\' {
            i += 1;
            continue;
        }
        let next = c[i + 1];
        let escape = match next {
            '"' => true,
            'u' => c.len() >= i + 6 && c[i + 2..i + 6].iter().all(char::is_ascii_hexdigit),
            _ => false,
        };
        if escape {
            return true;
        }
        // a doubled backslash is one token (a line break, or an escaped backslash)
        i += if next == '\\' { 2 } else { 1 };
    }
    false
}

/// Does contextEquation know this command? Used to rebuild a command whose backslash JSON ate (`repair_controls`).
fn known(name: &str) -> bool {
    symbols::command(name).is_some()
        || symbols::greek(name).is_some()
        || symbols::function(name).is_some()
        || symbols::large_operator(name).is_some()
        || symbols::accent(name).is_some()
        || matches!(
            name,
            "begin"
                | "end"
                | "binom"
                | "dbinom"
                | "cfrac"
                | "textbf"
                | "textit"
                | "textrm"
                | "mathit"
                | "boldsymbol"
                | "mathbold"
                | "bmatrix"
                | "pmatrix"
                | "vmatrix"
                | "overbrace"
                | "underbrace"
                | "widehat"
                | "tilde"
                | "dot"
                | "ddot"
                | "big"
                | "Big"
                | "bigg"
                | "Bigg"
                | "newline"
                | "not"
                | "centernot"
                | "notin"
                | "neq"
                | "nabla"
                | "frac"
                | "sqrt"
                | "text"
                | "mathrm"
                | "mathbf"
                | "mathbb"
                | "mathcal"
                | "left"
                | "right"
                | "cdot"
                | "times"
                | "quad"
                | "qquad"
                | "vec"
                | "hat"
                | "bar"
                | "cal"
                | "bf"
                | "rm"
                | "it"
                | "tt"
                | "sf"
                | "overline"
                | "underline"
                | "mathsf"
                | "mathtt"
                | "boxed"
                | "cancel"
                | "dfrac"
                | "tfrac"
        )
}

/// `\\frac{\\sqrt{3}}{2}`: every command comes with a doubled backslash. Undoes one level of escaping,
/// as long as there is no command with a single backslash (then the double backslashes are real line breaks).
pub fn undo_doubled_backslashes(s: &str) -> Option<String> {
    let c: Vec<char> = s.chars().collect();
    let mut doubles = 0;
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' {
            let mut n = 0;
            while i < c.len() && c[i] == '\\' {
                n += 1;
                i += 1;
            }
            if c.get(i).is_some_and(|x| x.is_ascii_alphabetic()) {
                if n % 2 == 1 {
                    return None;
                }
                doubles += 1;
            }
        } else {
            i += 1;
        }
    }
    if doubles == 0 {
        return None;
    }
    let mut output = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' {
            let mut n: usize = 0;
            while i < c.len() && c[i] == '\\' {
                n += 1;
                i += 1;
            }
            output.extend(std::iter::repeat_n('\\', n.div_ceil(2)));
        } else {
            output.push(c[i]);
            i += 1;
        }
    }
    Some(output)
}

/// `\frac` stored in JSON without escaping the backslash becomes form feed + "rac"; `\begin` becomes backspace + "egin";
/// `\times`, `\right`, `\neq` become tab, CR and line feed. Rebuilds the command when the letters that follow
/// it make a known command (KaTeX's texvc aliases aside: a tab before `h` is not `\th`); otherwise the control
/// character becomes a space. A tab or a line break is whitespace, as in TeX, unless the input looks
/// JSON-decoded ([`json_decoded`]) and shows another sign of JSON ([`json_evidence`], #253); the other
/// control characters are never the author's whitespace.
fn repair_controls(s: &str) -> Option<String> {
    let letter_of = |c: char| match c {
        '\u{0C}' => Some('f'),
        '\u{08}' => Some('b'),
        '\u{0B}' => Some('v'),
        '\u{07}' => Some('a'),
        '\t' => Some('t'),
        '\r' => Some('r'),
        '\n' => Some('n'),
        _ => None,
    };
    if !s.chars().any(|c| letter_of(c).is_some()) {
        return None;
    }
    let c: Vec<char> = s.chars().collect();
    let whitespace_rebuilt = json_decoded(&c) && json_evidence(&c);
    let mut output = String::with_capacity(s.len() + 8);
    let mut changed = false;
    for (i, ch) in c.iter().enumerate() {
        let letter = letter_of(*ch).filter(|_| whitespace_rebuilt || !matches!(ch, '\t' | '\r' | '\n'));
        let Some(letter) = letter else {
            output.push(*ch);
            continue;
        };
        let following: String = c[i + 1..].iter().take_while(|x| x.is_ascii_alphabetic()).collect();
        let name = format!("{letter}{following}");
        // the whole run of letters, as TeX reads a command: "and" + line break + "exhaust" is not \ne xhaust
        let rebuild = name.len() > 1 && known(&name) && !TEXVC_ALIASES.contains(&name.as_str());
        if rebuild {
            output.push('\\');
            output.push(letter);
            changed = true;
        } else {
            output.push(' ');
        }
    }
    changed.then_some(output)
}

/// KaTeX's texvc aliases a lost backslash could rebuild (#172): short names that ordinary text after a tab
/// or a line break also makes (`\th` from a tab and `h`).
const TEXVC_ALIASES: [&str; 9] = ["alef", "alefsym", "arcctg", "bull", "rang", "rarr", "rArr", "real", "th"];

/// Does the text look JSON-decoded with its backslashes lost (#208)? Only when every tab and line break
/// stands where a lost backslash would, right before a letter (none ends a line, makes a blank line or
/// indents the next one; the whitespace that ends the input aside), and no command starting with `n`, `r`
/// or `t` kept its backslash: JSON would have eaten that one too (`\to` becomes tab + "o").
fn json_decoded(c: &[char]) -> bool {
    let end = c.iter().rposition(|x| !x.is_whitespace()).map_or(0, |e| e + 1);
    let c = &c[..end];
    let mut i = 0;
    while i < c.len() {
        let next = c.get(i + 1).copied();
        match c[i] {
            '\t' | '\r' | '\n' if !next.is_some_and(|n| n.is_ascii_alphabetic()) => return false,
            '\\' if matches!(next, Some('n' | 'r' | 't')) => return false,
            // a doubled backslash is one token (a line break, or an escaped backslash)
            '\\' if next == Some('\\') => i += 1,
            _ => {}
        }
        i += 1;
    }
    true
}

/// The input without the whitespace around it, except the space of a control space that ends it (#254):
/// `a\ ` keeps its `\ `, which would otherwise be a lone backslash.
#[inline(never)]
fn trim_keeping_control_space(s: &str) -> String {
    let trimmed = s.trim();
    let backslashes = trimmed.chars().rev().take_while(|c| *c == '\\').count();
    if backslashes % 2 == 1 && s.trim_start().len() > trimmed.len() {
        format!("{trimmed} ")
    } else {
        trimmed.to_string()
    }
}

/// #211: a backslash before a line break or a tab is a control space, `\ `, as in plain TeX (`\\` before
/// a line break is a line break and then whitespace).
#[inline(never)]
fn control_spaces(s: &mut String) {
    if !s.contains(['\n', '\r', '\t']) {
        return;
    }
    let mut backslashes = 0;
    *s = s
        .chars()
        .map(|c| {
            let odd = backslashes % 2 == 1;
            backslashes = if c == '\\' { backslashes + 1 } else { 0 };
            if odd && matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .collect();
}

/// The plain character of a Unicode superscript or subscript: every character whose Unicode
/// decomposition is `<super>` or `<sub>` of a Latin or Greek letter, a digit, `+ − = ( )`. The
/// ordinal indicators `ª` and `º` are left out: they stand for other things (`º` for a degree).
fn plain_script_char(c: char) -> Option<char> {
    Some(match c {
        '⁰' | '₀' => '0',
        '¹' | '₁' => '1',
        '²' | '₂' => '2',
        '³' | '₃' => '3',
        '⁴' | '₄' => '4',
        '⁵' | '₅' => '5',
        '⁶' | '₆' => '6',
        '⁷' | '₇' => '7',
        '⁸' | '₈' => '8',
        '⁹' | '₉' => '9',
        '⁺' | '₊' => '+',
        '⁻' | '₋' => '-',
        '⁼' | '₌' => '=',
        '⁽' | '₍' => '(',
        '⁾' | '₎' => ')',
        'ᵃ' | 'ₐ' => 'a',
        'ᵇ' => 'b',
        'ᶜ' => 'c',
        'ᵈ' => 'd',
        'ᵉ' | 'ₑ' => 'e',
        'ᶠ' => 'f',
        'ᵍ' => 'g',
        'ʰ' | 'ₕ' => 'h',
        'ⁱ' | 'ᵢ' => 'i',
        'ʲ' | 'ⱼ' => 'j',
        'ᵏ' | 'ₖ' => 'k',
        'ˡ' | 'ₗ' => 'l',
        'ᵐ' | 'ₘ' => 'm',
        'ⁿ' | 'ₙ' => 'n',
        'ᵒ' | 'ₒ' => 'o',
        'ᵖ' | 'ₚ' => 'p',
        '𐞥' => 'q',
        'ʳ' | 'ᵣ' => 'r',
        'ˢ' | 'ₛ' => 's',
        'ᵗ' | 'ₜ' => 't',
        'ᵘ' | 'ᵤ' => 'u',
        'ᵛ' | 'ᵥ' => 'v',
        'ʷ' => 'w',
        'ˣ' | 'ₓ' => 'x',
        'ʸ' => 'y',
        'ᶻ' => 'z',
        'ᴬ' => 'A',
        'ᴮ' => 'B',
        'ꟲ' => 'C',
        'ᴰ' => 'D',
        'ᴱ' => 'E',
        'ꟳ' => 'F',
        'ᴳ' => 'G',
        'ᴴ' => 'H',
        'ᴵ' => 'I',
        'ᴶ' => 'J',
        'ᴷ' => 'K',
        'ᴸ' => 'L',
        'ᴹ' => 'M',
        'ᴺ' => 'N',
        'ᴼ' => 'O',
        'ᴾ' => 'P',
        'ꟴ' => 'Q',
        'ᴿ' => 'R',
        'ᵀ' => 'T',
        'ᵁ' => 'U',
        'ⱽ' => 'V',
        'ᵂ' => 'W',
        'ᵝ' | 'ᵦ' => 'β',
        'ᵞ' | 'ᵧ' => 'γ',
        'ᵟ' => 'δ',
        'ᶿ' => 'θ',
        'ᵨ' => 'ρ',
        'ᵠ' | 'ᵩ' => 'φ',
        'ᵡ' | 'ᵪ' => 'χ',
        _ => return None,
    })
}

/// The characters of [`plain_script_char`] that are subscripts (the rest are superscripts).
fn is_subscript_char(c: char) -> bool {
    matches!(c, '\u{2080}'..='\u{209C}' | '\u{1D62}'..='\u{1D6A}' | '\u{2C7C}')
}

/// TeX's comments: an unescaped `%` drops the rest of its line, the line break and the spaces that
/// start the next line. Returns the text without them and the column (1-based, in `s`) of each `%`.
/// The verbatim arguments of `\verb`, `\url` and `\href` keep their `%`, as in TeX.
fn comments(s: &str) -> (String, Vec<usize>) {
    let c: Vec<char> = s.chars().collect();
    let mut output = String::with_capacity(s.len());
    let mut starts = Vec::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => {
                let start = i;
                i += 1;
                while c.get(i).is_some_and(char::is_ascii_alphabetic) {
                    i += 1;
                }
                if i == start + 1 && i < c.len() {
                    i += 1;
                }
                let name: String = c[start + 1..i].iter().collect();
                output.extend(&c[start..i]);
                match name.as_str() {
                    "verb" => {
                        if c.get(i) == Some(&'*') {
                            output.push('*');
                            i += 1;
                        }
                        if let Some(&delim) = c.get(i) {
                            let end = c[i + 1..].iter().position(|&x| x == delim).map_or(c.len(), |p| p + i + 2);
                            output.extend(&c[i..end]);
                            i = end;
                        }
                    }
                    "url" | "href" => {
                        while c.get(i).is_some_and(|x| x.is_whitespace()) {
                            output.push(c[i]);
                            i += 1;
                        }
                        if c.get(i) == Some(&'{') {
                            let end = (group_end(&c, i + 1) + 1).min(c.len());
                            output.extend(&c[i..end]);
                            i = end;
                        }
                    }
                    _ => {}
                }
            }
            '%' => {
                starts.push(i + 1);
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                i += 1;
                while c.get(i).is_some_and(|x| *x == ' ' || *x == '\t') {
                    i += 1;
                }
            }
            other => {
                output.push(other);
                i += 1;
            }
        }
    }
    (output, starts)
}

/// #204: the code point of `\unicode{…}`, as MathJax reads it: decimal (`8918`), hexadecimal after `x` or
/// `0x` (`x22D6`, `0x22D6`); `U+22D6` and `#22D6` stay hexadecimal, as contextEquation read them before.
#[inline(never)]
fn unicode_code_point(raw: &str) -> Option<char> {
    let code = raw.trim();
    let hex = ["0x", "0X", "x", "X", "U+", "u+", "#"].iter().find_map(|p| code.strip_prefix(p));
    match hex {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => code.parse::<u32>().ok(),
    }
    .and_then(char::from_u32)
}
