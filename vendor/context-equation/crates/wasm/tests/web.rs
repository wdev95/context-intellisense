//! Inside wasm, with `wasm-bindgen-test` on Node:
//! `cargo test -p context-equation-wasm --target wasm32-unknown-unknown` (needs wasm-bindgen-test-runner).
#![cfg(target_arch = "wasm32")]

use context_equation_wasm::{tex_to_svg, tex_to_tree};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn svg_and_tree() {
    assert!(tex_to_svg(r"\frac{1}{2}", None, None, None, None, None, None, None, None, None, None, None, None).unwrap().contains("1 over 2"));
    assert!(tex_to_svg(r"\frac{1}{2}", Some("pt-BR".into()), None, None, None, None, None, None, None, None, None, None, None).unwrap().contains("um meio"));
    assert!(tex_to_tree("x", None).unwrap().contains("identifier"));
    // max_width, in em: one <g data-line> per line
    let broken = tex_to_svg("a = b + c + d + e + f + g + h", None, None, None, None, None, None, None, None, Some(4.0), None, None, None).unwrap();
    assert!(broken.contains(r#"<g data-line="1">"#));
}

#[wasm_bindgen_test]
fn error_becomes_js_error() {
    assert!(tex_to_svg("x", Some("xx".into()), None, None, None, None, None, None, None, None, None, None, None).is_err());
    assert!(tex_to_tree(&format!("{}x{}", "{".repeat(100), "}".repeat(100)), None).is_err());
}

#[wasm_bindgen_test]
fn no_emoji_font_embedded() {
    // the npm package leaves out the `emoji` feature: fallback fonts come from the caller
    assert!(context_equation_core::font::FallbackFont::defaults().is_empty());
}
