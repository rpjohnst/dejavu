#![cfg(target_arch = "wasm32")]

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct JsValue(usize);

impl Default for JsValue {
    fn default() -> JsValue { JsValue::UNDEFINED }
}

impl JsValue {
    pub const UNDEFINED: JsValue = JsValue(0);
}
