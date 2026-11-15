//! siunitx v3 beyond the quantities of `latex.rs`: complex numbers (`\complexnum`, `\complexqty`),
//! the units based on constants (`\planckbar`, `\clight`…), `\highlight`, units the equation declares
//! (`\DeclareSIUnit`) and the options of `\sisetup` contextEquation can honor, both for the rest of the input,
//! as `\definecolor` defines a color.

use super::*;

/// What `\DeclareSIUnit` and `\sisetup` set, for the rest of the input.
#[derive(Default)]
pub(super) struct State {
    /// Declared units: the name (without the backslash) and its definition as written.
    pub units: std::collections::HashMap<String, String>,
    pub per_mode: PerMode,
    /// `inter-unit-product`, as LaTeX (`\cdot`); the thin space when not set.
    pub inter_unit_product: Option<String>,
    /// `quantity-product`, as LaTeX; the thin space when not set.
    pub quantity_product: Option<String>,
    /// `output-complex-root`, as LaTeX; an upright i when not set.
    pub complex_root: Option<String>,
    /// `complex-root-position = before-number`: i2 instead of 2i.
    pub root_before: bool,
    /// `retain-explicit-plus`: `\qty{+3}{m}` keeps its plus.
    pub retain_plus: bool,
}

/// How the units with negative powers are written (`per-mode`). contextEquation's own default is the slash
/// (`spec/rules.md`, L-UNI-1); only `\sisetup{per-mode=…}` changes it.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub(super) enum PerMode {
    /// m/s, J/(mol·K): `symbol`, `single-symbol` and `symbol-or-fraction`.
    #[default]
    Symbol,
    /// J/mol/K: `repeated-symbol`.
    Repeated,
    /// m s⁻¹: `power` and `power-positive-first`.
    Power,
    /// m over s, as `\frac`: `fraction`.
    Fraction,
}

/// A unit's symbol: text, which its prefix joins (km), or the math of a unit based on a constant (c₀).
pub(super) enum UnitSymbol {
    Text(String),
    Math(Node),
}

/// The `\sisetup` options contextEquation honors.
pub(super) const HONORED: [&str; 6] = ["per-mode", "inter-unit-product", "quantity-product", "output-complex-root", "complex-root-position", "retain-explicit-plus"];

/// The units based on constants (siunitx's emulation of version 2): their symbols as LaTeX, in
/// italic, as siunitx sets them.
pub(super) fn constant(name: &str) -> Option<&'static str> {
    Some(match name {
        "planckbar" => "\\hbar",
        "clight" => "c_{0}",
        "bohr" => "a_{0}",
        "hartree" => "E_{\\mathrm{h}}",
        "electronmass" => "m_{\\mathrm{e}}",
        "elementarycharge" => "e",
        _ => return None,
    })
}

/// A declared unit's definition as the symbol it writes, when it is plain text (`rpm`, `\text{rpm}`,
/// `\mathrm{B}`); None when it is made of other units (`\kilo\watt\hour`).
pub(super) fn plain_symbol(definition: &str) -> Option<String> {
    let mut d = definition.trim();
    for wrapper in ["\\text{", "\\textrm{", "\\mathrm{", "\\ensuremath{", "\\textup{"] {
        if let Some(inner) = d.strip_prefix(wrapper).and_then(|x| x.strip_suffix('}')) {
            d = inner.trim();
        }
    }
    (!d.contains('\\') && !d.is_empty()).then(|| d.replace(['{', '}'], ""))
}

