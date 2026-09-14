use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn ActivityPlate() -> impl IntoView {
    let events = RwSignal::new(Vec::<api::ActivityEvent>::new());

    let refresh = move || {
        spawn_local(async move {
            if let Ok(list) = api::activity().await {
                events.set(list);
            }
        });
    };

    Effect::new(move |_| {
        refresh();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(4_000).await;
                refresh();
            }
        });
    });

    view! {
        <div class="plate-body">
            <h2 style="margin:0 0 16px;font-size:22px;font-weight:700">"Activity"</h2>
            {move || (events.get().is_empty()).then(|| view! { <p class="dim">"Nothing yet."</p> })}
            <div style="display:flex;flex-direction:column;gap:4px">
                <For
                    each={move || events.get().into_iter().rev().enumerate().collect::<Vec<_>>()}
                    key=|(i, e)| (*i, e.at.clone())
                    children=move |(_, e): (usize, api::ActivityEvent)| {
                        view! {
                            <div style="display:flex;justify-content:space-between;border-bottom:1px solid var(--line-soft);padding:8px 0;font-size:13px">
                                <span>{e.kind}</span>
                                <span class="dim">{e.at}</span>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}
