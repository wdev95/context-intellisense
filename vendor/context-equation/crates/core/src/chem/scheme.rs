//! The reaction scheme reader (L-CHE-2): `\schemestart … \schemestop` with `\arrow`, `\+`,
//! `\chemname`, `\subscheme` and `\chemmove`, and the compounds in it (`\chemfig{…}`, `\ce{…}`,
//! `$…$`, loose formulas, material between `\chemleft…\chemright` or `\chemup…\chemdown`). The
//! LaTeX reader hands it the whole construction as written.

use super::chemfig;
use super::model::*;
use crate::rules::{self, Registry};
use crate::Error;

pub type R<T> = Result<T, Error>;

/// Shared by the chemistry readers: strict mode, the warnings of tolerant mode, and the column of the
/// command the construction started at (errors point there).
pub struct Reader<'w> {
    pub strict: bool,
    pub registry: &'w mut Registry,
    pub column: usize,
    /// The `\definesubmol`s the input made before this construction.
    pub submols: &'w Submols,
}

impl Reader<'_> {
    /// In strict mode, an error; in tolerant mode, a warning, and the caller's recovery goes on.
    pub fn tolerate(&mut self, rule: &'static str, problem: impl Into<String>, recovery: &str) -> R<()> {
        let problem = problem.into();
        if self.strict {
            return Err(Error::Syntax { column: self.column, message: problem });
        }
        self.registry.warn(rule, format!("column {}: {problem}; {recovery}", self.column));
        Ok(())
    }
}

/// LaTeX text (arrow labels, compound names) as math for contextEquation: `$…$` stays math, `\ce{…}` and other
/// commands are math as in LaTeX's text mode, and the words around them go in `\text{…}`.
pub fn text_to_tex(t: &str) -> String {
    let c: Vec<char> = t.trim().chars().collect();
    let mut out = String::new();
    let mut words = String::new();
    let flush = |words: &mut String, out: &mut String| {
        if !words.is_empty() {
            out.push_str(&format!("\\text{{{words}}}"));
            words.clear();
        }
    };
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '$' => {
                flush(&mut words, &mut out);
                i += 1;
                while i < c.len() && c[i] != '$' {
                    out.push(c[i]);
                    i += 1;
                }
                i += 1;
            }
            // a mark around text (\htmlData{m=a}{heat}): the text inside stays text, the mark around it
            '\\' if text_mark(&c[i..]).is_some() => {
                flush(&mut words, &mut out);
                let (command, value, inner, len) = text_mark(&c[i..]).unwrap_or_default();
                out.push_str(&format!("\\{command}{{{value}}}{{{}}}", text_to_tex(&inner)));
                i += len;
            }
            '\\' if c.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic()) => {
                flush(&mut words, &mut out);
                out.push('\\');
                i += 1;
                while i < c.len() && c[i].is_ascii_alphabetic() {
                    out.push(c[i]);
                    i += 1;
                }
                // the command's braced arguments go with it
                while i < c.len() && c[i] == '{' {
                    let mut level = 0;
                    while i < c.len() {
                        out.push(c[i]);
                        match c[i] {
                            '{' => level += 1,
                            '}' => {
                                level -= 1;
                                if level == 0 {
                                    i += 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                out.push(' ');
            }
            ch => {
                words.push(ch);
                i += 1;
            }
        }
    }
    flush(&mut words, &mut out);
    out.trim_end().to_string()
}

/// A mark command at the start of `c` (`\htmlData{value}{inner}`, `\class`, `\htmlId`…): its name,
/// value, inner text and length in characters.
fn text_mark(c: &[char]) -> Option<(String, String, String, usize)> {
    let name: String = c.get(1..)?.iter().take_while(|x| x.is_ascii_alphabetic()).collect();
    if !matches!(name.as_str(), "htmlData" | "class" | "htmlClass" | "htmlId" | "cssId") {
        return None;
    }
    let mut i = 1 + name.len();
    let arg = |i: &mut usize| -> Option<String> {
        if c.get(*i) != Some(&'{') {
            return None;
        }
        let mut level = 0;
        let start = *i + 1;
        while *i < c.len() {
            match c[*i] {
                '{' => level += 1,
                '}' => {
                    level -= 1;
                    if level == 0 {
                        *i += 1;
                        return Some(c[start..*i - 1].iter().collect());
                    }
                }
                _ => {}
            }
            *i += 1;
        }
        None
    };
    let value = arg(&mut i)?;
    let inner = arg(&mut i)?;
    Some((name, value, inner, i))
}

/// Splits at `sep` outside braces and brackets.
pub fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut level = 0i32;
    for c in s.chars() {
        match c {
            '{' | '[' | '(' => level += 1,
            '}' | ']' | ')' => level -= 1,
            x if x == sep && level == 0 => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        parts.last_mut().expect("never empty").push(c);
    }
    parts
}

/// The hook a TikZ point of `\chemmove` names: `name`, `name.anchor`, or `[yshift=7pt]name.180` with
/// the point's own move, which contextEquation leaves out (the arrow ends at the hook).
fn hook_of(point: &str) -> String {
    let p = point.trim();
    let p = match p.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map_or(p, |(_, after)| after.trim()),
        None => p,
    };
    p.split('.').next().unwrap_or("").trim().to_string()
}

/// A compound named in an arrow: its name, its anchor's angle, and its move in em.
type NodeSpec = (String, Option<f32>, Option<(f32, f32)>);

/// Reads a scheme, or a single `\chemfig{…}`, as the LaTeX reader found it.
pub fn read(src: &str, reader: &mut Reader) -> R<Scheme> {
    let mut s = SchemeReader { c: src.trim().chars().collect(), pos: 0, reader, scheme: Scheme { items: vec![], moves: vec![] }, names: vec![], arrows: (0.0, 1.0), boxes: vec![] };
    s.items()?;
    let mut scheme = s.scheme;
    // a mechanism arrow must point at hooked atoms
    let hooks: Vec<String> = scheme
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Compound(Compound { body: Body::Structure(m), .. }) => Some(m),
            _ => None,
        })
        .flat_map(|m| m.atoms.iter().filter_map(|a| a.hook.clone()).chain(m.bonds.iter().filter_map(|b| b.hook.as_ref().map(|h| h.0.clone()))))
        .collect();
    let mut kept = Vec::new();
    for m in std::mem::take(&mut scheme.moves) {
        let missing = [&m.from, &m.to].into_iter().find(|h| !hooks.contains(h)).cloned();
        match missing {
            Some(missing) => reader.tolerate(rules::E_REC_9, format!("no atom is hooked as @{{{missing}}}"), "arrow left out")?,
            None => kept.push(m),
        }
    }
    scheme.moves = kept;
    Ok(scheme)
}

