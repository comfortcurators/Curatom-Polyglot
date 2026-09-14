use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;

#[component]
pub fn Enrollment(on_enrolled: Callback<()>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    let begin = move |_| {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match api::claim().await {
                Ok(_) => on_enrolled.run(()),
                Err(e) => {
                    error.set(Some(e.to_string()));
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <div style="height:100%;display:flex;flex-direction:column;justify-content:center;padding:24px;max-width:480px;margin:0 auto">
            <h1 style="font-size:28px;font-weight:700;margin:0 0 16px">"You are the operator."</h1>
            <p class="dim" style="font-size:16px;line-height:1.6;margin:0 0 16px">
                "Curatom is where machines ask you for things. You say yes or no. Nothing else."
            </p>
            <p class="dim" style="font-size:16px;line-height:1.6;margin:0 0 24px">
                "You will get a token. Give it to any AI you trust. "
                "When they come to Curatom with it, you will see their knock and decide."
            </p>
            {move || error.get().map(|e| view! { <p class="err" style="margin-bottom:16px">{e}</p> })}
            <button class="btn" style="padding:16px 20px" disabled=move || busy.get() on:click=begin>
                {move || if busy.get() { "…" } else { "BEGIN" }}
            </button>
        </div>
    }
}
