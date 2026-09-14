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

#[component]
pub fn KeysPlate() -> impl IntoView {
    let keys = RwSignal::new(Vec::<api::KeyInfo>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let creating = RwSignal::new(false);
    let detail = RwSignal::new(None::<api::KeyInfo>);
    let last_file = RwSignal::new(None::<String>);

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

    let revoke = move |token: String| {
        busy.set(true);
        spawn_local(async move {
            if let Err(e) = api::revoke_key(&token).await {
                error.set(Some(e.to_string()));
            }
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
                        let token_for_revoke = k.token.clone();
                        let k_for_detail = k.clone();
                        view! {
                            <div class="card">
                                <div style="display:flex;justify-content:space-between;align-items:center">
                                    <strong>{k.label.clone()}</strong>
                                    <span class="dim" style="font-size:12px">{k.activity_count} " activities"</span>
                                </div>
                                <p class="mono" style="margin:6px 0">
                                    {k.token.chars().take(24).collect::<String>()} "…"
                                </p>
                                <p class="dim" style="margin:0;font-size:12px">"created " {k.created_at.clone()}</p>
                                {k.last_used_at.clone().map(|t| view! {
                                    <p class="dim" style="margin:0;font-size:12px">"last used " {t}</p>
                                })}
                                <div class="row" style="margin-top:10px">
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| detail.set(Some(k_for_detail.clone()))
                                    >
                                        "LOG"
                                    </button>
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| revoke(token_for_revoke.clone())
                                    >
                                        "REVOKE"
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
                        last_file.set(Some(file_text));
                        creating.set(false);
                        refresh();
                    })
                />
            })}

            {move || last_file.get().map(|text| {
                let text_for_copy = text.clone();
                view! {
                    <Sheet on_close=Callback::new(move |_| last_file.set(None))>
                        <h3 style="margin:0 0 12px">"Give this to an AI"</h3>
                        <pre class="code-box">{text.clone()}</pre>
                        <button
                            class="btn"
                            style="width:100%"
                            on:click=move |_| { copy_to_clipboard(&text_for_copy); last_file.set(None); }
                        >
                            "COPY & CLOSE"
                        </button>
                    </Sheet>
                }
            })}

            {move || detail.get().map(|info| view! {
                <KeyLogModal info=info on_close=Callback::new(move |_| detail.set(None)) />
            })}
        </div>
    }
}

#[component]
fn CreateKeyModal(on_close: Callback<()>, on_created: Callback<String>) -> impl IntoView {
    let label = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit = move |_| {
        let value = label.get().trim().to_string();
        if value.is_empty() {
            return;
        }
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match api::create_key(&value).await {
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
