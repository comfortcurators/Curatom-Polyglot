use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn ActivityPlate() -> impl IntoView {
    let events = RwSignal::new(Vec::<api::ActivityEvent>::new());
    // A fetch that failed must never look identical to a fetch that
    // genuinely found nothing -- that exact silent swallow is what let
    // this tile show "Nothing yet." for real activity for as long as it
    // did (a field-shape mismatch, fixed 2026-09-18; see ActivityView's
    // own doc comment). `error` is `None` only once a fetch has actually
    // succeeded, empty or not.
    let error = RwSignal::new(None::<String>);

    let refresh = move || {
        spawn_local(async move {
            match api::activity().await {
                Ok(list) => {
                    events.set(list);
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
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
            {move || error.get().map(|e| view! { <p class="err">{e}</p> })}
            {move || (error.get().is_none() && events.get().is_empty()).then(|| view! { <p class="dim">"Nothing yet."</p> })}
            <div style="display:flex;flex-direction:column;gap:4px">
                <For
                    each={move || events.get().into_iter().rev().enumerate().collect::<Vec<_>>()}
                    key=|(i, e)| (*i, e.at.clone())
                    children=move |(_, e): (usize, api::ActivityEvent)| {
                        view! {
                            <div style="display:flex;flex-direction:column;gap:2px;border-bottom:1px solid var(--line-soft);padding:8px 0;font-size:13px">
                                <div style="display:flex;justify-content:space-between">
                                    <span>{e.kind}</span>
                                    <span class="dim">{e.at}</span>
                                </div>
                                <span class="dim" style="font-size:12px">{e.summary}</span>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}
