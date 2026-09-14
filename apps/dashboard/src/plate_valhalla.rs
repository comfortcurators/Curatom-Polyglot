use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn ValhallaPlate() -> impl IntoView {
    let sessions = RwSignal::new(Vec::<api::ValhallaSession>::new());
    let error = RwSignal::new(None::<String>);

    let refresh = move || {
        spawn_local(async move {
            match api::valhalla_sessions().await {
                Ok(list) => {
                    sessions.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh());

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 16px;font-size:22px;font-weight:700">"Valhalla"</h2>
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (sessions.get().is_empty()).then(|| view! {
                <p class="dim">"No Valhalla sessions."</p>
            })}
            <div style="display:flex;flex-direction:column;gap:10px">
                <For
                    each={move || sessions.get()}
                    key=|s| s.sandbox_id.clone()
                    children=move |v: api::ValhallaSession| {
                        let status_color = if v.alive { "var(--ok)" } else { "var(--urgent)" };
                        let status_text = if v.alive { "live" } else { "closed" };
                        view! {
                            <div class="card">
                                <div style="display:flex;justify-content:space-between;align-items:center">
                                    <strong>{v.label.clone()}</strong>
                                    <span style=format!("font-size:12px;font-weight:700;color:{status_color}")>
                                        {status_text}
                                    </span>
                                </div>
                                <p class="mono" style="margin:6px 0">{v.sandbox_id.clone()}</p>
                                <p class="dim" style="margin:0;font-size:12px">"opened " {v.opened_at.clone()}</p>
                                {v.closed_at.clone().map(|c| view! {
                                    <p class="dim" style="margin:0;font-size:12px">"closed " {c}</p>
                                })}
                                <p class="dim" style="margin:0;font-size:12px">{v.entry_count} " actions inside"</p>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}
