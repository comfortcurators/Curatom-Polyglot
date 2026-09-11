//! Known resources. Keep this small. Unknown targets fail closed.

pub const HOSTOS_INVENTORY: &str = "hostos.inventory";
pub const HOSTOS_METADATA: &str = "hostos.metadata";
pub const CLOUDFLARE_INVENTORY: &str = "cloudflare.inventory";

pub fn known_resource(r: &str) -> bool {
    matches!(r, HOSTOS_INVENTORY | HOSTOS_METADATA | CLOUDFLARE_INVENTORY)
}

pub fn all() -> &'static [&'static str] {
    &[HOSTOS_INVENTORY, HOSTOS_METADATA, CLOUDFLARE_INVENTORY]
}
