//! The browser's native View Transition API: morphs the DOM between two
//! states (here, a plate tile <-> its fullscreen panel, matched by the
//! `view-transition-name` each carries) without hand-rolled FLIP math.
//! Falls back to an instant swap where unsupported (Safari, older Firefox)
//! -- a real fallback, not a broken page.

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::window;

pub fn with_view_transition(update: impl FnOnce() + 'static) {
    let Some(doc) = window().and_then(|w| w.document()) else {
        update();
        return;
    };

    let start_fn = js_sys::Reflect::get(&doc, &JsValue::from_str("startViewTransition"))
        .ok()
        .filter(|v| !v.is_undefined())
        .and_then(|v| v.dyn_into::<js_sys::Function>().ok());

    let Some(start_fn) = start_fn else {
        update();
        return;
    };

    let cb = Closure::once_into_js(update);
    let _ = start_fn.call1(&doc, &cb);
}