struct SchemeReader<'r, 'w> {
    c: Vec<char>,
    pos: usize,
    reader: &'r mut Reader<'w>,
    scheme: Scheme,
    /// Compounds named by `\arrow(a--b)` or by `\merge…--(b)` (chemfig manual, sections 6.4 and 6.11),
    /// with their index among the scheme's compounds; the last one given for a name counts.
    names: Vec<(String, usize)>,
    /// The angle and length coefficient of an arrow that gives none: `\schemestart[arrow angle=…,arrow
    /// coeff=…]` (chemfig manual, section 6.1); 0 and 1 by default.
    arrows: (f32, f32),
    /// Each `\subscheme`'s compounds, as the index of its first and of the one after its last: chemfig
    /// names its box too, with the number before its first compound's (manual, section 6.8), so `ck`
    /// counts them, and an arrow can leave from a box.
    boxes: Vec<(usize, usize)>,
}

impl SchemeReader<'_, '_> {
    fn current(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn group(&mut self, open: char, close: char) -> R<String> {
        let mut level = 0;
        let mut s = String::new();
        while let Some(ch) = self.current() {
            self.pos += 1;
            if ch == '\\' && level > 0 {
                s.push(ch);
                if let Some(n) = self.current() {
                    s.push(n);
                    self.pos += 1;
                }
                continue;
            }
            if ch == open {
                level += 1;
                if level == 1 {
                    continue;
                }
            } else if ch == close {
                level -= 1;
                if level == 0 {
                    return Ok(s);
                }
            }
            s.push(ch);
        }
        self.reader.tolerate(rules::E_REC_4, format!("'{open}' without a matching '{close}'"), "closed at the end")?;
        Ok(s)
    }

    fn braced(&mut self) -> R<String> {
        self.skip_spaces();
        if self.current() == Some('{') {
            self.group('{', '}')
        } else {
            self.reader.tolerate(rules::E_REC_4, "missing braced argument", "left empty")?;
            Ok(String::new())
        }
    }

    fn optional(&mut self) -> R<Option<String>> {
        let saved = self.pos;
        self.skip_spaces();
        if self.current() == Some('[') {
            return Ok(Some(self.group('[', ']')?));
        }
        self.pos = saved;
        Ok(None)
    }

    fn command(&mut self) -> String {
        self.pos += 1;
        let mut name = String::new();
        while let Some(l) = self.current().filter(char::is_ascii_alphabetic) {
            name.push(l);
            self.pos += 1;
        }
        if name.is_empty() {
            if let Some(sym) = self.current() {
                name.push(sym);
                self.pos += 1;
            }
        }
        name
    }

    fn compound(&mut self, body: Body, name: Option<String>) {
        self.scheme.items.push(Item::Compound(Compound { body, name }));
    }

    fn items(&mut self) -> R<()> {
        let mut text = String::new();
        while let Some(ch) = self.current() {
            if ch != '\\' && ch != '$' {
                text.push(ch);
                self.pos += 1;
                continue;
            }
            self.flush(&mut text);
            if ch == '$' {
                self.pos += 1;
                let mut math = String::new();
                while let Some(m) = self.current() {
                    self.pos += 1;
                    if m == '$' {
                        break;
                    }
                    math.push(m);
                }
                self.compound(Body::Math { tex: math.trim().to_string() }, None);
                continue;
            }
            let start = self.pos;
            let name = self.command();
            match name.as_str() {
                "schemestart" => {
                    if let Some(options) = self.optional()? {
                        self.scheme_options(&options);
                    }
                }
                "schemestop" | "par" => {}
                "chemfig" => {
                    let _ = self.optional()?;
                    let body = self.braced()?;
                    let m = chemfig::molecule(&body, self.reader)?;
                    self.compound(Body::Structure(m), None);
                }
                "ce" => {
                    let body = self.braced()?;
                    self.compound(Body::Math { tex: format!("\\ce{{{body}}}") }, None);
                }
                "chemname" => {
                    if self.current() == Some('*') {
                        self.pos += 1;
                    }
                    let _ = self.optional()?;
                    let inner = self.braced()?;
                    let label = self.braced()?;
                    let before = self.scheme.items.len();
                    let mut sub = SchemeReader {
                        c: inner.chars().collect(),
                        pos: 0,
                        reader: self.reader,
                        scheme: Scheme { items: vec![], moves: vec![] },
                        names: vec![],
                        arrows: (0.0, 1.0),
                        boxes: vec![],
                    };
                    sub.items()?;
                    let (items, moves) = (sub.scheme.items, sub.scheme.moves);
                    self.scheme.moves.extend(moves);
                    self.scheme.items.extend(items);
                    match self.scheme.items.get_mut(before) {
                        Some(Item::Compound(c)) => c.name = Some(label),
                        _ => self.compound(Body::Math { tex: String::new() }, Some(label)),
                    }
                }
                "+" => {
                    // \+{space before, space after, vertical shift}: the spacing is contextEquation's
                    if self.current() == Some('{') {
                        let _ = self.group('{', '}')?;
                    }
                    self.scheme.items.push(Item::Plus);
                }
                "arrow" => self.arrow()?,
                // chemfig manual, section 6.11
                "merge" => self.merge()?,
                // section 6.8: delimiters around material, as math the LaTeX reader draws
                "chemleft" | "chemup" => {
                    let closer = if name == "chemleft" { "chemright" } else { "chemdown" };
                    let end = delimited_end(&self.c, self.pos, &name, closer);
                    let tex: String = self.c[start..end].iter().collect();
                    self.pos = end;
                    self.compound(Body::Math { tex }, None);
                }
                "chemmove" => {
                    let _ = self.optional()?;
                    let body = self.braced()?;
                    self.moves(&body)?;
                }
                "subscheme" => {
                    let _ = self.optional()?;
                    let body = self.braced()?;
                    let arrows = self.arrows;
                    let first = self.compounds();
                    let mut sub = SchemeReader {
                        c: body.chars().collect(),
                        pos: 0,
                        reader: self.reader,
                        scheme: Scheme { items: vec![], moves: vec![] },
                        names: vec![],
                        arrows,
                        boxes: vec![],
                    };
                    sub.items()?;
                    // the names given inside are the scheme's (a \merge after it can join them)
                    self.names.extend(sub.names.into_iter().map(|(n, k)| (n, first + k)));
                    let inside = sub.scheme.items.iter().filter(|i| matches!(i, Item::Compound(_))).count();
                    self.boxes.push((first, first + inside));
                    self.boxes.extend(sub.boxes.into_iter().map(|(a, b)| (first + a, first + b)));
                    let (items, moves) = (sub.scheme.items, sub.scheme.moves);
                    self.scheme.items.extend(items);
                    self.scheme.moves.extend(moves);
                }
                "\\" | "," | ";" | "quad" | "qquad" | " " => {}
                _ => {
                    // any other command is math for contextEquation (\Delta, \text…), up to the next scheme command
                    self.pos = start;
                    let mut math = String::new();
                    while let Some(m) = self.current() {
                        if m == '\\' && self.starts_scheme_command() && !math.is_empty() {
                            break;
                        }
                        if m == '$' {
                            break;
                        }
                        math.push(m);
                        self.pos += 1;
                        if m == '\\' {
                            while let Some(l) = self.current().filter(char::is_ascii_alphabetic) {
                                math.push(l);
                                self.pos += 1;
                            }
                        }
                    }
                    self.compound(Body::Math { tex: math.trim().to_string() }, None);
                }
            }
        }
        self.flush(&mut text);
        Ok(())
    }

    /// The command at the current position is one of the scheme's own.
    fn starts_scheme_command(&self) -> bool {
        let rest = &self.c[self.pos + 1..];
        if rest.first() == Some(&'+') {
            return true;
        }
        let name: String = rest.iter().take_while(|c| c.is_ascii_alphabetic()).collect();
        matches!(name.as_str(), "chemfig" | "schemestart" | "schemestop" | "ce" | "arrow" | "chemname" | "chemmove" | "subscheme" | "merge" | "chemleft" | "chemup")
    }

    /// Loose text between commands is a formula for mhchem: `A`, `2 H2O`.
    fn flush(&mut self, text: &mut String) {
        let t = text.trim();
        if !t.is_empty() {
            let tex = format!("\\ce{{{t}}}");
            self.compound(Body::Math { tex }, None);
        }
        text.clear();
    }

    /// `\schemestart[key=value,…]` (chemfig manual, section 6.1): `arrow angle` and `arrow coeff` set the
    /// arrows that give no angle or length; the other keys (`arrow style`, `init anchor`,
    /// `compound sep`…) are ignored, as `\chemfig`'s options are.
    fn scheme_options(&mut self, options: &str) {
        for option in split_top(options, ',') {
            let Some((key, value)) = option.split_once('=') else { continue };
            let value = value.trim().trim_start_matches('{').trim_end_matches('}').trim().parse::<f32>().ok().filter(|v| v.is_finite());
            match (key.trim(), value) {
                ("arrow angle", Some(a)) => self.arrows.0 = a,
                ("arrow coeff", Some(c)) if c > 0.0 => self.arrows.1 = c,
                _ => {}
            }
        }
    }

    /// How many compounds the scheme has so far.
    fn compounds(&self) -> usize {
        self.scheme.items.iter().filter(|i| matches!(i, Item::Compound(_))).count()
    }

    /// The compound a name stands for: one given by `\arrow(a--b)` or `\merge`, or chemfig's own name
    /// of the k-th compound, `ck`. chemfig numbers what lies between two arrows as one compound, `\+`
    /// and what it joins included (section 6.12), and a `\subscheme`'s box too, before the compounds in
    /// it (section 6.8); a box, or a sign, is not a compound of contextEquation's, so its name stands for none.
    fn named(&self, name: &str) -> Option<usize> {
        let count = self.compounds();
        let given = self.names.iter().rev().find(|(n, _)| n == name).map(|&(_, k)| k);
        if let Some(k) = given.filter(|&k| k < count) {
            return Some(k);
        }
        let wanted = name.strip_prefix('c')?.parse::<usize>().ok()?;
        let (mut number, mut index, mut joined) = (0, 0, false);
        for item in &self.scheme.items {
            match item {
                Item::Compound(_) => {
                    number += self.boxes.iter().filter(|b| b.0 == index).count();
                    if number >= wanted {
                        return None;
                    }
                    if !joined {
                        number += 1;
                        if number == wanted {
                            return Some(index);
                        }
                    }
                    index += 1;
                    joined = false;
                }
                Item::Plus => joined = true,
                Item::Arrow(_) | Item::Merge(_) => joined = false,
            }
        }
        None
    }

    /// The `\subscheme` box chemfig's name `ck` stands for (manual, section 6.8): its compounds, from the
    /// first to the one after the last, when it holds any.
    fn named_box(&self, name: &str) -> Option<(usize, usize)> {
        let wanted = name.strip_prefix('c')?.parse::<usize>().ok()?;
        let (mut number, mut index, mut joined) = (0, 0, false);
        for item in &self.scheme.items {
            match item {
                Item::Compound(_) => {
                    for b in self.boxes.iter().filter(|b| b.0 == index) {
                        number += 1;
                        if number == wanted {
                            return Some(*b).filter(|b| b.1 > b.0);
                        }
                    }
                    if !joined {
                        number += 1;
                    }
                    index += 1;
                    joined = false;
                }
                Item::Plus => joined = true,
                Item::Arrow(_) | Item::Merge(_) => joined = false,
            }
        }
        None
    }

    /// `name.anchor[style]` (chemfig manual, sections 6.4 to 6.6) as the name, the anchor's angle when it
    /// is one, and the compound's move (`xshift`, `yshift`, in em). A named anchor (`mid east`) is left to
    /// contextEquation, whose arrows meet the border facing them; the rest of a style (colors, fill, delimiters),
    /// which contextEquation does not draw, warns.
    fn node_spec(&mut self, spec: &str) -> R<NodeSpec> {
        let (spec, style) = match spec.split_once('[') {
            Some((s, style)) => (s, style.trim_end_matches(']').trim()),
            None => (spec, ""),
        };
        let (name, anchor) = spec.split_once('.').map_or((spec.trim(), ""), |(n, a)| (n.trim(), a.trim()));
        let angle = anchor.parse::<f32>().ok().filter(|a| a.is_finite());
        let mut shift: Option<(f32, f32)> = None;
        let mut rest = Vec::new();
        let keys = split_top(style, ',');
        for key in keys.iter().map(|k| k.trim()).filter(|k| !k.is_empty()) {
            let moved = key.split_once('=').and_then(|(k, v)| {
                let em = chemfig::dimension_em(v)?;
                match k.trim() {
                    "xshift" => Some((em, 0.0)),
                    "yshift" => Some((0.0, em)),
                    _ => None,
                }
            });
            match moved {
                Some((x, y)) => {
                    let s = shift.get_or_insert((0.0, 0.0));
                    *s = (s.0 + x, s.1 + y);
                }
                None => rest.push(key),
            }
        }
        if !rest.is_empty() {
            self.reader.tolerate(rules::E_REC_9, format!("compound style [{}]", rest.join(",")), "ignored")?;
        }
        Ok((name.to_string(), angle, shift))
    }

    /// chemfig's `\merge` (manual, section 6.11): `\merge>[above][below](n1.a1)(n2)…--(n.a[style])[c1,c2,c,style]`,
    /// right after its name. The direction is `>` (the default), `<`, `^` or `v`; the compounds joined
    /// are named as `\arrow(a--b)` names them, or by chemfig's own names `c1`, `c2`…; the name after
    /// `--` is the next compound's.
    fn merge(&mut self) -> R<()> {
        self.skip_spaces();
        let mut direction = '>';
        match self.current() {
            Some(d @ ('>' | '<' | '^' | 'v')) => {
                direction = d;
                self.pos += 1;
            }
            Some('{') => {
                let d = self.group('{', '}')?;
                match d.trim() {
                    d @ (">" | "<" | "^" | "v") => direction = d.chars().next().unwrap_or('>'),
                    other => self.reader.tolerate(rules::E_REC_9, format!("\\merge direction {{{other}}}"), "to the right")?,
                }
            }
            _ => {}
        }
        let mut labels = Vec::new();
        while let Some(l) = self.optional()? {
            labels.push(l);
        }
        let mut from = Vec::new();
        let mut next = None;
        loop {
            self.skip_spaces();
            match self.current() {
                Some('(') => {
                    let spec = self.group('(', ')')?;
                    let (name, anchor, shift) = self.node_spec(&spec)?;
                    if shift.is_some() {
                        self.reader.tolerate(rules::E_REC_9, format!("\\merge: a move of {name}"), "ignored")?;
                    }
                    match self.named(&name) {
                        Some(k) => from.push((k, anchor)),
                        None => self.reader.tolerate(rules::E_REC_9, format!("\\merge: no compound is named {name}"), "its line left out")?,
                    }
                }
                Some('-') if self.c.get(self.pos + 1) == Some(&'-') => {
                    self.pos += 2;
                    self.skip_spaces();
                    if self.current() == Some('(') {
                        let spec = self.group('(', ')')?;
                        let (name, anchor, shift) = self.node_spec(&spec)?;
                        if shift.is_some() {
                            self.reader.tolerate(rules::E_REC_9, format!("\\merge: a move of {name}"), "ignored")?;
                        }
                        next = Some((name, anchor));
                    }
                    break;
                }
                _ => break,
            }
        }
        if next.is_none() {
            self.reader.tolerate(rules::E_REC_9, "\\merge without --(name) for the next compound", "the next compound follows the arrow")?;
        }
        let mut coefficients = (0.5f32, 0.5f32, 0.5f32);
        if let Some(opts) = self.optional()? {
            let parts = split_top(&opts, ',');
            let number = |i: usize, default: f32| parts.get(i).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).unwrap_or(default);
            coefficients = (number(0, 0.5), number(1, 0.5), number(2, 0.5).clamp(0.0, 1.0));
            let style = parts.get(3..).map(|s| s.join(",")).unwrap_or_default();
            if !style.trim().is_empty() {
                self.reader.tolerate(rules::E_REC_9, format!("\\merge style {}", style.trim()), "drawn as a plain arrow")?;
            }
        }
        if let Some((name, _)) = next.filter(|(n, _)| !n.is_empty()) {
            let k = self.compounds();
            self.names.push((name, k));
        }
        let mut label = |l: Option<&String>| -> R<Option<Label>> {
            let Some(t) = l.map(|t| t.trim()).filter(|t| !t.is_empty()) else { return Ok(None) };
            // `*{angle.anchor}text`: the rotation and anchor of the label, which contextEquation does not draw
            let t = match t.strip_prefix("*{").and_then(|r| r.split_once('}')) {
                Some((how, text)) => {
                    self.reader.tolerate(rules::E_REC_9, format!("\\merge label rotation *{{{how}}}"), "the label drawn level")?;
                    text.trim()
                }
                None => t,
            };
            Ok((!t.is_empty()).then(|| Label::Text { tex: t.to_string() }))
        };
        let above = label(labels.first())?;
        let below = label(labels.get(1))?;
        if from.is_empty() {
            return self.reader.tolerate(rules::E_REC_9, "\\merge joins no compound", "left out");
        }
        let angle = match direction {
            '<' => 180.0,
            '^' => 90.0,
            'v' => -90.0,
            _ => 0.0,
        };
        self.scheme.items.push(Item::Merge(Merge { from, angle, above, below, coefficients }));
        Ok(())
    }

