//! Repository snapshots for AI-friendly reading. What this actually is,
//! stated plainly rather than implied: "SYNC NOW" walks a repo's default
//! branch via the GitHub API, pulls the raw text of any dependency
//! manifest it recognizes (never parsed -- an LLM reading the manifest
//! is more reliable than this app guessing at it), and writes one JSON
//! snapshot. There is no cron and no webhook here yet -- it syncs when
//! you tap the button, not continuously. Say so instead of implying
//! otherwise.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::kit::Sheet;

#[component]
pub fn DataPlate() -> impl IntoView {
    let repos = RwSignal::new(Vec::<api::RepositoryInfo>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let syncing = RwSignal::new(None::<String>);
    let adding = RwSignal::new(false);

    let refresh = move || {
        spawn_local(async move {
            match api::list_repositories().await {
                Ok(list) => {
                    repos.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh());

    let sync = move |id: String| {
        syncing.set(Some(id.clone()));
        spawn_local(async move {
            if let Err(e) = api::sync_repository(&id).await {
                error.set(Some(e.to_string()));
            }
            syncing.set(None);
            refresh();
        });
    };

    let delete = move |id: String| {
        busy.set(true);
        spawn_local(async move {
            if let Err(e) = api::delete_repository(&id).await {
                error.set(Some(e.to_string()));
            }
            busy.set(false);
            refresh();
        });
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 4px;font-size:22px;font-weight:700">"Data"</h2>
            <p class="dim" style="font-size:13px;margin:0 0 16px">
                "Repository snapshots, AI-friendly. Sync is manual -- tap SYNC to pull the latest, nothing runs on its own yet."
            </p>
            <button class="btn" style="width:100%;margin-bottom:16px" on:click=move |_| adding.set(true)>
                "+ ADD REPOSITORY"
            </button>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (repos.get().is_empty()).then(|| view! {
                <p class="dim">"No repositories yet."</p>
            })}
            <div style="display:flex;flex-direction:column;gap:10px">
                <For
                    each={move || repos.get()}
                    key=|r| r.id.clone()
                    children=move |r: api::RepositoryInfo| {
                        let id_for_sync = r.id.clone();
                        let id_for_delete = r.id.clone();
                        let id_for_busy = r.id.clone();
                        view! {
                            <div class="card">
                                <div style="display:flex;justify-content:space-between;align-items:center">
                                    <strong>{r.name.clone()}</strong>
                                    {r.has_token.then(|| view! {
                                        <span class="dim" style="font-size:11px">"private"</span>
                                    })}
                                </div>
                                <p class="dim" style="margin:6px 0 0;font-size:12px">
                                    {match (r.last_synced_at.clone(), r.last_sync_file_count) {
                                        (Some(at), Some(n)) => format!("{n} files, synced {at}"),
                                        _ => "never synced".to_string(),
                                    }}
                                </p>
                                <p class="dim" style="margin:2px 0 0;font-size:12px">"added " {r.created_at.clone()}</p>
                                <div class="row" style="margin-top:10px">
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get() || syncing.get().is_some()
                                        on:click=move |_| sync(id_for_sync.clone())
                                    >
                                        {move || if syncing.get().as_deref() == Some(id_for_busy.as_str()) { "…" } else { "SYNC NOW" }}
                                    </button>
                                    <button
                                        class="btn-ghost"
                                        disabled=move || busy.get()
                                        on:click=move |_| delete(id_for_delete.clone())
                                    >
                                        "DELETE"
                                    </button>
                                </div>
                            </div>
                        }
                    }
                />
            </div>

            {move || adding.get().then(|| view! {
                <AddRepositoryModal
                    on_close=Callback::new(move |_| adding.set(false))
                    on_added=Callback::new(move |_| { adding.set(false); refresh(); })
                />
            })}
        </div>
    }
}

#[component]
fn AddRepositoryModal(on_close: Callback<()>, on_added: Callback<()>) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let token = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let submit = move |_| {
        let n = name.get().trim().to_string();
        if n.is_empty() {
            error.set(Some("Enter owner/repo, e.g. comfortcurators/curator.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        let tok = token.get();
        let tok_opt = (!tok.trim().is_empty()).then(|| tok.clone());
        spawn_local(async move {
            match api::create_repository(&n, tok_opt.as_deref()).await {
                Ok(_) => {
                    busy.set(false);
                    on_added.run(());
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
            <h3 style="margin:0 0 8px">"Add repository"</h3>
            <p class="dim" style="font-size:13px;margin-bottom:12px">
                "Public repos need nothing else. A private repo needs its own token, "
                "used only for this repository's own calls."
            </p>
            <input
                class="input"
                placeholder="owner/repo"
                prop:value=move || name.get()
                on:input=move |ev| name.set(event_target_value(&ev))
            />
            <input
                class="input"
                type="password"
                style="margin-top:8px"
                placeholder="GitHub token (optional, private repos only)"
                prop:value=move || token.get()
                on:input=move |ev| token.set(event_target_value(&ev))
            />
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            <div class="row" style="margin-top:12px">
                <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| on_close.run(())>
                    "CANCEL"
                </button>
                <button class="btn" disabled=move || busy.get() on:click=submit>
                    {move || if busy.get() { "…" } else { "ADD" }}
                </button>
            </div>
        </Sheet>
    }
}
