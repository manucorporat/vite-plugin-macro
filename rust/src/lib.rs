#![feature(box_patterns)]

mod collect;
mod parse;
mod transform;

use parse::{transform_code, TransformCodeOptions};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn get_macro_locations(
    code: String,
    filename: String,
    assert_type: String,
    js_filter: js_sys::Function,
) -> Result<JsValue, JsValue> {
    let filter = Box::new(move |name: String, id: String| {
        let this = JsValue::null();
        let name = JsValue::from(name);
        let id = JsValue::from(id);
        js_filter
            .call2(&this, &name, &id)
            .unwrap()
            .as_bool()
            .unwrap()
    });
    let result = transform_code(TransformCodeOptions {
        absolute_path: filename,
        code,
        assert_type,
        filter,
    })
    .unwrap();
    Ok(serde_wasm_bindgen::to_value(&result)?)
}