    /// `\arrow(a--b){type[above][below]}[angle,length]`, or a bare `\arrow` (a plain arrow). `(a--b)`
    /// names the compound before the arrow `a` and the one after it `b` (chemfig manual, section 6.4).
    fn arrow(&mut self) -> R<()> {
        self.skip_spaces();
        let (mut from, mut to) = (None, None);
        let mut shift = None;
        let mut from_box = None;
        if self.current() == Some('(') {
            let names = self.group('(', ')')?;
            let (before, after) = names.split_once("--").unwrap_or((&names, ""));
            let (before, _, moved) = self.node_spec(before)?;
            if moved.is_some() {
                self.reader.tolerate(rules::E_REC_9, "a move of the compound before an arrow", "ignored")?;
            }
            let (after, _, moved) = self.node_spec(after)?;
            shift = moved;
            let count = self.compounds();
            if let Some(start) = before.strip_prefix('@') {
                // section 6.7: the arrow branches from that compound
                from = self.named(start);
                // a \subscheme's box: the arrow leaves from the box around its compounds
                from_box = if from.is_none() { self.named_box(start) } else { None };
                if from.is_none() && from_box.is_none() {
                    self.reader.tolerate(rules::E_REC_9, format!("an arrow from @{start}: no compound is named {start}"), "drawn from the last compound")?;
                }
            } else if !before.is_empty() && count > 0 {
                self.names.push((before, count - 1));
            }
            if let Some(end) = after.strip_prefix('@') {
                // section 6.7: an arrow to a compound already placed
                to = self.named(end);
                if to.is_none() {
                    self.reader.tolerate(rules::E_REC_9, format!("an arrow to @{end}: no compound is named {end}"), "drawn to the next compound")?;
                }
            } else if !after.is_empty() {
                self.names.push((after, count));
            }
            self.skip_spaces();
        }
        let spec = if self.current() == Some('{') { self.group('{', '}')? } else { String::new() };
        let spec = spec.trim();
        let type_end = spec.find('[').unwrap_or(spec.len());
        let (kind_text, rest) = spec.split_at(type_end);
        let u_arrow = kind_text.trim() == "-U>";
        let kind = match kind_text.trim() {
            "-U>" => ArrowKind::Forward,
            "" | "->" => ArrowKind::Forward,
            "<-" => ArrowKind::Backward,
            "<=>" => ArrowKind::Equilibrium,
            "<->>" => ArrowKind::EquilibriumRight,
            "<<->" => ArrowKind::EquilibriumLeft,
            "<->" => ArrowKind::Resonance,
            "=>" => ArrowKind::Retro,
            "-/>" => ArrowKind::NoReaction,
            "0" => ArrowKind::Invisible,
            other => {
                self.reader.tolerate(rules::E_REC_1, format!("unknown arrow type: {other}"), "drawn as ->")?;
                ArrowKind::Forward
            }
        };
        let mut labels = Vec::new();
        let mut sub = SchemeReader {
            c: rest.chars().collect(),
            pos: 0,
            reader: self.reader,
            scheme: Scheme { items: vec![], moves: vec![] },
            names: vec![],
            arrows: (0.0, 1.0),
            boxes: vec![],
        };
        while let Some(text) = sub.optional()? {
            labels.push(text);
        }
        let mut label = |text: Option<&String>| -> R<Option<Label>> {
            let Some(t) = text.map(|t| t.trim()).filter(|t| !t.is_empty()) else { return Ok(None) };
            if let Some(start) = t.find("\\chemfig") {
                let mut r = SchemeReader {
                    c: t[start + "\\chemfig".len()..].chars().collect(),
                    pos: 0,
                    reader: self.reader,
                    scheme: Scheme { items: vec![], moves: vec![] },
                    names: vec![],
                    arrows: (0.0, 1.0),
                    boxes: vec![],
                };
                let _ = r.optional()?;
                let body = r.braced()?;
                return Ok(Some(Label::Structure(chemfig::molecule(&body, self.reader)?)));
            }
            Ok(Some(Label::Text { tex: t.to_string() }))
        };
        let above = label(labels.first())?;
        let below = label(labels.get(1))?;
        // -U>[above][below][shift][radius][half angle]: the radius 0.333 of the length, the half angle 60°
        let number = |i: usize| labels.get(i).and_then(|v| v.trim().parse::<f32>().ok());
        let arc = u_arrow.then(|| UArc { radius: number(3).unwrap_or(0.333), half_angle: number(4).map_or(60.0, f32::abs) });
        let (mut angle, mut length) = self.arrows;
        if let Some(opts) = self.optional()? {
            let parts = split_top(&opts, ',');
            if let Some(a) = parts.first().and_then(|a| a.trim().parse::<f32>().ok()) {
                angle = a;
            }
            if let Some(l) = parts.get(1).and_then(|l| l.trim().parse::<f32>().ok()).filter(|l| *l > 0.0) {
                length = l;
            }
        }
        self.scheme.items.push(Item::Arrow(Arrow { kind, above, below, angle, length, arc, from, to, shift, from_box }));
        Ok(())
    }

