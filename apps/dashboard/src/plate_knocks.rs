use gloo_timers::future::TimeoutFuture;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use crate::api;
use crate::kit::Sheet;
use crate::turnstile::TurnstileGate;

const SWIPE_THRESHOLD: f64 = 120.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardState {
    Idle,
    Dragging,
    Settling,
    LeavingRight,
    LeavingLeft,
}

#[component]
pub fn KnocksPlate() -> impl IntoView {
    let knocks = RwSignal::new(Vec::<api::Knock>::new());
    let toast = RwSignal::new(String::new());
    let confirming = RwSignal::new(None::<api::Knock>);
    let ts_token = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let refresh = move || {
        spawn_local(async move {
            match api::list_knocks().await {
                Ok(list) => knocks.set(list),
                Err(e) => toast.set(e.to_string()),
            }
        });
    };

    Effect::new(move |_| {
        refresh();
        spawn_local(async move {
            loop {
                TimeoutFuture::new(2_000).await;
                refresh();
            }
        });
    });

    let remove_local = move |id: &str| {
        knocks.update(|list| list.retain(|k| k.id != id));
    };

    let do_refuse = move |id: String| {
        remove_local(&id);
        busy.set(true);
        spawn_local(async move {
            match api::refuse_knock(&id).await {
                Ok(_) => toast.set("Refused.".into()),
                Err(e) => toast.set(e.to_string()),
            }
            busy.set(false);
            refresh();
        });
    };

    let request_approve = Callback::new(move |k: api::Knock| {
        ts_token.set(None);
        confirming.set(Some(k));
    });

    let do_approve = move |_| {
        let (Some(k), Some(token)) = (confirming.get(), ts_token.get()) else {
            return;
        };
        remove_local(&k.id);
        confirming.set(None);
        ts_token.set(None);
        busy.set(true);
        spawn_local(async move {
            match api::approve_knock(&k.id, &token).await {
                Ok(_) => toast.set("Approved.".into()),
                Err(e) => toast.set(e.to_string()),
            }
            busy.set(false);
            refresh();
        });
    };

    view! {
        <div class="plate-body" style="display:flex;flex-direction:column;padding:24px;height:100%">
            <h2 style="margin:0 0 4px;font-size:22px;font-weight:700">"Knocks"</h2>
            <p class="dim" style="font-size:13px;margin:0 0 20px">
                "Swipe right to approve, left to refuse. Or use the buttons."
            </p>
            {move || (!toast.get().is_empty()).then(|| view! {
                <p class="ok" style="font-size:13px;margin-bottom:8px">{toast.get()}</p>
            })}

            <div class="knock-stack">
                {move || (knocks.get().is_empty()).then(|| view! {
                    <p class="dim">"Nothing at the door."</p>
                })}
                <For
                    each={move || knocks.get().into_iter().take(3).enumerate().collect::<Vec<_>>()}
                    key={|(_, k): &(usize, api::Knock)| k.id.clone()}
                    children={move |(depth, k): (usize, api::Knock)| {
                        view! {
                            <SwipeCard
                                knock=k
                                depth=depth
                                is_top=depth == 0
                                busy=busy
                                on_approve=request_approve
                                on_refuse={Callback::new(do_refuse)}
                            />
                        }
                    }}
                />
            </div>

            {move || confirming.get().map(|k| {
                let permissions = k.permissions.clone();
                let resources = k.resources.clone();
                view! {
                    <Sheet on_close={Callback::new(move |_| { confirming.set(None); ts_token.set(None); })}>
                        <h3 style="margin:0 0 8px">{k.name.clone()} " is at the door"</h3>
                        <For
                            each={move || {
                                let paired: Vec<(String, String)> = permissions.clone().into_iter().zip(
                                    resources.clone().into_iter().chain(std::iter::repeat(String::new()))
                                ).collect();
                                paired
                            }}
                            key={|(p, r): &(String, String)| format!("{p}{r}")}
                            children={move |(p, r): (String, String)| view! {
                                <p style="color:#ccc;margin:2px 0">"· " {p} " " {r}</p>
                            }}
                        />
                        <p class="dim" style="font-size:13px;margin:12px 0">"Reason: " {k.reason.clone()}</p>
                        <p class="dim" style="font-size:13px;margin-bottom:12px">"Do you recognize this name?"</p>
                        {move || if ts_token.get().is_none() {
                            view! { <TurnstileGate on_token={Callback::new(move |t| ts_token.set(Some(t)))} /> }.into_any()
                        } else {
                            view! { <p class="ok" style="padding:12px">"Human verified."</p> }.into_any()
                        }}
                        <div class="row" style="margin-top:12px">
                            <button class="btn-ghost" on:click=move |_| { confirming.set(None); ts_token.set(None); }>
                                "CANCEL"
                            </button>
                            <button
                                class="btn"
                                disabled=move || ts_token.get().is_none() || busy.get()
                                on:click=do_approve
                            >
                                "CONFIRM"
                            </button>
                        </div>
                    </Sheet>
                }
            })}
        </div>
    }
}