impl Reader<'_> {
    /// `\DeclareSIUnit` and `\sisetup`, which `definition` asks for. False for another command.
    #[inline(never)]
    pub(super) fn siunitx_definition(&mut self, name: &str, col: usize) -> R<bool> {
        match name {
            // \DeclareSIUnit[options]{\name}{definition}: a unit for the rest of the input
            "DeclareSIUnit" => {
                self.raw_optional();
                let target = self.defined_name(col)?;
                let definition = self.raw_text()?;
                self.registry.note(rules::L_MAC_1, format!("unit \\{target} declared as '{}'", definition.trim()));
                self.macros.siunitx.units.insert(target, definition);
                Ok(true)
            }
            "sisetup" => {
                let options = self.raw_text()?;
                self.si_options(&options, col)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Applies `key=value, …` of `\sisetup`: the honored options for the rest of the input, the others
    /// ignored with a warning (E-REC-9; refused in strict mode).
    fn si_options(&mut self, options: &str, col: usize) -> R<()> {
        for option in split_options(options) {
            let (key, value) = option.split_once('=').map_or((option.trim(), ""), |(k, v)| (k.trim(), v.trim()));
            let value = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')).unwrap_or(value).trim();
            let s = &mut self.macros.siunitx;
            let text = Some(value.to_string());
            match (key, value) {
                ("per-mode", "symbol" | "single-symbol" | "symbol-or-fraction") => s.per_mode = PerMode::Symbol,
                ("per-mode", "repeated-symbol") => s.per_mode = PerMode::Repeated,
                ("per-mode", "power" | "power-positive-first") => s.per_mode = PerMode::Power,
                ("per-mode", "fraction") => s.per_mode = PerMode::Fraction,
                ("inter-unit-product", _) => s.inter_unit_product = text,
                ("quantity-product", _) => s.quantity_product = text,
                ("output-complex-root", _) => s.complex_root = text,
                ("complex-root-position", "before-number" | "after-number") => s.root_before = value == "before-number",
                ("retain-explicit-plus", "" | "true" | "false") => s.retain_plus = value != "false",
                _ => {
                    let what = if HONORED.contains(&key) {
                        format!("\\sisetup: {key}={value} is not a value contextEquation knows")
                    } else {
                        format!("\\sisetup: {key} is not an option contextEquation honors")
                    };
                    self.tolerate(rules::E_REC_9, col, what, "ignored")?;
                }
            }
        }
        Ok(())
    }

    /// The units of `unit` with the declared ones made of other units written out, so the quantity
    /// reads them as those (`\kWh` → `\kilo\watt\hour`). A unit that declares itself stops there.
    pub(super) fn declared_units_written_out(&self, unit: &str) -> String {
        let mut text = unit.to_string();
        for _ in 0..8 {
            let mut changed = false;
            let mut out = String::new();
            for (i, token) in text.split('\\').enumerate() {
                if i == 0 {
                    out.push_str(token);
                    continue;
                }
                let end = token.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(token.len());
                let (name, rest) = token.split_at(end);
                match self.macros.siunitx.units.get(name).filter(|d| plain_symbol(d).is_none() && !d.contains(&format!("\\{name}"))) {
                    Some(definition) => {
                        out.push_str(definition);
                        out.push_str(rest);
                        changed = true;
                    }
                    None => {
                        out.push('\\');
                        out.push_str(token);
                    }
                }
            }
            text = out;
            if !changed {
                break;
            }
        }
        text
    }

    /// The symbol of a unit named `name`, which is not one of siunitx's own: a declared one, a unit
    /// based on a constant, or None.
    pub(super) fn other_unit_symbol(&mut self, name: &str) -> R<Option<UnitSymbol>> {
        if let Some(symbol) = self.macros.siunitx.units.get(name).and_then(|d| plain_symbol(d)) {
            return Ok(Some(UnitSymbol::Text(symbol)));
        }
        match constant(name) {
            Some(tex) => Ok(Some(UnitSymbol::Math(self.sub_reader(tex, Variant::Italic)?))),
            None => Ok(None),
        }
    }

    /// The node `inter-unit-product` or `quantity-product` asks for: a thin space by default.
    pub(super) fn si_product(&mut self, product: Option<String>) -> R<Node> {
        match product {
            Some(p) if !p.trim().is_empty() => self.sub_reader(&p, Variant::Upright),
            Some(_) => Ok(Node::row(vec![])),
            None => Ok(Node::Space { em: 3.0 / 18.0 }),
        }
    }

    /// `\complexnum{1+2i}`: the real and the imaginary part (each a siunitx number, with the locale's
    /// decimal marker), the root after the number (or before, by `complex-root-position`); `r:θ` is
    /// polar, the angle in degrees. Also whether both parts are there, which `\complexqty` brackets.
    #[inline(never)]
    pub(super) fn complex_number(&mut self, raw: &str, col: usize) -> R<(Node, bool)> {
        let clean: String = raw.chars().filter(|c| !c.is_whitespace() && *c != '{' && *c != '}').collect();
        if let Some((modulus, angle)) = clean.split_once(':') {
            let modulus = self.number_value(modulus, col)?;
            let angle = self.angle(angle, col)?;
            return Ok((Node::row(vec![modulus, Node::Operator { symbol: '∠', class: Class::Ord }, angle]), false));
        }
        // the sign between the parts: the last + or − that does not start the text or an exponent
        let c: Vec<char> = clean.chars().collect();
        let split = (1..c.len()).rev().find(|&k| matches!(c[k], '+' | '-') && !matches!(c[k - 1], 'e' | 'E' | '+' | '-'));
        let (first, second) = match split {
            Some(k) => (c[..k].iter().collect::<String>(), Some(c[k..].iter().collect::<String>())),
            None => (clean.clone(), None),
        };
        let imaginary = |t: &str| t.contains(['i', 'j']);
        let (real, imag) = match second {
            Some(s) if imaginary(&s) && !imaginary(&first) => (Some(first), Some(s)),
            Some(s) if imaginary(&first) && !imaginary(&s) => (Some(s), Some(first)),
            None if imaginary(&first) => (None, Some(first)),
            None => (Some(first), None),
            Some(_) => {
                self.tolerate(rules::E_REC_9, col, format!("{raw} is not a complex number"), "drawn as written")?;
                return Ok((self.sub_reader(raw, Variant::Italic)?, false));
            }
        };
        let mut children = Vec::new();
        let both = real.is_some() && imag.is_some();
        if let Some(r) = &real {
            children.push(self.number_value(r.trim_start_matches('+'), col)?);
        }
        if let Some(i) = imag {
            let negative = i.starts_with('-');
            let digits = i.trim_start_matches(['+', '-']).replace(['i', 'j'], "");
            match (both, negative) {
                (true, true) => children.push(Node::Operator { symbol: '−', class: Class::Bin }),
                (true, false) => children.push(Node::Operator { symbol: '+', class: Class::Bin }),
                (false, true) => children.push(Node::Operator { symbol: '−', class: Class::Ord }),
                (false, false) => {}
            }
            let root = match self.macros.siunitx.complex_root.clone() {
                Some(tex) => self.sub_reader(&tex, Variant::Italic)?,
                None => Node::Identifier { name: "i".into(), variant: Variant::Upright },
            };
            let number = if digits.is_empty() { None } else { Some(self.number_value(&digits, col)?) };
            let before = self.macros.siunitx.root_before;
            match number {
                Some(n) if before => children.extend([root, n]),
                Some(n) => children.extend([n, root]),
                None => children.push(root),
            }
        }
        Ok((Node::row(children), both))
    }

    /// `\complexqty{3+4i}{\ohm}`: the complex number, in parentheses when it has both parts, and the unit.
    #[inline(never)]
    pub(super) fn complex_quantity(&mut self, raw: &str, unit: &str, col: usize) -> R<Node> {
        let (number, both) = self.complex_number(raw, col)?;
        let Node::Quantity { unit: unit_row, .. } = self.quantity(None, unit, col)? else { unreachable!("a quantity") };
        let mut children = Vec::new();
        if both {
            children.push(Node::Operator { symbol: '(', class: Class::Open });
            children.push(number);
            children.push(Node::Operator { symbol: ')', class: Class::Close });
        } else {
            children.push(number);
        }
        let product = self.macros.siunitx.quantity_product.clone();
        children.push(self.si_product(product)?);
        children.extend(undo_line(*unit_row));
        Ok(Node::Quantity { value: None, unit: Box::new(Node::row(children)) })
    }
}

/// A unit of a quantity: its symbol, its power, the names of its prefixes and its name (for the speech)
/// and the color `\highlight` gave it.
pub(super) type UnitEntry = (Node, i32, String, String, Option<String>);

/// One unit as a node: the symbol with its power (negated for `per-mode=power`). `\highlight` colors
/// the symbol; the power stays in the text color, so the speech still reads it ("squared").
pub(super) fn unit_node(u: &UnitEntry, negative: bool) -> Node {
    let symbol = match &u.4 {
        Some(color) => Node::Colored { color: color.clone(), base: Box::new(u.0.clone()) },
        None => u.0.clone(),
    };
    let base = Node::Unit { prefix: Some(u.2.clone()).filter(|p| !p.is_empty()), name: u.3.clone(), base: Box::new(symbol) };
    let power = Node::Number { value: u.1.to_string(), group: false, thousands: None, variant: Variant::Upright };
    let power = match negative {
        true => Node::row(vec![Node::Operator { symbol: '−', class: Class::Ord }, power]),
        false if u.1 == 1 => return base,
        false => power,
    };
    Node::Scripts { base: Box::new(base), sup: Some(Box::new(power)), sub: None }
}

/// Units one after the other, `between` them (`inter-unit-product`).
pub(super) fn joined_units(units: &[UnitEntry], negative: bool, between: &Node) -> Vec<Node> {
    let mut out = Vec::new();
    for (i, u) in units.iter().enumerate() {
        if i > 0 {
            out.push(between.clone());
        }
        out.push(unit_node(u, negative));
    }
    out
}

/// The options of `key=value, …`, split at the commas outside braces.
fn split_options(options: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0usize, 0);
    for (i, c) in options.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&options[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&options[start..]);
    parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
}