    /// The `\draw` paths of a `\chemmove`, between hooks: `(a)..controls +(90:1cm) and +(45:5mm)..(b)`,
    /// `(a) to[out=90,in=45] (b)`, `(a) to[bend left] (b)`.
    fn moves(&mut self, body: &str) -> R<()> {
        for statement in body.split(';') {
            let st = statement.trim();
            if st.is_empty() {
                continue;
            }
            let Some(path) = st.strip_prefix("\\draw") else {
                self.reader.tolerate(rules::E_REC_1, format!("only \\draw is read inside \\chemmove: {st}"), "ignored")?;
                continue;
            };
            let mut p = SchemeReader {
                c: path.chars().collect(),
                pos: 0,
                reader: self.reader,
                scheme: Scheme { items: vec![], moves: vec![] },
                names: vec![],
                arrows: (0.0, 1.0),
                boxes: vec![],
            };
            let options = p.optional()?.unwrap_or_default();
            let single = ["harpoon", "left to", "right to", "fishhook", "half"].iter().any(|k| options.contains(k));
            let names: Vec<String> = parens(path).into_iter().filter(|n| !n.contains(':')).collect();
            if names.len() < 2 {
                self.reader.tolerate(rules::E_REC_9, format!("an electron arrow needs two hooks: {st}"), "ignored")?;
                continue;
            }
            let controls: Vec<(f32, f32)> = parens(path).iter().filter_map(|n| polar(n)).collect();
            let mut m = ElectronMove { from: hook_of(&names[0]), to: hook_of(&names[names.len() - 1]), out_angle: None, in_angle: None, reach: None, bend: 45.0, single };
            if controls.len() >= 2 {
                m.out_angle = Some(controls[0].0);
                m.in_angle = Some(controls[1].0);
                m.reach = Some((controls[0].1, controls[1].1));
            }
            let key = |k: &str| -> Option<f32> {
                let at = path.find(k)?;
                let rest = &path[at + k.len()..];
                let end = rest.find([',', ']']).unwrap_or(rest.len());
                rest[..end].trim().parse().ok()
            };
            if let (Some(o), Some(i)) = (key("out="), key("in=")) {
                m.out_angle = Some(o);
                m.in_angle = Some(i);
            }
            if path.contains("bend right") {
                m.bend = -key("bend right=").unwrap_or(45.0);
            } else if path.contains("bend left") {
                m.bend = key("bend left=").unwrap_or(45.0);
            }
            self.scheme.moves.push(m);
        }
        Ok(())
    }
}

