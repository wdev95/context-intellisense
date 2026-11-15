use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = renderEquation)]
pub fn render_equation(source: &str, display: bool) -> Result<String, JsError> {
    let options = context_equation_core::Options { display, ..Default::default() };
    context_equation_core::convert(source, &options)
        .map(|output| output.to_json())
        .map_err(|error| JsError::new(&error.to_string()))
}

