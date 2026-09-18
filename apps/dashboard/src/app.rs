use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::dashboard::Dashboard;
use crate::enrollment::Enrollment;
use crate::login::Login;
use crate::reset_password::ResetPassword;

/// The token from `?reset_token=...` on the current URL, if the emailed
/// reset link is what loaded this page. Read once, at module scope
/// rather than per-render, since it describes how this page load
/// started, not something that changes while the app runs.
fn reset_token_from_url() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search)
        .ok()?
        .get("reset_token")
}

#[derive(Clone)]
enum Boot {
    Loading,
    /// No session at all (a fresh visitor, or one who signed out) -- show
    /// the sign-in/register page rather than an error. `/organic/me`
    /// answers this the same 401 shape it always has; the only thing new
    /// is that this is now an expected, common state instead of a boot
    /// failure, because Access no longer sits in front of this domain to
    /// keep an anonymous visitor from ever reaching it.
    SignedOut,
    SignedIn(api::Me),
    /// A real failure -- the network, or the Worker itself -- not merely
    /// "nobody is signed in yet".
    Error(String),
}

#[component]
pub fn App() -> impl IntoView {
    let reset_token = RwSignal::new(reset_token_from_url());
    let boot = RwSignal::new(Boot::Loading);

    let refresh_me = move || {
        spawn_local(async move {
            match api::me().await {
                Ok(m) => boot.set(Boot::SignedIn(m)),
                Err(e) => {
                    // ApiError's shape is always "{path}: {status} {body}"
                    // (see api.rs's get()/post()), so "401 " right after the
                    // path is unambiguous -- not a substring that could
                    // appear elsewhere in a URL or an error body.
                    let msg = e.to_string();
                    if msg.contains(": 401 ") {
                        boot.set(Boot::SignedOut);
                    } else {
                        boot.set(Boot::Error(msg));
                    }
                }
            }
        });
    };

    Effect::new(move |_| refresh_me());

    view! {
        <div style="height:100vh">
            {move || {
                if let Some(token) = reset_token.get() {
                    return view! {
                        <ResetPassword
                            token=token
                            on_done=Callback::new(move |_| {
                                // The reset just created a session (see
                                // handle_password_reset_confirm), so the
                                // next step is the normal signed-in boot,
                                // not another sign-in form. Clearing the
                                // query param stops a page refresh from
                                // replaying an already-spent token.
                                if let Some(win) = web_sys::window() {
                                    let _ = win.history().and_then(|h| {
                                        h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/"))
                                    });
                                }
                                reset_token.set(None);
                                refresh_me();
                            })
                        />
                    }.into_any();
                }
                match boot.get() {
                    Boot::Error(err) => view! {
                        <div style="height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;padding:24px">
                            <p class="err" style="font-weight:700">"Could not reach Curatom."</p>
                            <p class="dim">{err}</p>
                            <button class="btn" style="padding:10px 18px" on:click=move |_| refresh_me()>
                                "RETRY"
                            </button>
                        </div>
                    }.into_any(),
                    Boot::SignedOut => view! {
                        <Login on_signed_in=Callback::new(move |_| refresh_me()) />
                    }.into_any(),
                    Boot::SignedIn(m) => if m.enrolled {
                        view! { <Dashboard /> }.into_any()
                    } else {
                        view! { <Enrollment on_enrolled=Callback::new(move |_| refresh_me()) /> }.into_any()
                    },
                    Boot::Loading => view! {
                        <div style="height:100%;display:flex;align-items:center;justify-content:center">"…"</div>
                    }.into_any(),
                }
            }}
        </div>
    }
}
