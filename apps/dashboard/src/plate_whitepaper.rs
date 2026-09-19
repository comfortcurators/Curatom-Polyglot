//! This account's own company context. Read and written via the
//! kernel's `get_whitepaper` / `set_whitepaper`, and readable by a
//! machine as the `company.whitepaper` resource when a knock for it is
//! approved.
//!
//! Per-account, not shared: the kernel stores this inside each owner's
//! own Durable Object, and no key belonging to any other owner can read
//! into it. `Kernel::set_whitepaper` documents the same property and a
//! test in `crates/key-kernel/src/lib.rs`
//! (`whitepaper_never_crosses_owners`) pins it.
//!
//! Empty text clears the whitepaper. The kernel stores that as `None`,
//! not `Some("")`, so a machine that knocks for it after a clear gets a
//! null result rather than an empty string.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

/// Kernel-side cap (`Kernel::WHITEPAPER_MAX_LEN`). Duplicated here
/// rather than fetched, because the only consequence of them drifting
/// is the countdown display being wrong -- the kernel is what actually
/// rejects an over-long save, and this client's own disabled-button
/// check is a courtesy, not a gate.
const MAX_LEN: usize = 20_000;

#[component]
pub fn WhitepaperPlate() -> impl IntoView {
    // `text` is what the textarea shows; `saved` is the last value the
    // server acknowledged. `dirty` is exactly `text != saved`, so a
    // freshly-loaded plate is never dirty and a save that fails leaves
    // the button enabled for a retry.
    let text = RwSignal::new(String::new());
    let saved = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    // Brief confirmation that auto-clears. Separate from `saving` so
    // the button can show "SAVED" for a moment without pretending a
    // save is still in flight.
    let just_saved = RwSignal::new(false);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::get_whitepaper().await {
                Ok(body) => {
                    let value = body.whitepaper.unwrap_or_default();
                    text.set(value.clone());
                    saved.set(value);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    });

    let dirty = move || text.get() != saved.get();
    let char_count = move || text.get().chars().count();
    let over_limit = move || char_count() > MAX_LEN;

    let save = move |_| {
        if saving.get() {
            return;
        }
        let value = text.get();
        saving.set(true);
        error.set(None);
        just_saved.set(false);
        spawn_local(async move {
            match api::set_whitepaper(&value).await {
                Ok(()) => {
                    saved.set(value);
                    saving.set(false);
                    just_saved.set(true);
                    // Auto-clear the confirmation. If the user types
                    // again before this fires, `dirty` flips true and
                    // the badge becomes irrelevant anyway; clearing it
                    // here just stops "saved" from lingering after
                    // they've clearly moved on.
                    gloo_timers::future::TimeoutFuture::new(1500).await;
                    just_saved.set(false);
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                    saving.set(false);
                }
            }
        });
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 4px;font-size:22px;font-weight:700">"Whitepaper"</h2>
            <p class="dim" style="font-size:13px;margin:0 0 16px">
                "This account's own company context. Readable as the "
                <span class="mono">"company.whitepaper"</span>
                " resource when a machine knocks for it. Private to you -- no other account can read it."
            </p>

            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}

            <textarea
                class="whitepaper-editor"
                placeholder="Write this account's context here: what the business is, what it does, who it serves. A machine asking for `company.whitepaper` reads exactly this text."
                prop:value=move || text.get()
                on:input=move |ev| text.set(event_target_value(&ev))
            ></textarea>

            <div class="row" style="margin-top:12px;align-items:center;justify-content:space-between">
                <span class="dim" style="font-size:12px">
                    {move || format!("{} / {}", char_count(), MAX_LEN)}
                    {move || over_limit().then(|| " — over the limit; save will be rejected".to_string())}
                </span>
                <div class="row" style="align-items:center">
                    {move || just_saved.get().then(|| view! {
                        <span class="ok" style="font-size:13px;padding:12px 4px">"saved"</span>
                    })}
                    <button
                        class="btn"
                        disabled=move || saving.get() || !dirty() || over_limit()
                        on:click=save
                    >
                        {move || if saving.get() { "…" } else { "SAVE" }}
                    </button>
                </div>
            </div>

            <p class="dim" style="font-size:12px;margin:8px 0 0">
                "Saving an empty or blank whitepaper clears it. The next machine that knocks for "
                <span class="mono">"company.whitepaper"</span>
                " gets nothing."
            </p>
        </div>
    }
}
