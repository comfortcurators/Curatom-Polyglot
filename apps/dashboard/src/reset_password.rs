//! Lands when the emailed reset link is followed
//! (`/?reset_token=...`). Collects the new password and completes the
//! reset in one POST -- see `auth_users::handle_password_reset_confirm`
//! for why nothing here runs on the GET that loaded this page.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn ResetPassword(token: String, on_done: Callback<()>) -> impl IntoView {
    let new_pw = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit = std::rc::Rc::new(move || {
        if new_pw.get().len() < 8 {
            error.set(Some("Password must be at least 8 characters.".into()));
            return;
        }
        if new_pw.get() != confirm.get() {
            error.set(Some("The two passwords don't match.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        let token = token.clone();
        spawn_local(async move {
            match api::password_reset_confirm(&token, &new_pw.get()).await {
                Ok(()) => on_done.run(()),
                Err(e) => {
                    let msg = e.to_string();
                    error.set(Some(if msg.contains("unknown_or_expired") || msg.contains("token_expired") {
                        "This reset link has expired or was already used. Request a new one.".into()
                    } else {
                        "Could not reset password. Try again.".into()
                    }));
                    busy.set(false);
                }
            }
        });
    });

    view! {
        <div style="height:100%;display:flex;flex-direction:column;justify-content:center;padding:24px;max-width:420px;margin:0 auto">
            <h1 style="font-size:24px;font-weight:700;margin:0 0 6px">"Set a new password"</h1>
            <p class="dim" style="font-size:14px;margin:0 0 20px">
                "Choose a new password for your Curatom account."
            </p>
            <div style="display:flex;flex-direction:column;gap:10px">
                <input
                    class="input" type="password" placeholder="New password (min 8 characters)"
                    prop:value=move || new_pw.get()
                    on:input=move |ev| new_pw.set(event_target_value(&ev))
                />
                <input
                    class="input" type="password" placeholder="Confirm new password"
                    prop:value=move || confirm.get()
                    on:input=move |ev| confirm.set(event_target_value(&ev))
                    on:keydown={
                        let submit = submit.clone();
                        move |ev| { if ev.key() == "Enter" { submit(); } }
                    }
                />
                {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
                <button
                    class="btn" style="padding:14px" disabled=move || busy.get()
                    on:click=move |_| submit()
                >
                    {move || if busy.get() { "…" } else { "SET PASSWORD" }}
                </button>
            </div>
        </div>
    }
}
