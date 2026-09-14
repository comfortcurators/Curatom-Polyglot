use gloo_timers::future::TimeoutFuture;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use crate::api;
use crate::counts::use_counts;
use crate::plate_activity::ActivityPlate;
use crate::plate_billboard::BillboardPlate;
use crate::plate_keys::KeysPlate;
use crate::plate_knocks::KnocksPlate;
use crate::plate_valhalla::ValhallaPlate;
use crate::plates::PlateId;
use crate::view_transition::with_view_transition;

#[component]
pub fn Dashboard() -> impl IntoView {
    let zoomed = RwSignal::new(None::<PlateId>);
    let frozen = RwSignal::new(Vec::<api::Frozen>::new());
    let counts = use_counts();

    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                if let Ok(list) = api::frozen_list().await {
                    frozen.set(list);
                }
                TimeoutFuture::new(5_000).await;
            }
        });
    });

    let open = move |id: PlateId| with_view_transition(move || zoomed.set(Some(id)));
    let close = move || with_view_transition(move || zoomed.set(None));

    view! {
        <div style="height:100%;display:flex;flex-direction:column">
            {move || (!frozen.get().is_empty()).then(|| {
                let scopes = frozen.get().iter().map(|f| f.scope.clone()).collect::<Vec<_>>().join(", ");
                let n = frozen.get().len();
                view! {
                    <div class="frozen-banner">
                        <p style="color:#fff;font-weight:700;font-size:13px;margin:0">
                            {n} " resource" {if n == 1 { "" } else { "s" }} " frozen by Valhalla"
                        </p>
                        <p style="color:#fee;font-size:11px;margin:2px 0 0">{scopes}</p>
                    </div>
                }
            })}

            <div style="flex:1;position:relative;overflow:hidden">
                {move || match zoomed.get() {
                    None => view! { <Overview counts=counts on_open=Callback::new(open) /> }.into_any(),
                    Some(id) => view! { <PlateFullscreen plate=id on_close=Callback::new(move |_| close()) /> }.into_any(),
                }}
            </div>
        </div>
    }
}

#[component]
fn Overview(counts: RwSignal<crate::counts::Counts>, on_open: Callback<PlateId>) -> impl IntoView {
    view! {
        <div style="position:absolute;inset:0;overflow-y:auto;padding:24px">
            <div style="margin-bottom:20px">
                <p style="color:var(--gold);letter-spacing:2px;font-size:11px;font-weight:700;margin:0">
                    "CURATOM"
                </p>
                <h1 style="margin:4px 0 0;font-size:26px;font-weight:700">"The whole door."</h1>
            </div>
            <div class="overview-grid">
                <For
                    each=|| PlateId::ALL
                    key=|id| *id as u8
                    children=move |id: PlateId| {
                        let count = move || counts.get().get(id).map(|n| n.to_string()).unwrap_or_else(|| "–".into());
                        view! {
                            <button
                                class=format!("plate-tile {}", id.view_transition_class())
                                on:click=move |_| on_open.run(id)
                            >
                                <span style="font-size:16px;font-weight:700">{id.title()}</span>
                                <span>
                                    <span class="plate-count">{count}</span>
                                    <p class="dim" style="font-size:12px;margin:6px 0 0">{id.blurb()}</p>
                                </span>
                            </button>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[component]
fn PlateFullscreen(plate: PlateId, on_close: Callback<()>) -> impl IntoView {
    let panel_ref: NodeRef<html::Div> = NodeRef::new();
    let drag_y = std::rc::Rc::new(std::cell::Cell::new(0.0_f64));
    let start_y = std::rc::Rc::new(std::cell::Cell::new(0.0_f64));
    let dragging = std::rc::Rc::new(std::cell::Cell::new(false));

    let apply_y = move |dy: f64| {
        if let Some(el) = panel_ref.get() {
            let el: web_sys::HtmlElement = (*el).clone().unchecked_into();
            let opacity = (1.0 - (dy / 400.0)).clamp(0.4, 1.0);
            let _ = el
                .style()
                .set_property("transform", &format!("translateY({}px)", dy.max(0.0)));
            let _ = el.style().set_property("opacity", &opacity.to_string());
        }
    };

    let on_pointer_down = {
        let start_y = start_y.clone();
        let dragging = dragging.clone();
        move |ev: PointerEvent| {
            dragging.set(true);
            start_y.set(ev.client_y() as f64);
            if let Some(target) = ev.target() {
                if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                    let _ = el.set_pointer_capture(ev.pointer_id());
                }
            }
        }
    };

    let apply_y_move = apply_y.clone();
    let on_pointer_move = {
        let start_y = start_y.clone();
        let dragging = dragging.clone();
        let drag_y = drag_y.clone();
        move |ev: PointerEvent| {
            if !dragging.get() {
                return;
            }
            let dy = (ev.client_y() as f64 - start_y.get()).max(0.0);
            drag_y.set(dy);
            apply_y_move(dy);
        }
    };

    let on_pointer_up = {
        let dragging = dragging.clone();
        let drag_y = drag_y.clone();
        move |_: PointerEvent| {
            if !dragging.get() {
                return;
            }
            dragging.set(false);
            let dy = drag_y.get();
            if dy > 100.0 {
                on_close.run(());
            } else {
                drag_y.set(0.0);
                if let Some(el) = panel_ref.get() {
                    let el: web_sys::HtmlElement = (*el).clone().unchecked_into();
                    let _ = el.class_list().add_1("plate-panel-drag");
                    let _ = el.style().set_property("transform", "translateY(0px)");
                    let _ = el.style().set_property("opacity", "1");
                }
            }
        }
    };

    view! {
        <div
            node_ref=panel_ref
            class=format!("plate-panel {}", plate.view_transition_class())
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up.clone()
            on:pointercancel=on_pointer_up.clone()
        >
            <div class="plate-header">
                <button class="back-btn" on:click=move |_| on_close.run(()) aria-label="Back to the whole map">
                    "←"
                </button>
                <span style="font-size:13px;color:var(--ink-faint)">{plate.title()}</span>
            </div>
            {match plate {
                PlateId::Knocks => view! { <KnocksPlate /> }.into_any(),
                PlateId::Keys => view! { <KeysPlate /> }.into_any(),
                PlateId::Valhalla => view! { <ValhallaPlate /> }.into_any(),
                PlateId::Billboard => view! { <BillboardPlate /> }.into_any(),
                PlateId::Activity => view! { <ActivityPlate /> }.into_any(),
            }}
        </div>
    }
}
