//! The generic adapter. Curatom ships no connector pre-wired -- not
//! hostos, not hostos-mcp, not anything. Add your own MCP (or any
//! HTTP-reachable) endpoint with whatever auth headers it needs; the
//! founder's own hostos/hostos-mcp go in this exact same form, with his
//! own credentials, like everyone else's.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::kit::Sheet;

#[component]
pub fn ComputePlate() -> impl IntoView {
    let connectors = RwSignal::new(Vec::<api::ConnectorInfo>::new());
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let creating = RwSignal::new(false);

    let refresh = move || {
        spawn_local(async move {
            match api::list_connectors().await {
                Ok(list) => {
                    connectors.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh());

    let delete = move |id: String| {
        busy.set(true);
        spawn_local(async move {
            if let Err(e) = api::delete_connector(&id).await {
                error.set(Some(e.to_string()));
            }
            busy.set(false);
            refresh();
        });
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 4px;font-size:22px;font-weight:700">"Compute"</h2>
            <p class="dim" style="font-size:13px;margin:0 0 16px">
                "Nothing is connected by default. Add your own endpoint and auth below."
            </p>
            <button class="btn" style="width:100%;margin-bottom:16px" on:click=move |_| creating.set(true)>
                "+ NEW CONNECTOR"
            </button>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (connectors.get().is_empty()).then(|| view! {
                <p class="dim">"No connectors yet."</p>
            })}
            <div style="display:flex;flex-direction:column;gap:10px">
                <For
                    each={move || connectors.get()}
                    key=|c| c.id.clone()
                    children=move |c: api::ConnectorInfo| {
                        let id_for_delete = c.id.clone();
                        view! {
                            <div class="card">
                                <strong>{c.name.clone()}</strong>
                                <p class="mono dim" style="margin:6px 0;font-size:12px;word-break:break-all">
                                    {c.endpoint.clone()}
                                </p>
                                {(!c.header_names.is_empty()).then(|| view! {
                                    <p class="dim" style="margin:0;font-size:12px">
                                        "auth: " {c.header_names.join(", ")}
                                    </p>
                                })}
                                <p class="dim" style="margin:2px 0 0;font-size:12px">"added " {c.created_at.clone()}</p>
                                <div class="row" style="margin-top:10px">
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

            {move || creating.get().then(|| view! {
                <CreateConnectorModal
                    on_close=Callback::new(move |_| creating.set(false))
                    on_created=Callback::new(move |_| { creating.set(false); refresh(); })
                />
            })}
        </div>
    }
}

#[derive(Clone)]
struct HeaderRow {
    id: u32,
    name: RwSignal<String>,
    value: RwSignal<String>,
}

#[component]
fn CreateConnectorModal(on_close: Callback<()>, on_created: Callback<()>) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let endpoint = RwSignal::new(String::new());
    let next_id = RwSignal::new(0u32);
    let rows = RwSignal::new(Vec::<HeaderRow>::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let add_row = move |_| {
        let id = next_id.get();
        next_id.set(id + 1);
        rows.update(|r| r.push(HeaderRow { id, name: RwSignal::new(String::new()), value: RwSignal::new(String::new()) }));
    };
    let remove_row = move |id: u32| rows.update(|r| r.retain(|row| row.id != id));

    let submit = move |_| {
        let n = name.get().trim().to_string();
        let ep = endpoint.get().trim().to_string();
        if n.is_empty() || ep.is_empty() {
            error.set(Some("Name and endpoint are both required.".into()));
            return;
        }
        busy.set(true);
        error.set(None);
        let header_values: Vec<(String, String)> = rows
            .get()
            .iter()
            .map(|r| (r.name.get(), r.value.get()))
            .filter(|(n, v)| !n.trim().is_empty() && !v.is_empty())
            .collect();
        spawn_local(async move {
            let headers: Vec<api::ConnectorHeaderIn> = header_values
                .iter()
                .map(|(n, v)| api::ConnectorHeaderIn { name: n, value: v })
                .collect();
            match api::create_connector(&n, &ep, &headers).await {
                Ok(_) => {
                    busy.set(false);
                    on_created.run(());
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
            <h3 style="margin:0 0 8px">"New connector"</h3>
            <p class="dim" style="font-size:13px;margin-bottom:12px">
                "Any MCP server, or any HTTP endpoint. Your own auth, your own credentials."
            </p>
            <input
                class="input"
                placeholder="Name (e.g. my hostos-mcp)"
                prop:value=move || name.get()
                on:input=move |ev| name.set(event_target_value(&ev))
            />
            <input
                class="input"
                style="margin-top:8px"
                placeholder="https://your-endpoint.example.com/mcp"
                prop:value=move || endpoint.get()
                on:input=move |ev| endpoint.set(event_target_value(&ev))
            />
            <p class="dim" style="font-size:12px;margin:12px 0 6px">"Auth headers (optional)"</p>
            <For
                each={move || rows.get()}
                key=|r| r.id
                children=move |row: HeaderRow| {
                    let id = row.id;
                    view! {
                        <div class="row" style="margin-bottom:6px">
                            <input
                                class="input"
                                placeholder="Header name"
                                prop:value=move || row.name.get()
                                on:input=move |ev| row.name.set(event_target_value(&ev))
                            />
                            <input
                                class="input"
                                type="password"
                                placeholder="Value"
                                prop:value=move || row.value.get()
                                on:input=move |ev| row.value.set(event_target_value(&ev))
                            />
                            <button class="btn-ghost" on:click=move |_| remove_row(id)>"×"</button>
                        </div>
                    }
                }
            />
            <button class="btn-ghost" style="width:100%;margin-top:4px" on:click=add_row>
                "+ ADD HEADER"
            </button>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            <div class="row" style="margin-top:12px">
                <button class="btn-ghost" disabled=move || busy.get() on:click=move |_| on_close.run(())>
                    "CANCEL"
                </button>
                <button class="btn" disabled=move || busy.get() on:click=submit>
                    {move || if busy.get() { "…" } else { "CREATE" }}
                </button>
            </div>
        </Sheet>
    }
}
