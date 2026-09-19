use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen_futures::JsFuture;
use web_sys::window;

use crate::api;
use crate::kit::Sheet;

fn copy_to_clipboard(text: &str) {
    if let Some(nav) = window().map(|w| w.navigator()) {
        let promise = nav.clipboard().write_text(text);
        spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValidityChoice {
    Never,
    Day,
    Week,
    Month,
}

impl ValidityChoice {
    fn seconds(self) -> Option<u64> {
        match self {
            ValidityChoice::Never => None,
            ValidityChoice::Day => Some(24 * 60 * 60),
            ValidityChoice::Week => Some(7 * 24 * 60 * 60),
            ValidityChoice::Month => Some(30 * 24 * 60 * 60),
        }
    }
    fn label(self) -> &'static str {
        match self {
            ValidityChoice::Never => "NEVER",
            ValidityChoice::Day => "24H",
            ValidityChoice::Week => "7D",
            ValidityChoice::Month => "30D",
        }
    }
}

/// What the key card shows in place of a timestamp, computed against the
/// browser's own clock. The kernel is authoritative; this is display.
fn validity_label(k: &api::KeyInfo) -> String {
    if k.expires_unix == 0 {
        return "never expires".to_string();
    }
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let remaining = k.expires_unix - now;
    if remaining <= 0 {
        return "expired".to_string();
    }
    if remaining < 3600 {
        return format!("{}m left", remaining / 60);
    }
    if remaining < 86_400 {
        return format!("{}h left", remaining / 3600);
    }
    format!("{}d left", remaining / 86_400)
}

#[derive(Clone)]
enum ConfirmAction {
    Reroll(api::KeyInfo),
    Delete(api::KeyInfo),
}

#[component]
pub fn KeysPlate() -> impl IntoView {
    let keys = RwSignal::new(Vec::<api::KeyInfo>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let creating = RwSignal::new(false);
    let detail = RwSignal::new(None::<api::KeyInfo>);
    let checkpoints_for = RwSignal::new(None::<api::KeyInfo>);
    let confirm = RwSignal::new(None::<ConfirmAction>);
    // Both create and reroll reveal a file-text block; one signal holds
    // whichever came last, titled so the sheet says which it is.
    let revealed = RwSignal::new(None::<(String, String)>);

    let refresh = move || {
        spawn_local(async move {
            match api::list_keys().await {
                Ok(list) => {
                    keys.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh());

    let do_confirm = move |action: ConfirmAction| {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match action {
                ConfirmAction::Reroll(k) => match api::reroll_key(&k.token).await {
                    Ok(created) => {
                        revealed.set(Some((
                            format!("Successor to \u{201c}{}\u{201d}", k.label),
                            created.file_text,
                        )));
                    }
                    Err(e) => error.set(Some(e.to_string())),
                },
                ConfirmAction::Delete(k) => match api::delete_key(&k.token).await {
                    Ok(()) => {}
                    Err(e) => error.set(Some(e.to_string())),
                },
            }
            confirm.set(None);
            busy.set(false);
            refresh();
        });
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 16px;font-size:22px;font-weight:700">"Keys"</h2>
            <button class="btn" style="width:100%;margin-bottom:16px" on:click=move |_| creating.set(true)>
                "+ NEW KEY"
            </button>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (keys.get().is_empty()).then(|| view! {
                <p class="dim">"No keys yet. Create one to hand to an AI."</p>
            })}
            <div style="display:flex;flex-direction:column;gap:10px">
                <For
                    each={move || keys.get()}
                    key=|k| k.token.clone()
                    children=move |k: api::KeyInfo| {
                        let k_for_log = k.clone();
                        let k_for_cp = k.clone();
                        let k_for_reroll = k.clone();
                        let k_for_delete = k.clone();
                        let validity = validity_label(&k);
                        let expired = k.expires_unix != 0 && {
                            let now = (js_sys::Date::now() / 1000.0) as i64;
                            k.expires_unix <= now
                        };
                        view! {
                            <div class="card">
                                <div style="display:flex;justify-content:space-between;align-items:center">
                                    <strong>{k.label.clone()}</strong>
                                    <span class="dim" style="font-size:12px">{k.activity_count} " activities"</span>
                                </div>
                                <p class="mono" style="margin:6px 0">
                                    {k.token.chars().take(24).collect::<String>()} "…"
                                </p>
                                <p class="dim" style="margin:0;font-size:12px">
                                    "created " {k.created_at.clone()}
                                    " · "
                                    <span style=if expired { "color:var(--urgent)" } else { "" }>{validity}</span>
                                </p>
                                {k.last_used_at.clone().map(|t| view! {
                                    <p class="dim" style="margin:0;font-size:12px">"last used " {t}</p>
                                })}
                                <div class="row" style="margin-top:10px">
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| detail.set(Some(k_for_log.clone()))
                                    >
                                        "LOG"
                                    </button>
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| checkpoints_for.set(Some(k_for_cp.clone()))
                                    >
                                        "CHECKPOINTS"
                                    </button>
                                </div>
                                <div class="row" style="margin-top:6px">
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| confirm.set(Some(ConfirmAction::Reroll(k_for_reroll.clone())))
                                    >
                                        "REROLL"
                                    </button>
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| confirm.set(Some(ConfirmAction::Delete(k_for_delete.clone())))
                                    >
                                        "DELETE"
                                    </button>
                                </div>
                            </div>
                        }
                    }
                />
            </div>

            {move || creating.get().then(|| view! {
                <CreateKeyModal
                    on_close=Callback::new(move |_| creating.set(false))
                    on_created=Callback::new(move |file_text| {
                        revealed.set(Some(("Give this to an AI".to_string(), file_text)));
                        creating.set(false);
                        refresh();
                    })
                />
            })}

            {move || confirm.get().map(|action| {
                let (heading, body, k) = match &action {
                    ConfirmAction::Reroll(k) => (
                        "Reroll this key?",
                        "A new key with the same label and the same lifetime is minted, and this one stops working immediately. Any AI holding it will be refused on its next knock.",
                        k.clone(),
                    ),
                    ConfirmAction::Delete(k) => (
                        "Delete this key?",
                        "This key stops working immediately, and its checkpoint history is removed. Its knock history, ledger entries, and billboard rows are kept — the record of what it did survives, the key itself does not.",
                        k.clone(),
                    ),
                };
                let label = k.label.clone();
                let action_for_yes = action.clone();
                view! {
                    <Sheet on_close=Callback::new(move |_| confirm.set(None))>
                        <h3 style="margin:0 0 8px">{heading}</h3>
                        <p class="mono" style="margin:0 0 10px">{label}</p>
                        <p class="dim" style="font-size:13px;margin:0 0 16px">{body}</p>
                        <div class="row">
                            <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| confirm.set(None)>
                                "CANCEL"
                            </button>
                            <button
                                class="btn"
                                disabled=move || busy.get()
                                on:click={
                                    let a = action_for_yes.clone();
                                    move |_| do_confirm(a.clone())
                                }
                            >
                                {move || if busy.get() { "…" } else { "CONFIRM" }}
                            </button>
                        </div>
                    </Sheet>
                }
            })}

            {move || revealed.get().map(|(heading, text)| {
                let text_for_copy = text.clone();
                view! {
                    <Sheet on_close=Callback::new(move |_| revealed.set(None))>
                        <h3 style="margin:0 0 12px">{heading.clone()}</h3>
                        <pre class="code-box">{text.clone()}</pre>
                        <button
                            class="btn"
                            style="width:100%"
                            on:click=move |_| { copy_to_clipboard(&text_for_copy); revealed.set(None); }
                        >
                            "COPY & CLOSE"
                        </button>
                    </Sheet>
                }
            })}

            {move || detail.get().map(|info| view! {
                <KeyLogModal info=info on_close=Callback::new(move |_| detail.set(None)) />
            })}

            {move || checkpoints_for.get().map(|info| view! {
                <CheckpointsModal info=info on_close=Callback::new(move |_| checkpoints_for.set(None)) />
            })}
        </div>
    }
}

#[component]
fn CreateKeyModal(on_close: Callback<()>, on_created: Callback<String>) -> impl IntoView {
    let label = RwSignal::new(String::new());
    let validity = RwSignal::new(ValidityChoice::Never);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit = move |_| {
        let value = label.get().trim().to_string();
        if value.is_empty() {
            return;
        }
        busy.set(true);
        error.set(None);
        let validity_seconds = validity.get().seconds();
        spawn_local(async move {
            match api::create_key(&value, validity_seconds).await {
                Ok(created) => {
                    label.set(String::new());
                    busy.set(false);
                    on_created.run(created.file_text);
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <Sheet on_close=on_close>
            <h3 style="margin:0 0 8px">"New key"</h3>
            <p class="dim" style="font-size:13px;margin-bottom:12px">
                "Name it however you like. This label is yours. The AI never sees it."
            </p>
            <input
                class="input"
                placeholder="e.g. Claude laptop, work key"
                prop:value=move || label.get()
                on:input=move |ev| label.set(event_target_value(&ev))
            />
            <p class="dim" style="font-size:12px;margin:12px 0 6px">"Validity"</p>
            <div class="row">
                {[
                    ValidityChoice::Never,
                    ValidityChoice::Day,
                    ValidityChoice::Week,
                    ValidityChoice::Month,
                ]
                .into_iter()
                .map(|v| {
                    view! {
                        <button
                            class=move || if validity.get() == v { "chip on" } else { "chip" }
                            on:click=move |_| validity.set(v)
                        >
                            {v.label()}
                        </button>
                    }
                })
                .collect_view()}
            </div>
            <p class="dim" style="font-size:12px;margin:8px 0 0">
                "NEVER means the key works until you revoke or delete it. "
                "Any other choice is a hard deadline."
            </p>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            <div class="row" style="margin-top:12px">
                <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| on_close.run(())>
                    "CANCEL"
                </button>
                <button
                    class="btn"
                    disabled=move || busy.get() || label.get().trim().is_empty()
                    on:click=submit
                >
                    {move || if busy.get() { "…" } else { "CREATE" }}
                </button>
            </div>
        </Sheet>
    }
}

#[component]
fn KeyLogModal(info: api::KeyInfo, on_close: Callback<()>) -> impl IntoView {
    let log = RwSignal::new(Vec::<api::KeyLogEntry>::new());
    let token = info.token.clone();

    Effect::new(move |_| {
        let token = token.clone();
        spawn_local(async move {
            if let Ok(entries) = api::key_log(&token).await {
                log.set(entries);
            }
        });
    });

    view! {
        <Sheet on_close=on_close>
            <h3 style="margin:0 0 4px">{info.label.clone()}</h3>
            <p class="dim" style="font-size:12px;margin-bottom:12px">{move || log.get().len()} " entries"</p>
            <div style="max-height:50vh;overflow-y:auto">
                <For
                    each={move || log.get().into_iter().enumerate().collect::<Vec<_>>()}
                    key=|(i, e)| (*i, e.at.clone())
                    children=move |(_, e): (usize, api::KeyLogEntry)| {
                        view! {
                            <div style="border-top:1px solid var(--line-soft);padding:8px 0">
                                <strong style="font-size:14px">{e.kind}</strong>
                                <p class="dim" style="margin:2px 0;font-size:12px">{e.at}</p>
                                {e.name.map(|n| view! { <p class="dim" style="margin:0;font-size:12px">"by " {n}</p> })}
                                {e.reason.map(|r| view! { <p class="dim" style="margin:0;font-size:12px">{r}</p> })}
                            </div>
                        }
                    }
                />
            </div>
            {move || (log.get().is_empty()).then(|| view! { <p class="dim">"No activity yet."</p> })}
            <button class="btn" style="width:100%;margin-top:12px" on:click=move |_| on_close.run(())>
                "CLOSE"
            </button>
        </Sheet>
    }
}

#[component]
fn CheckpointsModal(info: api::KeyInfo, on_close: Callback<()>) -> impl IntoView {
    let checkpoints = RwSignal::new(Vec::<api::Checkpoint>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let new_note = RwSignal::new(String::new());
    let token = info.token.clone();

    let refresh = {
        let token = token.clone();
        move || {
            let token = token.clone();
            spawn_local(async move {
                match api::list_checkpoints(&token).await {
                    Ok(list) => {
                        checkpoints.set(list);
                        error.set(None);
                    }
                    Err(e) => error.set(Some(e.to_string())),
                }
            });
        }
    };

    Effect::new(move |_| refresh());

    let create = {
        let token = token.clone();
        move |_| {
            let note = new_note.get().trim().to_string();
            if note.is_empty() {
                return;
            }
            busy.set(true);
            error.set(None);
            let token = token.clone();
            spawn_local(async move {
                match api::create_checkpoint(&token, &note).await {
                    Ok(_) => {
                        new_note.set(String::new());
                        busy.set(false);
                        // Re-run the fetch rather than pushing locally, so
                        // the sheet shows exactly what the kernel now holds
                        // (including the 200-entry cap's own eviction).
                        if let Ok(list) = api::list_checkpoints(&token).await {
                            checkpoints.set(list);
                        }
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        busy.set(false);
                    }
                }
            });
        }
    };

    view! {
        <Sheet on_close=on_close>
            <h3 style="margin:0 0 4px">{info.label.clone()}</h3>
            <p class="dim" style="font-size:12px;margin-bottom:12px">
                "Named points in this key's own history. The last one is current."
            </p>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || checkpoints.get().is_empty().then(|| view! {
                <p class="dim">"No checkpoints yet."</p>
            })}
            <div style="max-height:40vh;overflow-y:auto">
                <For
                    each={move || checkpoints.get().into_iter().rev().enumerate().collect::<Vec<_>>()}
                    key=|(i, c)| (*i, c.id.clone())
                    children=move |(i, c): (usize, api::Checkpoint)| {
                        let is_current = i == 0;
                        view! {
                            <div style="border-top:1px solid var(--line-soft);padding:8px 0">
                                <div style="display:flex;justify-content:space-between">
                                    <strong style="font-size:14px">
                                        {if is_current { "current" } else { "earlier" }}
                                    </strong>
                                    <span class="dim" style="font-size:12px">{c.created_at.clone()}</span>
                                </div>
                                <p class="dim" style="margin:2px 0 0;font-size:13px">{c.note.clone()}</p>
                            </div>
                        }
                    }
                />
            </div>
            <input
                class="input"
                style="margin-top:12px"
                placeholder="Note (e.g. before auth refactor)"
                prop:value=move || new_note.get()
                on:input=move |ev| new_note.set(event_target_value(&ev))
            />
            <div class="row" style="margin-top:8px">
                <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| on_close.run(())>
                    "CLOSE"
                </button>
                <button
                    class="btn"
                    disabled=move || busy.get() || new_note.get().trim().is_empty()
                    on:click=create
                >
                    {move || if busy.get() { "…" } else { "SAVE CHECKPOINT" }}
                </button>
            </div>
        </Sheet>
    }
}