#[component]
fn SwipeCard(
    knock: api::Knock,
    depth: usize,
    is_top: bool,
    busy: RwSignal<bool>,
    on_approve: Callback<api::Knock>,
    on_refuse: Callback<String>,
) -> impl IntoView {
    let card_ref: NodeRef<html::Div> = NodeRef::new();
    let drag_x = RwSignal::new(0.0_f64);
    let state = RwSignal::new(CardState::Idle);
    let start_x = std::rc::Rc::new(std::cell::Cell::new(0.0_f64));
    let dragging_pointer = std::rc::Rc::new(std::cell::Cell::new(false));

    let depth_scale = 1.0 - (depth.min(2) as f64) * 0.04;
    let depth_y = -(depth.min(2) as f64) * 10.0;

    let apply_transform = move |dx: f64, rotate_extra: bool| {
        let Some(el) = card_ref.get() else { return };
        let el: web_sys::HtmlElement = (*el).clone().unchecked_into();
        let rotate = if rotate_extra {
            (dx / 300.0 * 18.0).clamp(-18.0, 18.0)
        } else {
            0.0
        };
        let scale = 1.0 - (depth.min(2) as f64) * 0.04;
        let y = -(depth.min(2) as f64) * 10.0;
        let _ = el.style().set_property(
            "transform",
            &format!("translate({dx}px, {y}px) rotate({rotate}deg) scale({scale})"),
        );
    };

    Effect::new(move |_| {
        // Re-apply the resting transform whenever this card's stack depth
        // changes (a card ahead of it left) so it settles into its new slot.
        if state.get() == CardState::Idle {
            apply_transform(0.0, false);
        }
    });

    let on_pointer_down = {
        let dragging_pointer = dragging_pointer.clone();
        let start_x = start_x.clone();
        move |ev: PointerEvent| {
            if !is_top {
                return;
            }
            dragging_pointer.set(true);
            start_x.set(ev.client_x() as f64);
            state.set(CardState::Dragging);
            if let Some(target) = ev.target() {
                if let Ok(el) = target.dyn_into::<web_sys::Element>() {
                    let _ = el.set_pointer_capture(ev.pointer_id());
                }
            }
        }
    };

    let on_pointer_move = {
        let dragging_pointer = dragging_pointer.clone();
        let start_x = start_x.clone();
        move |ev: PointerEvent| {
            if !dragging_pointer.get() {
                return;
            }
            let dx = ev.client_x() as f64 - start_x.get();
            drag_x.set(dx);
            apply_transform(dx, true);
        }
    };

    let knock_for_up = knock.clone();
    let on_pointer_up = move |_: PointerEvent| {
        if !dragging_pointer.get() {
            return;
        }
        dragging_pointer.set(false);
        let dx = drag_x.get();
        if dx > SWIPE_THRESHOLD {
            state.set(CardState::LeavingRight);
            apply_transform(500.0, true);
            let k = knock_for_up.clone();
            spawn_local(async move {
                TimeoutFuture::new(220).await;
                on_approve.run(k);
            });
        } else if dx < -SWIPE_THRESHOLD {
            state.set(CardState::LeavingLeft);
            apply_transform(-500.0, true);
            let id = knock_for_up.id.clone();
            spawn_local(async move {
                TimeoutFuture::new(220).await;
                on_refuse.run(id);
            });
        } else {
            state.set(CardState::Settling);
            drag_x.set(0.0);
            apply_transform(0.0, true);
        }
    };

    let border_color = if knock.seconds_remaining < 20.0 {
        "#a44"
    } else {
        "var(--line)"
    };
    let secs = knock.seconds_remaining.max(0.0).floor() as i64;
    let secs_color = if knock.seconds_remaining < 20.0 {
        "var(--urgent)"
    } else {
        "var(--ok)"
    };

    let permissions = knock.permissions.clone();
    let resources = knock.resources.clone();
    let reason = knock.reason.clone();
    let name = knock.name.clone();
    let knock_for_approve_btn = knock.clone();
    let id_for_refuse_btn = knock.id.clone();

    let css_class = move || match state.get() {
        CardState::Dragging => "knock-card dragging",
        CardState::Settling => "knock-card settling",
        CardState::LeavingRight | CardState::LeavingLeft => "knock-card leaving",
        CardState::Idle => "knock-card settling",
    };

    let z_index = 10 + (3 - depth.min(3));

    view! {
        <div
            node_ref=card_ref
            class=css_class
            style=format!(
                "z-index:{z_index};transform:translate(0px, {depth_y}px) scale({depth_scale})"
            )
            on:pointerdown=on_pointer_down
            on:pointermove=on_pointer_move
            on:pointerup=on_pointer_up.clone()
            on:pointercancel=on_pointer_up.clone()
        >
            <div class="knock-card-inner" style=format!("border:1px solid {border_color}")>
                {move || is_top.then(|| {
                    let approve_opacity = (drag_x.get() / 140.0).clamp(0.0, 1.0);
                    let refuse_opacity = (-drag_x.get() / 140.0).clamp(0.0, 1.0);
                    view! {
                        <span class="badge-tag badge-approve" style=format!("opacity:{approve_opacity}")>"APPROVE"</span>
                        <span class="badge-tag badge-refuse" style=format!("opacity:{refuse_opacity}")>"REFUSE"</span>
                    }
                })}
                <div style="display:flex;justify-content:space-between;align-items:center">
                    <h3 style="margin:0;font-size:18px">{name}</h3>
                    <span style=format!("font-size:12px;font-weight:700;color:{secs_color}")>{secs} "s"</span>
                </div>
                <div style="margin:10px 0">
                    <For
                        each={move || {
                            let paired: Vec<(String, String)> = permissions.clone().into_iter().zip(
                                resources.clone().into_iter().chain(std::iter::repeat(String::new()))
                            ).collect();
                            paired
                        }}
                        key={|(p, r): &(String, String)| format!("{p}{r}")}
                        children={move |(p, r): (String, String)| view! {
                            <p style="color:#ccc;margin:2px 0;font-size:14px">"· " {p} " " {r}</p>
                        }}
                    />
                </div>
                <p class="dim" style="font-size:13px">"Reason: " {reason}</p>
                <div class="row" style="margin-top:16px">
                    <button
                        class="btn-ghost"
                        disabled=move || busy.get()
                        on:click=move |_| on_refuse.run(id_for_refuse_btn.clone())
                    >
                        "REFUSE"
                    </button>
                    <button
                        class="btn"
                        disabled=move || busy.get()
                        on:click=move |_| on_approve.run(knock_for_approve_btn.clone())
                    >
                        "APPROVE"
                    </button>
                </div>
            </div>
        </div>
    }
}
