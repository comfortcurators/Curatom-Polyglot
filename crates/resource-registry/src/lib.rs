//! Known resources. Keep this small. Unknown targets fail closed.

pub const HOSTOS_INVENTORY: &str = "hostos.inventory";
pub const HOSTOS_METADATA: &str = "hostos.metadata";
pub const CLOUDFLARE_INVENTORY: &str = "cloudflare.inventory";
pub const COMPANY_WHITEPAPER: &str = "company.whitepaper";
pub const COMPANY_INVENTORY: &str = "company.inventory";
pub const REPOSITORY_INVENTORY: &str = "repository.inventory";

pub fn known_resource(r: &str) -> bool {
    matches!(
        r,
        HOSTOS_INVENTORY
            | HOSTOS_METADATA
            | CLOUDFLARE_INVENTORY
            | COMPANY_WHITEPAPER
            | COMPANY_INVENTORY
            | REPOSITORY_INVENTORY
    )
}

pub fn all() -> &'static [&'static str] {
    &[
        HOSTOS_INVENTORY,
        HOSTOS_METADATA,
        CLOUDFLARE_INVENTORY,
        COMPANY_WHITEPAPER,
        COMPANY_INVENTORY,
        REPOSITORY_INVENTORY,
    ]
}
