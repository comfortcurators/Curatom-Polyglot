# Shape

```text
Browser / Leptos SPA
        │ HTTPS JSON — Contract 1
        ▼
Cloudflare Worker / Rust-WASM kernel
  ├─ CuratomKernel + Scratchpad Durable Objects
  ├─ D1 CURATOM_LEDGER
  ├─ R2 CURATOM_ARTIFACTS
  ├─ HTTPS → Valhalla Worker / Sandbox container
  └─ service binding + signed handoff — Contract 2
                         ▼
                    Elixir/OTP orchestrator
                         │ verified action fields — Contract 3
                         ▼
                    execution adapter / target
                         │
                         └─ HMAC-authenticated outcome → Worker

Valhalla Worker / Sandbox container
        └─ service binding → kernel internal authorization/freeze/restore routes
```

The kernel owns capability law: TTLs, digest binding, single-use consumption, owner scoping and presentation views that do not expose live capability secrets. Cloudflare substrate supplies persistence and platform bindings. The orchestrator verifies the signed handoff, runs an action through an adapter and reports an authenticated outcome. The HostOS adapter shipped in rv0.4.0 is a mock.

Valhalla is a separate execution subsystem. The kernel calls its configured HTTPS endpoint for provisioning/snapshot work; Valhalla calls kernel-internal routes through a Cloudflare service binding for authorization, freeze/release and checkpoint resolution. Its workspace state is disposable; checkpoint manifests and content-addressed blobs live in R2.

Rust is used for the kernel and shared protocol because enforcement benefits from a small typed core and the same types can compile into Worker and browser WASM. Elixir/OTP is used where concurrent orchestration is the dominant problem. Leptos keeps the dashboard in the same Rust type ecosystem as the protocol.

The shape keeps the enforcement kernel small and remote execution replaceable. A remote executor does not receive a reusable Curatom capability: consumption happens before handoff.
