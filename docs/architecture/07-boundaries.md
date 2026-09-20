# Boundaries

Curatom is not an identity provider for other systems, an OAuth authorization server, a general-purpose secrets manager, a policy-language engine, or a workflow engine. It is a capability gate with explicit operator approval and bounded execution evidence. The HostOS adapter shipped here is a mock; HostOS itself is external.

The rv0.4.0 release includes account/password/passkey flows; key lifecycle; bearer-key machine requests; knocks and the 88-second decision window; capability issue/consume/digest binding; HMAC attestation and handoff; mock orchestrated execution and authenticated outcome callbacks; whitepaper/inventory/repository/compute local surfaces; sketchpads; Valhalla sandbox lifecycle; checkpoint snapshot/restore; freezes; and configured connector machinery.

Two implementation defects found during final publication review were fixed in the release candidate: all `/scratch/*` machine methods now route by the presenting key's owner prefix, not only GETs; and Valhalla `/read` now extracts the Sandbox SDK result's `content` field instead of serializing the SDK wrapper object.

Deferred work includes signed machine-request enforcement, real HostOS execution, crash-persistent replay protection, action timeout/cancellation supervision, distributed rate limiting, notification/preferences semantics, production Access activation, and scaling decisions such as D1 tenancy/sharding. `DEFERRED.md` is authoritative for unfinished implementation work.

When writing publicly, say **deferred** when implementation is deliberately absent and **unresolved design question** when the product semantics have not been chosen. Do not turn either category into a roadmap promise. Cite the release/commit when making architectural claims.

A v1 is not defined by a marketing checklist. It would mean the public surfaces and machine guide agree with executable behavior, deferred items are either implemented or explicitly retained with user-facing rationale, and another operator can run the system without founder intervention.
