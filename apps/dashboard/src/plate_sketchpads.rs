//! What each key's machine has written into its own sketchpad.
//!
//! The sketchpad is the machine's working memory: an append-only log of
//! `intent`, `pattern`, and free-form notes, grouped into sessions, with
//! rounds inside a session when a machine iterates. `docs/llm-guide.md`
//! documents the machine-facing endpoints (`/scratch/write`, `/scratch/
//! notes`, `/scratch/close`, `/scratch/history`); this plate is the
//! operator's read-only view of the same data.
//!
//! Read-only on purpose. A sketchpad belongs to whoever writes it, and
//! that is the machine. An operator who needs to say something to a
//! machine says it by approving or refusing a knock, not by typing into
//! the machine's own memory.
//!
//! Per-key grouping matches the design's "individual assigned sketch
//! pads to keys with their workspace ID": each key holds one pad, and
//! switching keys is switching pads.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn SketchpadsPlate() -> impl IntoView {
    let keys = RwSignal::new(Vec::<api::KeyInfo>::new());
    let selected = RwSignal::new(None::<String>);
    let notes = RwSignal::new(None::<api::ScratchpadNotes>);
    let history = RwSignal::new(Vec::<api::ScratchpadSession>::new());
    let checkpoints = RwSignal::new(Vec::<api::Checkpoint>::new());
    let error = RwSignal::new(None::<String>);

    // Load the key list once. Default selection: the first key, which
    // `list_keys` returns newest-first, so the most recently created key
    // opens by default. `get_untracked` so this effect does not re-run
    // when `selected` changes below.
    Effect::new(move |_| {
        spawn_local(async move {
            match api::list_keys().await {
                Ok(list) => {
                    if selected.get_untracked().is_none() {
                        selected.set(list.first().map(|k| k.token.clone()));
                    }
                    keys.set(list);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    });

    // Reload notes, history, and checkpoints whenever selection changes.
    // Everything is cleared first so a fast switch between keys never
    // shows the previous key's pad under the new key's chip.
    Effect::new(move |_| {
        let Some(token) = selected.get() else {
            notes.set(None);
            history.set(Vec::new());
            checkpoints.set(Vec::new());
            return;
        };
        notes.set(None);
        history.set(Vec::new());
        checkpoints.set(Vec::new());
        error.set(None);
        spawn_local(async move {
            match api::list_scratchpad_notes(&token).await {
                Ok(n) => notes.set(Some(n)),
                Err(e) => {
                    error.set(Some(e.to_string()));
                    return;
                }
            }
            if let Ok(h) = api::list_scratchpad_history(&token).await {
                history.set(h);
            }
            if let Ok(c) = api::list_checkpoints(&token).await {
                checkpoints.set(c);
            }
        });
    });

    let selected_key = move || {
        let tok = selected.get()?;
        keys.get().into_iter().find(|k| k.token == tok)
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 4px;font-size:22px;font-weight:700">"Sketchpads"</h2>
            <p class="dim" style="font-size:13px;margin:0 0 16px">
                "What each key's machine has written. One pad per key, read-only here."
            </p>

            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}

            {move || if keys.get().is_empty() {
                view! {
                    <p class="dim">"No keys yet. Create one to see its sketchpad."</p>
                }.into_any()
            } else {
                view! {
                    <div class="row" style="flex-wrap:wrap;margin-bottom:16px;gap:6px">
                        <For
                            each={move || keys.get()}
                            key=|k| k.token.clone()
                            children=move |k: api::KeyInfo| {
                                let token = k.token.clone();
                                let is_selected = {
                                    let token = token.clone();
                                    move || selected.get().as_deref() == Some(token.as_str())
                                };
                                let token_for_click = k.token.clone();
                                view! {
                                    <button
                                        class=move || if is_selected() { "chip on" } else { "chip" }
                                        on:click=move |_| selected.set(Some(token_for_click.clone()))
                                    >
                                        {k.label.clone()}
                                    </button>
                                }
                            }
                        />
                    </div>
                }.into_any()
            }}

            {move || selected_key().map(|k| {
                let workspace = if k.workspace_id.is_empty() {
                    "—".to_string()
                } else {
                    let w = &k.workspace_id;
                    if w.len() > 28 { format!("{}…", &w[..28]) } else { w.clone() }
                };
                let last_checkpoint = checkpoints.get().last().cloned();
                view! {
                    <div>
                        <p class="dim" style="font-size:12px;margin:0 0 12px">
                            "workspace " <span class="mono">{workspace}</span>
                            {last_checkpoint.map(|c| view! {
                                " · last checkpoint: " <span class="mono">{c.note.clone()}</span>
                                " (" {c.created_at.clone()} ")"
                            })}
                        </p>

                        {move || match notes.get() {
                            None => view! {
                                <p class="dim">"Loading…"</p>
                            }.into_any(),
                            Some(n) => view! {
                                <div class="card" style="margin-bottom:20px">
                                    <p class="dim" style="font-size:12px;margin:0 0 12px">
                                        "Current session · round " {n.round.to_string()}
                                        " · opened " {n.opened_at.clone()}
                                    </p>
                                    {if n.notes.is_empty() {
                                        view! {
                                            <p class="dim" style="font-size:13px;margin:0">
                                                "No notes in this session. The machine has not written here yet."
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div style="display:flex;flex-direction:column;gap:12px">
                                                <For
                                                    each={move || n.notes.clone()}
                                                    key=|note| note.id.clone()
                                                    children=move |note: api::ScratchpadNote| {
                                                        view! {
                                                            <div style="border-top:1px solid var(--line-soft);padding-top:10px">
                                                                <div class="row" style="align-items:center;gap:8px;margin-bottom:6px">
                                                                    <span class="kind-badge">{note.kind.clone()}</span>
                                                                    <span class="dim" style="font-size:12px">"round " {note.round.to_string()}</span>
                                                                    <span class="dim" style="font-size:12px">{note.at.clone()}</span>
                                                                </div>
                                                                <p style="margin:0;font-size:13px;line-height:1.55;white-space:pre-wrap;word-break:break-word">
                                                                    {note.body.clone()}
                                                                </p>
                                                                {note.knock_id.clone().map(|k| view! {
                                                                    <p class="dim" style="margin:6px 0 0;font-size:11px">
                                                                        "knock " <span class="mono">{k.chars().take(16).collect::<String>()}</span>
                                                                    </p>
                                                                })}
                                                            </div>
                                                        }
                                                    }
                                                />
                                            </div>
                                        }.into_any()
                                    }}
                                </div>
                            }.into_any(),
                        }}

                        <h3 style="margin:0 0 8px;font-size:16px;font-weight:700">"Past sessions"</h3>
                        {move || if history.get().is_empty() {
                            view! {
                                <p class="dim" style="font-size:13px">"No closed sessions yet."</p>
                            }.into_any()
                        } else {
                            view! {
                                <div style="display:flex;flex-direction:column;gap:8px">
                                    <For
                                        each={move || history.get()}
                                        key=|s| s.session_id.clone()
                                        children=move |s: api::ScratchpadSession| {
                                            let short_id = if s.session_id.len() > 28 {
                                                format!("{}…", &s.session_id[..28])
                                            } else {
                                                s.session_id.clone()
                                            };
                                            view! {
                                                <div class="card">
                                                    <p class="mono" style="margin:0 0 4px;font-size:12px">{short_id}</p>
                                                    <p class="dim" style="margin:0;font-size:12px">
                                                        "opened " {s.opened_at.clone()}
                                                        {s.closed_at.clone().map(|c| format!(" · closed {c}"))}
                                                    </p>
                                                    <p class="dim" style="margin:4px 0 0;font-size:12px">
                                                        {s.round_count.to_string()} " rounds · "
                                                        {s.intent_count.to_string()} " intents · "
                                                        {s.pattern_count.to_string()} " patterns"
                                                    </p>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            }.into_any()
                        }}
                    </div>
                }
            })}
        </div>
    }
}
