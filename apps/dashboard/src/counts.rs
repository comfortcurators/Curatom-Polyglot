use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::plates::PlateId;

#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub knocks: Option<usize>,
    pub keys: Option<usize>,
    pub valhalla: Option<usize>,
    pub billboard: Option<usize>,
    pub activity: Option<usize>,
}

impl Counts {
    pub fn get(self, id: PlateId) -> Option<usize> {
        match id {
            PlateId::Knocks => self.knocks,
            PlateId::Keys => self.keys,
            PlateId::Valhalla => self.valhalla,
            PlateId::Billboard => self.billboard,
            PlateId::Activity => self.activity,
        }
    }
}

pub fn use_counts() -> RwSignal<Counts> {
    let counts = RwSignal::new(Counts::default());

    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                let knocks = api::list_knocks().await;
                let keys = api::list_keys().await;
                let valhalla = api::valhalla_sessions().await;
                let billboard = api::billboard(None, 200).await;
                let activity = api::activity().await;
                counts.set(Counts {
                    knocks: knocks.ok().map(|v| v.len()),
                    keys: keys.ok().map(|v| v.len()),
                    valhalla: valhalla.ok().map(|v| v.iter().filter(|s| s.alive).count()),
                    billboard: billboard.ok().map(|v| v.len()),
                    activity: activity.ok().map(|v| v.len()),
                });
                TimeoutFuture::new(3_000).await;
            }
        });
    });

    counts
}