/// Where chemfig's `\merge` ends, from right after its name: the direction (`>`, `<`, `^`, `v`, or
/// braced), the labels in brackets, the names of the compounds it joins in parentheses, `--`, the name
/// of the next one and its `[c1,c2,c,style]`.
pub fn merge_end(c: &[char], mut i: usize) -> usize {
    let skip = |i: &mut usize| {
        while c.get(*i).is_some_and(|x| x.is_whitespace()) {
            *i += 1;
        }
    };
    let group = |i: &mut usize, open: char, close: char| {
        let mut level = 0;
        while let Some(&x) = c.get(*i) {
            *i += 1;
            if x == open {
                level += 1;
            } else if x == close {
                level -= 1;
                if level == 0 {
                    break;
                }
            }
        }
    };
    skip(&mut i);
    match c.get(i) {
        Some('>' | '<' | '^' | 'v') => i += 1,
        Some('{') => group(&mut i, '{', '}'),
        _ => {}
    }
    loop {
        skip(&mut i);
        match c.get(i) {
            Some('[') => group(&mut i, '[', ']'),
            Some('(') => group(&mut i, '(', ')'),
            Some('-') if c.get(i + 1) == Some(&'-') => i += 2,
            _ => return i,
        }
    }
}

/// Where `\chemleft<d>…\chemright<d>` (or `\chemup…\chemdown`) ends, from right after the opening
/// name: after the closing name's delimiter, or at the end.
pub fn delimited_end(c: &[char], mut i: usize, open: &str, close: &str) -> usize {
    let word =
        |i: usize, w: &str| c.get(i) == Some(&'\\') && c[i + 1..].iter().take(w.len()).copied().eq(w.chars()) && !c.get(i + 1 + w.len()).is_some_and(|x| x.is_ascii_alphabetic());
    let mut level = 0;
    while i < c.len() {
        if word(i, open) {
            level += 1;
            i += 1 + open.len();
        } else if word(i, close) {
            i += 1 + close.len();
            if level == 0 {
                // the delimiter: a character, or a control sequence (\}, \rangle)
                while c.get(i).is_some_and(|x| x.is_whitespace()) {
                    i += 1;
                }
                if c.get(i) == Some(&'\\') {
                    i += 1;
                    let letters = c[i..].iter().take_while(|x| x.is_ascii_alphabetic()).count();
                    i += letters.max(1);
                } else {
                    i += 1;
                }
                return i.min(c.len());
            }
            level -= 1;
        } else {
            i += 1;
        }
    }
    c.len()
}

