//! Outside the browser: the happy path of the exported functions, and the errors before they become
//! a `JsError` (creating a `JsError` only works inside wasm; that is in tests/web.rs).

use context_equation_wasm::{convert, tex_to_svg, tex_to_tree};

#[test]
fn happy_path() {
    assert!(tex_to_svg("x", None, None, None, None, None, None, None, None, None, None, None, None).unwrap().starts_with("<svg"));
    assert!(tex_to_tree("x", Some("pt-BR".into())).unwrap().contains("identifier"));
}

#[test]
fn errors_become_text() {
    assert!(convert("x", Some("xx")).unwrap_err().contains("unknown locale"));
    let deep = format!("{}x{}", "{".repeat(100), "}".repeat(100));
    assert!(convert(&deep, None).unwrap_err().contains("nesting"));
}

#[test]
fn render_carries_every_output() {
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json(r"\vert A\vert}", None, true, false, None, None, false, false).unwrap()).unwrap();
    assert!(json["svg"].as_str().unwrap().starts_with("<svg"));
    assert!(json["speech"].as_str().unwrap().contains("absolute value"));
    assert_eq!(json["warnings"][0]["rule"], "E-REC-4");
    assert!(context_equation_wasm::render_json(r"\vert A\vert}", None, true, true, None, None, false, false).is_err());
}

#[test]
fn locales_lists_every_language() {
    let list: serde_json::Value = serde_json::from_str(&context_equation_wasm::locales_json()).unwrap();
    assert_eq!(list[0]["code"], "en");
    assert!(list.as_array().unwrap().iter().any(|l| l["code"] == "ja" && l["name"] == "日本語"));
}

#[test]
fn input_formats() {
    assert!(tex_to_svg("sqrt(x)", None, None, None, Some("asciimath".into()), None, None, None, None, None, None, None, None).unwrap().contains("square root of x"));
    let json: serde_json::Value =
        serde_json::from_str(&context_equation_wasm::render_json("<math><mi>x</mi></math>", None, true, true, Some("mathml"), None, false, false).unwrap()).unwrap();
    assert_eq!(json["text"], "x");
    assert!(context_equation_wasm::convert_with("x", None, true, false, Some("word"), None, false, false).unwrap_err().contains("unknown input format"));
}

#[test]
fn braille_in_render() {
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json(r"\frac{3}{4}", Some("pt-BR"), true, false, None, None, false, false).unwrap()).unwrap();
    assert_eq!(json["braille"], "⠼⠒⠙");
    assert_eq!(json["braille_code"], "cmu");
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json(r"\frac{3}{4}", Some("pt-BR"), true, false, None, Some("nemeth"), false, false).unwrap()).unwrap();
    assert_eq!(json["braille_code"], "nemeth");
    assert!(context_equation_wasm::convert_with("x", None, true, false, None, Some("ueb"), false, false).unwrap_err().contains("unknown braille code"));
}

