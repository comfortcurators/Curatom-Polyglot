# A knock, end to end

1. The operator creates a key and gives it to a machine.
2. The machine `POST`s `/inorganic/submit` with its key, reason, requested resources, permissions and duration. The Worker derives the owner from the key prefix and routes to that owner's `CuratomKernel` Durable Object.
3. `Kernel::create_knock` validates the key and request, rejects frozen resources, canonicalizes the request, computes its digest, gives the knock an 88-second expiry, records activity and persists it.
4. The dashboard polls `/organic/knocks`; expired knocks are lazily swept. The operator approves or refuses from the Knocks plate.
5. Approval is protected by Turnstile, then `Kernel::decide_knock` transitions the pending knock. `Kernel::issue_knock_grant` creates bounded authority tied to the approved digest and refuses duplicate issuance.
6. Before any remote handoff, `Kernel::consume_capability` spends each approved resource/permission pair. A second consumption fails.
7. For remote work the Worker creates HMAC-signed attestation claims and POSTs a signed job envelope to the orchestrator over its service binding. Elixir verifies the body HMAC before execution and verifies each attestation before adapter dispatch.
8. The orchestrator reports a signed outcome to `/internal/outcome`. The Worker verifies the HMAC and owner match, then `Kernel::record_outcome` records bounded outcome/activity state.
9. The operator sees the result in Activity.

Some approved resources are local: `company.whitepaper`, `company.inventory`, repository resources and compute resources are dispatched by `execute_locally` and never need the orchestrator. Valhalla requests provision an isolated sandbox and can freeze associated resources. Sketchpad writes use the same owner/key routing model but live in a separate `Scratchpad` Durable Object and are mirrored to D1/R2 for operator views.
