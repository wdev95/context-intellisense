//! siunitx v3 (issue #175): complex numbers, the units based on constants, `\highlight`,
//! `\DeclareSIUnit` and `\sisetup`, the last two for the rest of the input.

use context_equation_core::tree::{Class, Node, Variant};
use context_equation_core::{convert, Locale, Options};

fn en(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn pt(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { locale: Locale::PtBr, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn strict(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// Every node of the tree, depth first.
fn nodes(node: &Node) -> Vec<&Node> {
    let mut out = vec![node];
    let children: Vec<&Node> = match node {
        Node::Row { children } => children.iter().collect(),
        Node::Quantity { unit, .. } => vec![unit],
        Node::Unit { base, .. } | Node::Colored { base, .. } => vec![base],
        Node::Scripts { base, sup, sub } => [Some(base), sup.as_ref(), sub.as_ref()].into_iter().flatten().map(|b| b.as_ref()).collect(),
        Node::Fraction { num, den, .. } => vec![num, den],
        _ => vec![],
    };
    for c in children {
        out.extend(nodes(c));
    }
    out
}

#[test]
fn complex_numbers() {
    let r = strict(r"\complexnum{1+2i}");
    assert_eq!(r.text, "1 + 2i");
    // the root is an upright i, after the number; j is read as i, as siunitx writes it
    assert!(nodes(&r.tree).contains(&&Node::Identifier { name: "i".into(), variant: Variant::Upright }));
    assert_eq!(strict(r"\complexnum{3.5-0.25j}").text, "3.5 − 0.25i");
    assert_eq!(strict(r"\complexnum{2i + 1}").text, "1 + 2i");
    assert_eq!(strict(r"\complexnum{-i}").text, "−i");
    assert_eq!(strict(r"\complexnum{1e3+2i}").text, strict(r"\num{1e3}").text + " + 2i");
    // polar: the angle in degrees
    assert_eq!(strict(r"\complexnum{1:30}").text, "1∠30°");
    // L-NUM-1: the locale's decimal marker
    assert_eq!(pt(r"\complexnum{3.5-0.25i}").text, "3,5 − 0,25i");
    // something else is drawn as written, with a warning; strict mode refuses it
    assert!(en(r"\complexnum{2i+3i}").warnings.iter().any(|w| w.rule == "E-REC-9"));
    assert!(convert(r"\complexnum{2i+3i}", &Options { strict: true, ..Default::default() }).is_err());
    // the root and its position, by \sisetup
    assert_eq!(strict(r"\sisetup{output-complex-root=\mathrm{j}, complex-root-position=before-number} \complexnum{1+2i}").text, "1 + j2");
}

#[test]
fn complex_quantities() {
    let r = strict(r"\complexqty{3+4i}{\ohm}");
    assert_eq!(r.text, "(3 + 4i) Ω");
    assert!(matches!(r.tree.single(), Node::Quantity { .. }));
    assert_eq!(r.speech, "open paren 3 plus 4 i close paren ohms");
    // one part alone needs no parentheses
    assert_eq!(strict(r"\complexqty{2i}{\volt}").text, "2i V");
    assert_eq!(pt(r"\complexqty{1,5+2i}{\ohm}").text, "(1,5 + 2i) Ω");
    // in text, as the other siunitx commands
    assert_eq!(strict(r"\text{Z = \complexqty{3+4i}{\ohm}}").text, strict(r"\text{Z = }\complexqty{3+4i}{\ohm}").text);
}

#[test]
fn units_based_on_constants() {
    assert!(strict(r"\unit{\planckbar}").warnings.is_empty());
    assert_eq!(strict(r"\unit{\planckbar}").text, "ℏ");
    for (unit, text) in [("clight", "c_0"), ("bohr", "a_0"), ("hartree", "E_h"), ("electronmass", "m_e"), ("elementarycharge", "e")] {
        assert_eq!(strict(&format!(r"\unit{{\{unit}}}")).text, text, "{unit}");
    }
    // in italic, as siunitx sets them, with the subscript of E and m upright
    let r = strict(r"\qty{2}{\hartree}");
    assert!(nodes(&r.tree)
        .iter()
        .any(|n| matches!(n, Node::Scripts { base, sub: Some(_), .. } if matches!(base.as_ref(), Node::Identifier { name, variant: Variant::Italic } if name == "E"))));
    assert_eq!(strict(r"\qty{1}{\clight\per\second}").text, "1 c_0/s");
}

#[test]
fn highlight() {
    let r = strict(r"\qty{9.8}{\highlight{red}\metre\per\highlight{blue}\second\squared}");
    let colored: Vec<&str> = nodes(&r.tree).iter().filter_map(|n| if let Node::Colored { color, .. } = n { Some(color.as_str()) } else { None }).collect();
    // the unit's symbol (its power stays in the text color, so it is still read as "squared")
    assert_eq!(colored, ["red", "blue"]);
    assert_eq!(r.text, "9.8 m/s^2");
    assert_eq!(r.speech, strict(r"\qty{9.8}{\metre\per\second\squared}").speech);
}

#[test]
fn declared_units_last_for_the_rest_of_the_input() {
    let r = strict(r"\DeclareSIUnit\rpm{rpm} \DeclareSIUnit{\bel}{\text{Bel}} \qty{3000}{\rpm} + \unit{\bel}");
    assert_eq!(r.text, "3000 rpm + Bel");
    assert!(r.warnings.is_empty());
    // made of other units: read as those, prefixes and names included
    let r = strict(r"\DeclareSIUnit{\kWh}{\kilo\watt\hour} \qty{2}{\kWh\per\day}");
    assert_eq!(r.text, "2 kW h/d");
    assert!(r.speech.contains("kilowatt"), "{}", r.speech);
    // before the declaration it is unknown; on another equation (another line of --batch), too
    assert!(en(r"\unit{\rpm} \DeclareSIUnit\rpm{rpm}").warnings.iter().any(|w| w.description.contains("unsupported unit")));
    assert!(en(r"\unit{\rpm}").warnings.iter().any(|w| w.description.contains("unsupported unit")));
    // a unit that declares itself does not loop
    assert!(!en(r"\DeclareSIUnit\x{\x\metre} \unit{\x}").text.is_empty());
}

#[test]
fn sisetup_per_mode_and_products() {
    let speed = r"\qty{9.8}{\metre\per\second\squared}";
    // the slash by default (spec/rules.md, L-UNI-1)
    assert_eq!(strict(speed).text, "9.8 m/s^2");
    let r = strict(&format!(r"\sisetup{{per-mode=fraction}} {speed}"));
    assert!(nodes(&r.tree).iter().any(|n| matches!(n, Node::Fraction { .. })), "{:?}", r.tree);
    assert_eq!(r.speech, strict(speed).speech);
    let r = strict(&format!(r"\sisetup{{per-mode=power}} {speed}"));
    assert_eq!(r.text, "9.8 m s^(−2)");
    let r = strict(r"\sisetup{per-mode=repeated-symbol} \qty{8.3}{\joule\per\mole\per\kelvin}");
    assert_eq!(r.text, "8.3 J/mol/K");
    assert_eq!(r.speech, "8.3 joules per mole per kelvin");
    // the setting lasts for the rest of the input, and \sisetup again changes it back
    let r = strict(&format!(r"\sisetup{{per-mode=power}} {speed} \sisetup{{per-mode=symbol}} {speed}"));
    assert_eq!(r.text, "9.8 m s^(−2)9.8 m/s^2");
    // the products between units and between the number and the unit
    let r = strict(r"\sisetup{inter-unit-product=\cdot, quantity-product={}} \qty{3}{\newton\metre}");
    assert_eq!(r.text, "3N · m");
    assert!(nodes(&r.tree).iter().any(|n| matches!(n, Node::Operator { symbol: '·', class: Class::Bin })));
    // retain-explicit-plus keeps the plus siunitx otherwise drops
    assert_eq!(strict(r"\sisetup{retain-explicit-plus} \qty{+3}{\metre}").text, "+3 m");
    assert_eq!(strict(r"\qty{+3}{\metre}").text, "3 m");
}

#[test]
fn sisetup_options_context_equation_does_not_honor() {
    let r = en(r"\sisetup{locale=DE, per-mode=sideways, group-digits=false} \qty{1}{\metre}");
    let warned: Vec<&str> = r.warnings.iter().filter(|w| w.rule == "E-REC-9").map(|w| w.description.as_str()).collect();
    assert_eq!(warned.len(), 3, "{warned:?}");
    assert!(warned[0].contains("locale") && warned[1].contains("sideways") && warned[2].contains("group-digits"));
    assert_eq!(r.text, "1 m");
    assert!(convert(r"\sisetup{locale=DE}", &Options { strict: true, ..Default::default() }).is_err());
    // the honored ones say nothing
    assert!(strict(r"\sisetup{per-mode = fraction, inter-unit-product = {\,}} x").warnings.is_empty());
}
