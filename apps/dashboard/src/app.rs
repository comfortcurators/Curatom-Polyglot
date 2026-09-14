use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::dashboard::Dashboard;
use crate::enrollment::Enrollment;

#[component]
pub fn App() -> impl IntoView {
    let me = RwSignal::new(None::<api::Me>);
    let boot_error = RwSignal::new(None::<String>);

    let refresh_me = move || {
        spawn_local(async move {
            match api::me().await {
                Ok(m) => {
                    me.set(Some(m));
                    boot_error.set(None);
                }
                Err(e) => boot_error.set(Some(e.to_string())),
            }
        });
    };

    Effect::new(move |_| refresh_me());

    view! {
        <div style="height:100vh">
            {move || {
                if let Some(err) = boot_error.get() {
                    view! {
                        <div style="height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;padding:24px">
                            <p class="err" style="font-weight:700">"Could not reach Curatom."</p>
                            <p class="dim">{err}</p>
                            <button class="btn" style="padding:10px 18px" on:click=move |_| refresh_me()>
                                "RETRY"
                            </button>
                        </div>
                    }.into_any()
                } else if let Some(m) = me.get() {
                    if m.enrolled {
                        view! { <Dashboard /> }.into_any()
                    } else {
                        view! { <Enrollment on_enrolled=Callback::new(move |_| refresh_me()) /> }.into_any()
                    }
                } else {
                    view! {
                        <div style="height:100%;display:flex;align-items:center;justify-content:center">"…"</div>
                    }.into_any()
                }
            }}
        </div>
    }
}
