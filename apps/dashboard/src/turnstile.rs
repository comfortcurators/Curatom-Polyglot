//! Renders the real Cloudflare Turnstile widget for curatom-kernel.
//! Site keys are public by design (Cloudflare embeds them client-side in
//! every integration guide); this one is scoped to curatom.rajvansh.dev
//! only, verified against the account's own Turnstile widget list.

use std::rc::Rc;

use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{window, HtmlScriptElement};

const SITE_KEY: &str = "0x4AAAAAAEwKSc4TRkI-kHxT";
const SCRIPT_SRC: &str = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

#[component]
pub fn TurnstileGate(on_token: Callback<String>) -> impl IntoView {
    let container: NodeRef<html::Div> = NodeRef::new();
    let error = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let Some(el) = container.get() else { return };
        let el: web_sys::Element = (*el).clone().into();

        let render_now: Rc<dyn Fn()> = Rc::new(move || {
            let Some(win) = window() else { return };
            let turnstile = match js_sys::Reflect::get(&win, &JsValue::from_str("turnstile")) {
                Ok(v) if !v.is_undefined() => v,
                _ => return,
            };
            let render_fn = match js_sys::Reflect::get(&turnstile, &JsValue::from_str("render")) {
                Ok(v) => v,
                Err(_) => return,
            };
            let Ok(render_fn) = render_fn.dyn_into::<js_sys::Function>() else {
                return;
            };

            let opts = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&opts, &"sitekey".into(), &SITE_KEY.into());
            let _ = js_sys::Reflect::set(&opts, &"theme".into(), &"dark".into());

            let cb = Closure::<dyn FnMut(JsValue)>::new(move |token: JsValue| {
                if let Some(t) = token.as_string() {
                    on_token.run(t);
                }
            });
            let _ = js_sys::Reflect::set(&opts, &"callback".into(), cb.as_ref());
            cb.forget();

            let error_cb = Closure::<dyn FnMut()>::new(move || {
                error.set(Some("Turnstile failed. Try again.".into()));
            });
            let _ = js_sys::Reflect::set(&opts, &"error-callback".into(), error_cb.as_ref());
            error_cb.forget();

            let _ = render_fn.call2(&turnstile, &el.clone().into(), &opts);
        });

        let already_loaded = window()
            .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("turnstile")).ok())
            .map(|v| !v.is_undefined())
            .unwrap_or(false);

        if already_loaded {
            render_now();
            return;
        }

        let doc = window().unwrap().document().unwrap();
        let selector = format!("script[src=\"{SCRIPT_SRC}\"]");
        if let Ok(Some(existing)) = doc.query_selector(&selector) {
            let existing: HtmlScriptElement = existing.unchecked_into();
            let go = render_now.clone();
            let cb = Closure::<dyn FnMut()>::new(move || go());
            existing.set_onload(Some(cb.as_ref().unchecked_ref()));
            cb.forget();
            return;
        }

        let script: HtmlScriptElement = doc.create_element("script").unwrap().unchecked_into();
        script.set_src(SCRIPT_SRC);
        script.set_async(true);
        let go = render_now.clone();
        let cb = Closure::<dyn FnMut()>::new(move || go());
        script.set_onload(Some(cb.as_ref().unchecked_ref()));
        cb.forget();
        let _ = doc.head().unwrap().append_child(&script);
    });

    view! {
        <div class="turnstile-wrap">
            <div node_ref=container style="display:flex;justify-content:center;min-height:66px"></div>
            {move || error.get().map(|e| view! { <div class="turnstile-error">{e}</div> })}
        </div>
    }
}
