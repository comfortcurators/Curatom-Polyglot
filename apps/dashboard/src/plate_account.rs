//! The missing "who am I, and how do I leave" plate. Every other plate
//! assumes you're already signed in; this is the one place that says so
//! out loud and gives you a way out.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn AccountPlate() -> impl IntoView {
    let user = RwSignal::new(None::<api::AuthUser>);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::auth_me().await {
                Ok(u) => user.set(Some(u)),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    });

    let sign_out = move |_| {
        busy.set(true);
        spawn_local(async move {
            let _ = api::logout().await;
            // A full reload is deliberate: App's own boot effect re-asks
            // /organic/me, and this is the one place that state should
            // come from a clean request rather than local signals this
            // plate would otherwise have to reset by hand.
            if let Some(win) = web_sys::window() {
                let _ = win.location().reload();
            }
        });
    };

    view! {
        <div style="padding:20px;display:flex;flex-direction:column;gap:16px">
            {move || match user.get() {
                Some(u) => view! {
                    <div class="card" style="padding:20px">
                        <p class="dim" style="font-size:12px;letter-spacing:1px;margin:0 0 6px">"SIGNED IN AS"</p>
                        <p style="font-size:20px;font-weight:700;margin:0">{u.username.clone()}</p>
                    </div>
                }.into_any(),
                None => view! {
                    <p class="dim" style="font-size:14px">
                        {move || error.get().unwrap_or_else(|| "…".into())}
                    </p>
                }.into_any(),
            }}
            <button class="btn" style="padding:14px" disabled=move || busy.get() on:click=sign_out>
                {move || if busy.get() { "…" } else { "SIGN OUT" }}
            </button>
        </div>
    }
}