#[test]
fn mathml_in_render() {
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json(r"\sen x", Some("pt-BR"), false, false, None, None, false, false).unwrap()).unwrap();
    let mathml = json["mathml"].as_str().unwrap();
    assert!(mathml.starts_with(r#"<math xmlns="http://www.w3.org/1998/Math/MathML" display="inline">"#), "{mathml}");
    assert!(mathml.contains(r#"<mi intent="sine">sen</mi>"#), "{mathml}");
}

#[test]
fn animate_and_steps() {
    let plain = tex_to_svg("x+1", None, None, None, None, None, None, None, None, None, None, None, None).unwrap();
    let animated = tex_to_svg("x+1", None, None, None, None, None, Some(true), None, None, None, None, None, None).unwrap();
    let steps = tex_to_svg("x+1", None, None, None, None, None, None, Some(true), None, None, None, None, None).unwrap();
    assert!(!plain.contains("data-step"));
    assert!(animated.contains("@keyframes mr-f") && animated.contains(r#"data-steps="3""#));
    assert!(steps.contains(r#"data-steps="3""#) && !steps.contains("@keyframes"));
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json("x", None, true, false, None, None, true, false).unwrap()).unwrap();
    assert!(json["svg"].as_str().unwrap().contains("mr-f"));
}

#[test]
fn max_width_breaks_lines() {
    let tex = "a = b + c + d + e + f + g + h";
    let broken = context_equation_wasm::convert_in(tex, None, true, false, None, None, false, false, None, Some(4.0)).unwrap();
    let plain = context_equation_wasm::convert_in(tex, None, true, false, None, None, false, false, None, None).unwrap();
    assert!(broken.svg.contains(r#"<g data-line="1">"#) && !plain.svg.contains("data-line"));
    assert_eq!((broken.speech, broken.braille), (plain.speech, plain.braille));
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::render_json_in(tex, None, true, false, None, None, false, false, None, Some(4.0)).unwrap()).unwrap();
    assert_eq!(json["applied"].as_array().unwrap().iter().filter(|a| a["rule"] == "L-BRK-1").count(), 1);
}

#[test]
fn labels_and_citations_as_json() {
    let labels = Some(r#"{"eq:a": 3, "eq:b": "4a"}"#);
    let citations = Some(r#"{"knuth": 1}"#);
    let r = context_equation_wasm::convert_document(r"\eqref{eq:a} \ref{eq:b} \cite{knuth}", None, true, true, None, None, false, false, None, labels, citations).unwrap();
    assert_eq!(r.text, "(3)4a[1]");
    let r = context_equation_wasm::convert_document(r"\eqref{eq:z}", None, true, false, None, None, false, false, None, labels, None).unwrap();
    assert_eq!((r.text.as_str(), r.warnings[0].rule), ("(??)", "E-REC-9"));
    assert!(context_equation_wasm::convert_document(r"\eqref{eq:a}", None, true, false, None, None, false, false, None, Some("[1]"), None).unwrap_err().contains("labels"));
    assert!(context_equation_wasm::convert_document(r"\cite{k}", None, true, false, None, None, false, false, None, None, Some(r#"{"k": [1]}"#)).unwrap_err().contains("citations"));
}

#[test]
fn a_loaded_font_by_its_id() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fonts/latinmodern-math.otf")).unwrap();
    let (id, name) = context_equation_wasm::load_font_bytes(bytes).unwrap();
    assert_eq!(name, "Latin Modern Math");
    let svg = context_equation_wasm::convert_in("x", None, true, false, None, None, false, false, Some(id), None).unwrap().svg;
    assert!(svg.contains("<contextEquation:font>Latin Modern Math</contextEquation:font>"));
    assert!(context_equation_wasm::pen_svg("x", None, true, None, None, Some(id)).is_ok());
    assert!(context_equation_wasm::convert_in("x", None, true, false, None, None, false, false, Some(999), None).unwrap_err().contains("no font loaded"));
}

#[test]
fn fallback_fonts_by_id() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fonts/fallback-a.ttf")).unwrap();
    let (id, name) = context_equation_wasm::load_fallback_font_bytes(bytes).unwrap();
    assert_eq!(name, "fallback-a");
    let out = context_equation_wasm::convert_full(r"\text{中}", None, true, true, None, None, false, false, None, None, &[id as i32], None, None).unwrap();
    assert!(out.warnings.is_empty() && out.svg.contains("-f"));
    // a math font from load_font, as -(id + 1)
    let lm = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fonts/latinmodern-math.otf")).unwrap();
    let (math, _) = context_equation_wasm::load_font_bytes(lm).unwrap();
    assert!(context_equation_wasm::convert_full("x", None, true, false, None, None, false, false, None, None, &[-(math as i32) - 1], None, None).is_ok());
    assert!(context_equation_wasm::convert_full("x", None, true, false, None, None, false, false, None, None, &[999], None, None).unwrap_err().contains("no fallback font loaded"));
    assert!(context_equation_wasm::pen_svg_with(r"\text{中}", None, true, None, None, None, &[id as i32]).is_ok());
}

/// The JavaScript order of the trailing parameters: `font`, `max_width`, `fallback_fonts`, `labels`,
/// `citations` (the older ones first, so earlier positional callers keep working).
#[test]
fn every_trailing_parameter_in_order() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fonts/fallback-a.ttf")).unwrap();
    let (id, _) = context_equation_wasm::load_fallback_font_bytes(bytes).unwrap();
    let tex = r"\text{中} = \eqref{eq:a} + b + c + d + e + f + g + h \cite{knuth}";
    let (labels, citations) = (Some(r#"{"eq:a": 3}"#.to_string()), Some(r#"{"knuth": 1}"#.to_string()));
    let svg = tex_to_svg(tex, None, None, Some(true), None, None, None, None, None, Some(4.0), Some(vec![id as i32]), labels.clone(), citations.clone()).unwrap();
    assert!(svg.contains(r#"<g data-line="1">"#) && svg.contains("M100 -100 L900 -100"));
    let json = context_equation_wasm::render(tex, None, None, Some(true), None, None, None, None, None, Some(4.0), Some(vec![id as i32]), labels, citations).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let text = json["text"].as_str().unwrap();
    assert!(text.contains("(3)") && text.contains("[1]"), "{text}");
    assert!(json["mathml"].as_str().unwrap().starts_with("<math"));
}

#[test]
fn locales_is_the_json_list() {
    assert_eq!(context_equation_wasm::locales(), context_equation_wasm::locales_json());
}

#[test]
fn load_font_and_load_fallback_font_answer_json() {
    let lm = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fonts/latinmodern-math.otf")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::load_font(lm.clone()).unwrap()).unwrap();
    assert_eq!(json["name"], "Latin Modern Math");
    // the same bytes again: the same id
    assert_eq!(json["id"], context_equation_wasm::load_font_bytes(lm).unwrap().0);
    let a = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../core/tests/fonts/fallback-a.ttf")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&context_equation_wasm::load_fallback_font(a.clone()).unwrap()).unwrap();
    assert_eq!((json["name"].as_str(), json["fallback"].as_bool()), (Some("fallback-a"), Some(true)));
    assert_eq!(json["id"], context_equation_wasm::load_fallback_font_bytes(a).unwrap().0);
    // what is not a font is refused before it becomes a `JsError`
    assert!(context_equation_wasm::load_font_bytes(b"not a font".to_vec()).unwrap_err().contains("not a font"));
    assert!(context_equation_wasm::load_fallback_font_bytes(b"not a font".to_vec()).unwrap_err().contains("not a font"));
    assert!(context_equation_wasm::font_by_id(Some(999)).unwrap_err().contains("no font loaded with id 999"));
}

#[test]
fn tex_to_pen_writes_the_equation() {
    let svg = context_equation_wasm::tex_to_pen("x+1", None, None, None, Some(2.0), None, None).unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("<animate"), "{svg}");
    let inline = context_equation_wasm::tex_to_pen("sqrt(x)", Some("pt-BR".into()), Some(false), Some("asciimath".into()), None, None, Some(vec![])).unwrap();
    assert!(inline.starts_with("<svg"));
    assert!(context_equation_wasm::pen_svg("x", Some("xx"), true, None, None, None).unwrap_err().contains("unknown locale"));
    assert!(context_equation_wasm::pen_svg("x", None, true, Some("word"), None, None).unwrap_err().contains("unknown input format"));
    assert!(context_equation_wasm::pen_svg("x", None, true, None, None, Some(999)).unwrap_err().contains("no font loaded"));
    assert!(context_equation_wasm::pen_svg_with("x", None, true, None, None, None, &[999]).unwrap_err().contains("no fallback font loaded"));
    // a math font id as a fallback (-(id + 1)) that no load_font gave
    assert!(context_equation_wasm::pen_svg_with("x", None, true, None, None, None, &[-1000]).unwrap_err().contains("no font loaded with id 999"));
    let deep = format!("{}x{}", "{".repeat(100), "}".repeat(100));
    assert!(context_equation_wasm::pen_svg(&deep, None, true, None, None, None).unwrap_err().contains("nesting"));
}

#[test]
fn labels_must_be_numbers() {
    assert!(context_equation_wasm::numbers(None, "labels").unwrap().is_empty());
    assert!(context_equation_wasm::numbers(Some("{"), "labels").unwrap_err().starts_with("labels: "));
    assert_eq!(context_equation_wasm::numbers(Some(r#"{"a": true}"#), "labels").unwrap_err(), "labels: a is true, not a string or a number");
}

/// The exported functions turn a refused input into a `JsError`, which only a wasm host can create:
/// outside it, wasm-bindgen panics there. That panic shows the error path was taken (no SVG came
/// out); the error text itself is checked above and in tests/web.rs.
#[test]
fn refused_input_reaches_js_error() {
    fn refused(f: impl FnOnce() + std::panic::UnwindSafe) {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let caught = std::panic::catch_unwind(f);
        std::panic::set_hook(hook);
        let payload = caught.expect_err("the input is refused");
        let message = payload.downcast_ref::<String>().map(String::as_str).or_else(|| payload.downcast_ref::<&str>().copied()).unwrap_or_default();
        assert!(message.contains("wasm"), "{message}");
    }
    refused(|| drop(context_equation_wasm::load_font(b"not a font".to_vec())));
    refused(|| drop(context_equation_wasm::load_fallback_font(b"not a font".to_vec())));
    refused(|| drop(tex_to_svg("x", Some("xx".into()), None, None, None, None, None, None, None, None, None, None, None)));
    refused(|| drop(tex_to_tree("x", Some("xx".into()))));
    refused(|| drop(context_equation_wasm::render("x", Some("xx".into()), None, None, None, None, None, None, None, None, None, None, None)));
    refused(|| drop(context_equation_wasm::tex_to_pen("x", Some("xx".into()), None, None, None, None, None)));
}
