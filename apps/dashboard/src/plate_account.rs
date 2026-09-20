//! Who you are, and everything about staying that way: change your
//! password, reset it by email if you're locked out, manage passkeys,
//! sign out.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::passkey_browser;

#[component]
pub fn AccountPlate() -> impl IntoView {
    let user = RwSignal::new(None::<api::AuthUser>);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let refresh_user = move || {
        spawn_local(async move {
            match api::auth_me().await {
                Ok(u) => user.set(Some(u)),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh_user());

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
        <div style="padding:20px;display:flex;flex-direction:column;gap:20px">
            {move || match user.get() {
                Some(u) => view! {
                    <div class="card" style="padding:20px">
                        <p class="dim" style="font-size:12px;letter-spacing:1px;margin:0 0 6px">"SIGNED IN AS"</p>
                        <p style="font-size:20px;font-weight:700;margin:0">{u.username.clone()}</p>
                        {u.email.clone().map(|e| view! {
                            <p class="dim" style="font-size:13px;margin:4px 0 0">{e}</p>
                        })}
                    </div>
                }.into_any(),
                None => view! {
                    <p class="dim" style="font-size:14px">
                        {move || error.get().unwrap_or_else(|| "…".into())}
                    </p>
                }.into_any(),
            }}

            <ChangePasswordCard />
            <ResetPasswordCard email=Signal::derive(move || user.get().and_then(|u| u.email)) />
            <PasskeysCard />

            <button class="btn" style="padding:14px" disabled=move || busy.get() on:click=sign_out>
                {move || if busy.get() { "…" } else { "SIGN OUT" }}
            </button>
        </div>
    }
}

#[component]
fn ChangePasswordCard() -> impl IntoView {
    let current = RwSignal::new(String::new());
    let new_pw = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let done = RwSignal::new(false);

    let submit = move |_| {
        if new_pw.get().len() < 8 {
            error.set(Some("New password must be at least 8 characters.".into()));
            return;
        }
        if new_pw.get() != confirm.get() {
            error.set(Some("The two new passwords don't match.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        done.set(false);
        spawn_local(async move {
            match api::change_password(&current.get(), &new_pw.get()).await {
                Ok(()) => {
                    current.set(String::new());
                    new_pw.set(String::new());
                    confirm.set(String::new());
                    done.set(true);
                    busy.set(false);
                }
                Err(e) => {
                    let msg = e.to_string();
                    error.set(Some(if msg.contains("current_password_incorrect") {
                        "Current password is wrong.".into()
                    } else {
                        "Could not change password.".into()
                    }));
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <div class="card" style="padding:18px;display:flex;flex-direction:column;gap:8px">
            <p style="font-weight:700;font-size:15px;margin:0">"Change password"</p>
            <input
                class="input" type="password" placeholder="Current password"
                prop:value=move || current.get()
                on:input=move |ev| current.set(event_target_value(&ev))
            />
            <input
                class="input" type="password" placeholder="New password (min 8 characters)"
                prop:value=move || new_pw.get()
                on:input=move |ev| new_pw.set(event_target_value(&ev))
            />
            <input
                class="input" type="password" placeholder="Confirm new password"
                prop:value=move || confirm.get()
                on:input=move |ev| confirm.set(event_target_value(&ev))
            />
            {move || error.get().map(|e| view! { <p class="err" style="margin:0">{e}</p> })}
            {move || done.get().then(|| view! { <p class="dim" style="margin:0;font-size:13px">"Password changed."</p> })}
            <button class="btn-ghost" disabled=move || busy.get() on:click=submit>
                {move || if busy.get() { "…" } else { "CHANGE PASSWORD" }}
            </button>
        </div>
    }
}

#[component]
fn ResetPasswordCard(email: Signal<Option<String>>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let sent = RwSignal::new(false);

    let send = move |_| {
        let Some(addr) = email.get() else { return };
        busy.set(true);
        spawn_local(async move {
            let _ = api::password_reset_begin(&addr).await;
            busy.set(false);
            sent.set(true);
        });
    };

    view! {
        <div class="card" style="padding:18px;display:flex;flex-direction:column;gap:8px">
            <p style="font-weight:700;font-size:15px;margin:0">"Forgot your password?"</p>
            <p class="dim" style="font-size:13px;margin:0">
                "Send a reset link to " {move || email.get().unwrap_or_default()}
                ". Following it signs you in with a new password."
            </p>
            {move || sent.get().then(|| view! {
                <p class="dim" style="margin:0;font-size:13px">"Check your email."</p>
            })}
            <button
                class="btn-ghost"
                disabled=move || busy.get() || email.get().is_none()
                on:click=send
            >
                {move || if busy.get() { "…" } else { "SEND RESET EMAIL" }}
            </button>
        </div>
    }
}

#[component]
fn PasskeysCard() -> impl IntoView {
    let passkeys = RwSignal::new(Vec::<api::PasskeyInfo>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let supported = passkey_browser::supported();

    let refresh = move || {
        spawn_local(async move {
            if let Ok(list) = api::list_passkeys().await {
                passkeys.set(list);
            }
        });
    };

    Effect::new(move |_| refresh());

    let add = move |_| {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let result = async {
                let challenge = api::passkey_register_begin().await.map_err(|e| e.to_string())?;
                let reg = passkey_browser::create_passkey(
                    &challenge.challenge,
                    &challenge.rp_id,
                    "Curatom Polyglot",
                    &challenge.user_id,
                    &challenge.username,
                )
                .await?;
                api::passkey_register_finish(
                    &challenge.challenge,
                    &reg.client_data_json,
                    &reg.attestation_object,
                    "passkey",
                )
                .await
                .map_err(|e| e.to_string())
            }
            .await;
            match result {
                Ok(()) => {
                    busy.set(false);
                    refresh();
                }
                Err(e) => {
                    error.set(Some(format!("Could not add passkey: {e}")));
                    busy.set(false);
                }
            }
        });
    };

    let remove = move |credential_id: String| {
        busy.set(true);
        spawn_local(async move {
            if let Err(e) = api::delete_passkey(&credential_id).await {
                error.set(Some(e.to_string()));
            }
            busy.set(false);
            refresh();
        });
    };

    view! {
        <div class="card" style="padding:18px;display:flex;flex-direction:column;gap:8px">
            <p style="font-weight:700;font-size:15px;margin:0">"Passkeys"</p>
            {move || error.get().map(|e| view! { <p class="err" style="margin:0">{e}</p> })}
            {move || (passkeys.get().is_empty()).then(|| view! {
                <p class="dim" style="font-size:13px;margin:0">"No passkeys yet."</p>
            })}
            <div style="display:flex;flex-direction:column;gap:8px">
                <For
                    each={move || passkeys.get()}
                    key=|p| p.credential_id.clone()
                    children=move |p: api::PasskeyInfo| {
                        let id_for_remove = p.credential_id.clone();
                        view! {
                            <div style="display:flex;justify-content:space-between;align-items:center;border-top:1px solid var(--line-soft);padding-top:8px">
                                <div>
                                    <p style="margin:0;font-size:14px">{p.label.clone().unwrap_or_else(|| "passkey".into())}</p>
                                    <p class="dim" style="margin:2px 0 0;font-size:12px">
                                        {p.last_used_at.map(|_| "used before".to_string()).unwrap_or_else(|| "never used".to_string())}
                                    </p>
                                </div>
                                <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| remove(id_for_remove.clone())>
                                    "REMOVE"
                                </button>
                            </div>
                        }
                    }
                />
            </div>
            <Show when=move || supported>
                <button class="btn-ghost" disabled=move || busy.get() on:click=add>
                    {move || if busy.get() { "…" } else { "+ ADD PASSKEY" }}
                </button>
            </Show>
            <Show when=move || !supported>
                <p class="dim" style="font-size:12px;margin:0">"This browser doesn't support passkeys."</p>
            </Show>
        </div>
    }
}
