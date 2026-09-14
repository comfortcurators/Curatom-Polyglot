//! Known resources. Keep this small. Unknown targets fail closed.

pub const HOSTOS_INVENTORY: &str = "hostos.inventory";
pub const HOSTOS_METADATA: &str = "hostos.metadata";
pub const CLOUDFLARE_INVENTORY: &str = "cloudflare.inventory";
pub const COMPANY_WHITEPAPER: &str = "company.whitepaper";
pub const COMPANY_INVENTORY: &str = "company.inventory";
pub const REPOSITORY_INVENTORY: &str = "repository.inventory";
/// A knock naming this resource is not a resource read: on approval it mints
/// a time-boxed capability in the `capabilities` D1 table (mcp_gateway.rs)
/// instead of a one-shot attestation, and the "permission" granted is really
/// a tool *scope* (oauth | full) rather than read/write. Kept in the same
/// registry anyway -- the door is still "does the operator approve this
/// knock" -- so a caller cannot request it by any spelling that skips that
/// approval.
pub const HOSTOS_MCP: &str = "hostos_mcp";

pub fn known_resource(r: &str) -> bool {
    matches!(
        r,
        HOSTOS_INVENTORY
            | HOSTOS_METADATA
            | CLOUDFLARE_INVENTORY
            | COMPANY_WHITEPAPER
            | COMPANY_INVENTORY
            | REPOSITORY_INVENTORY
            | HOSTOS_MCP
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
        HOSTOS_MCP,
    ]
}
