mod api;
mod app;
mod counts;
mod dashboard;
mod enrollment;
mod kit;
mod login;
mod passkey_browser;
mod plate_account;
mod plate_activity;
mod plate_billboard;
mod plate_compute;
mod plate_data;
mod plate_keys;
mod plate_knocks;
mod plate_valhalla;
mod plates;
mod reset_password;
mod turnstile;
mod view_transition;

use app::App;

fn main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Warn);
    leptos::mount::mount_to_body(App);
}
