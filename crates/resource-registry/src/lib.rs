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

/// Resources an operator may put on a standing grant.
///
/// Deliberately excludes `valhalla.*` and `hostos.*` — those stay on the
/// 88-second knock path. Standing is for company context, Cloudflare
/// inventory, and repository reads that an AI needs repeatedly without
/// tapping the operator every time.
pub fn standing_eligible(r: &str) -> bool {
    r == CLOUDFLARE_INVENTORY
        || r == COMPANY_WHITEPAPER
        || r == COMPANY_INVENTORY
        || r == REPOSITORY_INVENTORY
        || (r.starts_with("repository.") && r != REPOSITORY_INVENTORY)
}

pub fn all_standing_eligible() -> &'static [&'static str] {
    &[
        CLOUDFLARE_INVENTORY,
        COMPANY_WHITEPAPER,
        COMPANY_INVENTORY,
        REPOSITORY_INVENTORY,
    ]
}