/// The contents of each `( … )` in a TikZ path.
fn parens(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut level = 0;
    let mut s = String::new();
    for c in path.chars() {
        match c {
            '(' => {
                level += 1;
                if level == 1 {
                    s.clear();
                    continue;
                }
            }
            ')' => {
                level -= 1;
                if level == 0 {
                    out.push(s.trim().to_string());
                    continue;
                }
            }
            _ => {}
        }
        if level > 0 {
            s.push(c);
        }
    }
    out
}

/// `90:1cm` as (90°, 2.845 em).
fn polar(s: &str) -> Option<(f32, f32)> {
    let (a, l) = s.split_once(':')?;
    Some((a.trim().parse().ok()?, length_em(l.trim())?))
}

/// A TeX length in em, with the em of 10 pt type: 1 cm = 2.845 em, 1 pt = 0.1 em.
pub fn length_em(s: &str) -> Option<f32> {
    let split = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let (n, unit) = s.split_at(split);
    let n: f32 = n.trim().parse().ok()?;
    let factor = match unit.trim() {
        "cm" => 2.845,
        "mm" => 0.2845,
        "pt" | "bp" => 0.1,
        "in" => 7.227,
        "ex" => 0.43,
        "em" | "" => 1.0,
        _ => return None,
    };
    Some(n * factor)
}
