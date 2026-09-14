use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::kit::Sheet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Intent,
    Pattern,
}

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Filter::All => "ALL",
            Filter::Intent => "INTENT",
            Filter::Pattern => "PATTERN",
        }
    }
    fn kind(self) -> Option<&'static str> {
        match self {
            Filter::All => None,
            Filter::Intent => Some("intent"),
            Filter::Pattern => Some("pattern"),
        }
    }
}

#[component]
pub fn BillboardPlate() -> impl IntoView {
    let entries = RwSignal::new(Vec::<api::BillboardEntry>::new());
    let filter = RwSignal::new(Filter::All);
    let error = RwSignal::new(None::<String>);
    let detail = RwSignal::new(None::<(String, String)>);

    Effect::new(move |_| {
        let f = filter.get();
        spawn_local(async move {
            match api::billboard(f.kind(), 200).await {
                Ok(list) => {
                    entries.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    });

    let open_body = move |reference: String| {
        spawn_local(async move {
            match api::billboard_blob(&reference).await {
                Ok(body) => detail.set(Some((reference, body))),
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 16px;font-size:22px;font-weight:700">"Billboard"</h2>
            <div class="row" style="margin-bottom:16px">
                {[Filter::All, Filter::Intent, Filter::Pattern].into_iter().map(|f| {
                    view! {
                        <button
                            class=move || if filter.get() == f { "chip on" } else { "chip" }
                            on:click=move |_| filter.set(f)
                        >
                            {f.label()}
                        </button>
                    }
                }).collect_view()}
            </div>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (entries.get().is_empty()).then(|| view! { <p class="dim">"No entries yet."</p> })}
            <div style="display:flex;flex-direction:column;gap:10px">
                <For
                    each={move || entries.get()}
                    key=|e| e.id.clone()
                    children=move |e: api::BillboardEntry| {
                        let reference = e.body_ref.clone();
                        let label = if e.key_label.is_empty() {
                            e.key_hash.chars().take(10).collect::<String>()
                        } else {
                            e.key_label.clone()
                        };
                        view! {
                            <button
                                class="card"
                                style="text-align:left;width:100%;cursor:pointer;color:inherit;font:inherit;border-style:solid"
                                on:click=move |_| open_body(reference.clone())
                            >
                                <div style="display:flex;justify-content:space-between">
                                    <strong style="font-size:14px">{e.kind.to_uppercase()} " · " {label}</strong>
                                    <span class="dim" style="font-size:12px">"r" {e.round}</span>
                                </div>
                                <p class="dim" style="margin:4px 0 0;font-size:12px">{e.created_at.clone()}</p>
                                {e.knock_id.clone().map(|k| view! {
                                    <p class="dim" style="margin:0;font-size:12px">
                                        "knock " {k.chars().take(14).collect::<String>()} "…"
                                    </p>
                                })}
                            </button>
                        }
                    }
                />
            </div>

            {move || detail.get().map(|(reference, body)| {
                view! {
                    <Sheet on_close=Callback::new(move |_| detail.set(None))>
                        <h3 style="margin:0 0 4px">"Body"</h3>
                        <p class="dim" style="font-size:12px;margin-bottom:12px">{reference}</p>
                        <pre class="code-box">{body}</pre>
                        <button class="btn" style="width:100%;margin-top:12px" on:click=move |_| detail.set(None)>
                            "CLOSE"
                        </button>
                    </Sheet>
                }
            })}
        </div>
    }
}
