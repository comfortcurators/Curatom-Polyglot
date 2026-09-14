use leptos::prelude::*;

#[component]
pub fn Sheet(on_close: Callback<()>, children: Children) -> impl IntoView {
    view! {
        <div class="sheet-backdrop" on:click=move |_| on_close.run(())>
            <div class="sheet-card" on:click=|ev| ev.stop_propagation()>
                {children()}
            </div>
        </div>
    }
}
