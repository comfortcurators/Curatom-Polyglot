//! CURATOM ENTERPRISE sign-up & sign-in page. Three states: sign in
//! (username/email + password, or a passkey), register (email only --
//! curatom-kernel mints the username and password after ZeptoMail
//! verification), and "check your email" after registering.

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::passkey_browser;
use crate::turnstile::TurnstileGate;

#[derive(Clone, Copy, PartialEq)]
enum UsernameCheck {
    Idle,
    Checking,
    Available,
    Taken,
    Invalid,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    SignIn,
    Register,
    CheckEmail,
    ForgotPassword,
    ResetSent,
}

#[component]
pub fn Login(on_signed_in: Callback<()>) -> impl IntoView {
    let mode = RwSignal::new(Mode::SignIn);
    let username_or_email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let register_email = RwSignal::new(String::new());
    let register_password = RwSignal::new(String::new());
    let register_username = RwSignal::new(String::new());
    let username_check = RwSignal::new(UsernameCheck::Idle);
    let username_generation = RwSignal::new(0u32);
    let forgot_email = RwSignal::new(String::new());
    let turnstile_token = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let passkey_supported = RwSignal::new(passkey_browser::supported());
    // A separate signal from `turnstile_token` above: Turnstile tokens
    // are single-use, and register/login are two different ceremonies.
    // Sharing one signal meant registering, then switching to sign-in,
    // found a spent token and got a 403 the user could not see the cause
    // of. The register gate resets its own on mode change, this one
    // resets its own.
    let login_turnstile_token = RwSignal::new(None::<String>);

    let do_login = move || {
        let user = username_or_email.get();
        let pass = password.get();
        if user.trim().is_empty() || pass.is_empty() {
            error.set(Some("Enter your username or email, and your password.".into()));
            return;
        }
        let Some(ts) = login_turnstile_token.get() else {
            error.set(Some("Complete the verification above first.".into()));
            return;
        };
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match api::login(&user, &pass, &ts).await {
                Ok(()) => on_signed_in.run(()),
                Err(e) => {
                    // Turnstile tokens are single-use: a failed password
                    // burns the token along with the attempt. Force a
                    // fresh one so the next click is not silently
                    // rejected by Siteverify.
                    login_turnstile_token.set(None);
                    error.set(Some(readable_login_error(&e.to_string())));
                    busy.set(false);
                }
            }
        });
    };

    let do_passkey_login = move || {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            let result = async {
                let challenge = api::passkey_login_begin()
                    .await
                    .map_err(|e| e.to_string())?;
                let assertion = passkey_browser::get_passkey(&challenge.challenge, &challenge.rp_id)
                    .await?;
                api::passkey_login_finish(
                    &challenge.challenge,
                    &assertion.credential_id,
                    &assertion.client_data_json,
                    &assertion.authenticator_data,
                    &assertion.signature,
                )
                .await
                .map_err(|e| e.to_string())
            }
            .await;

            match result {
                Ok(()) => on_signed_in.run(()),
                Err(e) => {
                    error.set(Some(format!("Passkey sign-in failed: {e}")));
                    busy.set(false);
                }
            }
        });
    };

    let on_username_input = move |ev: web_sys::Event| {
        let value = event_target_value(&ev);
        register_username.set(value.clone());
        let my_gen = username_generation.get() + 1;
        username_generation.set(my_gen);
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            username_check.set(UsernameCheck::Idle);
            return;
        }
        username_check.set(UsernameCheck::Checking);
        spawn_local(async move {
            // Debounced: wait for typing to pause, then bail if another
            // keystroke already started a newer check -- otherwise a
            // slow response for an earlier, already-stale value could
            // land after a faster one for the current value and flash
            // the wrong verdict.
            TimeoutFuture::new(400).await;
            if username_generation.get() != my_gen {
                return;
            }
            let result = api::username_available(&trimmed).await;
            if username_generation.get() != my_gen {
                return;
            }
            match result {
                Ok(a) if a.available => username_check.set(UsernameCheck::Available),
                Ok(a) if a.reason.as_deref() == Some("invalid_format") => {
                    username_check.set(UsernameCheck::Invalid)
                }
                Ok(_) => username_check.set(UsernameCheck::Taken),
                Err(_) => username_check.set(UsernameCheck::Idle),
            }
        });
    };

    let do_register = move || {
        let email = register_email.get();
        let password = register_password.get();
        let username = register_username.get();
        if email.trim().is_empty() {
            error.set(Some("Enter your email.".into()));
            return;
        }
        if password.len() < 8 {
            error.set(Some("Password must be at least 8 characters.".into()));
            return;
        }
        if !username.trim().is_empty() {
            match username_check.get() {
                UsernameCheck::Taken => {
                    error.set(Some("That username is taken.".into()));
                    return;
                }
                UsernameCheck::Invalid => {
                    error.set(Some("Username must be 3-32 characters: letters, numbers, - or _.".into()));
                    return;
                }
                UsernameCheck::Checking => {
                    error.set(Some("Still checking that username -- wait a moment.".into()));
                    return;
                }
                UsernameCheck::Idle | UsernameCheck::Available => {}
            }
        }
        let Some(_) = turnstile_token.get() else {
            error.set(Some("Complete the verification above first.".into()));
            return;
        };
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match api::register(&email, &password, &username).await {
                Ok(()) => {
                    mode.set(Mode::CheckEmail);
                    busy.set(false);
                }
                Err(e) => {
                    error.set(Some(readable_register_error(&e.to_string())));
                    busy.set(false);
                }
            }
        });
    };

    let do_forgot_password = move || {
        let email = forgot_email.get();
        if email.trim().is_empty() {
            error.set(Some("Enter your email.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            // Always succeeds from the caller's point of view -- the
            // Worker answers the same way whether or not the email is
            // registered, so this can't be used to find out who has an
            // account here.
            let _ = api::password_reset_begin(&email).await;
            mode.set(Mode::ResetSent);
            busy.set(false);
        });
    };

    view! {
        <div style="height:100%;display:flex;flex-direction:column;justify-content:center;padding:24px;max-width:420px;margin:0 auto">
            <h1 style="font-size:26px;font-weight:700;margin:0 0 6px;letter-spacing:0.02em">"CURATOM ENTERPRISE"</h1>
            <p class="dim" style="font-size:14px;margin:0 0 28px">
                "Headquarters for man-made intelligences."
            </p>

            {move || error.get().map(|e| view! { <p class="err" style="margin-bottom:16px">{e}</p> })}

            <Show when=move || mode.get() == Mode::SignIn>
                <div style="display:flex;flex-direction:column;gap:10px">
                    <input
                        class="input"
                        placeholder="Username or email"
                        prop:value=move || username_or_email.get()
                        on:input=move |ev| username_or_email.set(event_target_value(&ev))
                    />
                    <input
                        class="input"
                        type="password"
                        placeholder="Password"
                        prop:value=move || password.get()
                        on:input=move |ev| password.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" { do_login(); }
                        }
                    />
                    <TurnstileGate on_token=Callback::new(move |t| login_turnstile_token.set(Some(t))) />
                    <button class="btn" style="padding:14px" disabled=move || busy.get() on:click=move |_| do_login()>
                        {move || if busy.get() { "…" } else { "SIGN IN" }}
                    </button>
                    <button
                        class="btn-ghost"
                        style="padding:8px;font-size:13px"
                        disabled=move || busy.get()
                        on:click=move |_| {
                            error.set(None);
                            login_turnstile_token.set(None);
                            mode.set(Mode::ForgotPassword);
                        }
                    >
                        "Forgot password?"
                    </button>

                    <Show when=move || passkey_supported.get()>
                        <button class="btn-ghost" style="padding:14px" disabled=move || busy.get() on:click=move |_| do_passkey_login()>
                            "USE A PASSKEY"
                        </button>
                    </Show>

                    <button
                        class="btn-ghost"
                        style="padding:14px;margin-top:8px"
                        disabled=move || busy.get()
                        on:click=move |_| {
                            error.set(None);
                            login_turnstile_token.set(None);
                            mode.set(Mode::Register);
                        }
                    >
                        "CREATE AN ACCOUNT"
                    </button>
                </div>
            </Show>

            <Show when=move || mode.get() == Mode::Register>
                <div style="display:flex;flex-direction:column;gap:10px">
                    <p class="dim" style="font-size:14px;line-height:1.6;margin:0 0 4px">
                        "Choose your password now. Curatom sends a verification link -- "
                        "follow it to activate your account."
                    </p>
                    <input
                        class="input"
                        placeholder="Email"
                        prop:value=move || register_email.get()
                        on:input=move |ev| register_email.set(event_target_value(&ev))
                    />
                    <input
                        class="input"
                        type="password"
                        placeholder="Password (min 8 characters)"
                        prop:value=move || register_password.get()
                        on:input=move |ev| register_password.set(event_target_value(&ev))
                    />
                    <input
                        class="input"
                        placeholder="Username (optional -- leave blank to get one minted from your email)"
                        prop:value=move || register_username.get()
                        on:input=on_username_input
                    />
                    {move || match username_check.get() {
                        UsernameCheck::Checking => Some(view! { <p class="dim" style="margin:0;font-size:12px">"checking…"</p> }.into_any()),
                        UsernameCheck::Available => Some(view! { <p style="margin:0;font-size:12px;color:#4ade80">"available"</p> }.into_any()),
                        UsernameCheck::Taken => Some(view! { <p class="err" style="margin:0;font-size:12px">"already taken"</p> }.into_any()),
                        UsernameCheck::Invalid => Some(view! { <p class="err" style="margin:0;font-size:12px">"3-32 characters: letters, numbers, - or _"</p> }.into_any()),
                        UsernameCheck::Idle => None,
                    }}
                    <TurnstileGate on_token=Callback::new(move |t| turnstile_token.set(Some(t))) />
                    <button class="btn" style="padding:14px" disabled=move || busy.get() on:click=move |_| do_register()>
                        {move || if busy.get() { "…" } else { "REGISTER" }}
                    </button>
                    <button
                        class="btn-ghost"
                        style="padding:14px"
                        disabled=move || busy.get()
                        on:click=move |_| { error.set(None); mode.set(Mode::SignIn); }
                    >
                        "BACK TO SIGN IN"
                    </button>
                </div>
            </Show>

            <Show when=move || mode.get() == Mode::CheckEmail>
                <div class="card" style="text-align:center;padding:28px 20px">
                    <p style="font-size:16px;font-weight:600;margin:0 0 10px">"Check your email"</p>
                    <p class="dim" style="font-size:14px;line-height:1.6;margin:0">
                        "Follow the link we sent to "<strong>{move || register_email.get()}</strong>
                        " to activate your account. Sign in with the password you just chose. "
                        "The link expires in 30 minutes."
                    </p>
                </div>
                <button
                    class="btn-ghost"
                    style="padding:14px;margin-top:14px"
                    on:click=move |_| { error.set(None); mode.set(Mode::SignIn); }
                >
                    "BACK TO SIGN IN"
                </button>
            </Show>

            <Show when=move || mode.get() == Mode::ForgotPassword>
                <div style="display:flex;flex-direction:column;gap:10px">
                    <p class="dim" style="font-size:14px;line-height:1.6;margin:0 0 4px">
                        "Enter your email. If it's registered, we'll send a link to set a new password."
                    </p>
                    <input
                        class="input"
                        placeholder="Email"
                        prop:value=move || forgot_email.get()
                        on:input=move |ev| forgot_email.set(event_target_value(&ev))
                        on:keydown=move |ev| { if ev.key() == "Enter" { do_forgot_password(); } }
                    />
                    <button class="btn" style="padding:14px" disabled=move || busy.get() on:click=move |_| do_forgot_password()>
                        {move || if busy.get() { "…" } else { "SEND RESET LINK" }}
                    </button>
                    <button
                        class="btn-ghost"
                        style="padding:14px"
                        disabled=move || busy.get()
                        on:click=move |_| { error.set(None); mode.set(Mode::SignIn); }
                    >
                        "BACK TO SIGN IN"
                    </button>
                </div>
            </Show>

            <Show when=move || mode.get() == Mode::ResetSent>
                <div class="card" style="text-align:center;padding:28px 20px">
                    <p style="font-size:16px;font-weight:600;margin:0 0 10px">"Check your email"</p>
                    <p class="dim" style="font-size:14px;line-height:1.6;margin:0">
                        "If " <strong>{move || forgot_email.get()}</strong> " is registered, "
                        "a reset link is on its way. It expires in 30 minutes."
                    </p>
                </div>
                <button
                    class="btn-ghost"
                    style="padding:14px;margin-top:14px"
                    on:click=move |_| { error.set(None); mode.set(Mode::SignIn); }
                >
                    "BACK TO SIGN IN"
                </button>
            </Show>
        </div>
    }
}

fn readable_login_error(raw: &str) -> String {
    if raw.contains("invalid_credentials") {
        "That username/email or password isn't right.".into()
    } else {
        "Could not sign in. Try again.".into()
    }
}

fn readable_register_error(raw: &str) -> String {
    if raw.contains("email_already_registered") {
        "An account already exists for that email. Try signing in instead.".into()
    } else if raw.contains("invalid_email") {
        "That doesn't look like a real email address.".into()
    } else if raw.contains("password_too_short") {
        "Password must be at least 8 characters.".into()
    } else if raw.contains("username_taken") {
        "That username is taken. Try a different one.".into()
    } else if raw.contains("invalid_username") {
        "Username must be 3-32 characters: letters, numbers, - or _.".into()
    } else if raw.contains("email_not_configured") || raw.contains("email_send_failed") {
        "Could not send the verification email. Try again shortly.".into()
    } else {
        "Could not register. Try again.".into()
    }
}
