# Curatom-Polyglot architecture

Expo handles the human, Rust/WASM handles the enforcer, Elixir handles the
orchestration. Each language does the thing it is actually better at.

```
┌─────────────────┐
│  EXPO (mobile)  │  owner mode only
│  organic human  │  never sees tokens, scopes, or infra
└────────┬────────┘
         │ HTTPS / JSON
         │ (Cloudflare Access → OrganicIdentity)
         ▼
┌─────────────────────────────────┐
│  CLOUDFLARE WORKER  (trust edge)│
│  ├─ Rust/WASM kernel            │  ← capability, digest, approvals
│  ├─ Durable Object storage      │  ← canonical state
│  ├─ R2                          │  ← artifacts
│  └─ HMAC-signed job handoff →   │
└────────┬────────────────────────┘
         │ HTTPS + HMAC-signed job
         │ (private network only)
         ▼
┌─────────────────────────────────┐
│  ELIXIR / OTP  (orchestrator)   │
│  ├─ JobQueue GenServer          │  ← intake, HMAC verify, replay guard
│  ├─ Task.Supervisor             │  ← one task per (resource, op)
│  ├─ ExecutionWorker             │  ← calls HostOS adapter
│  └─ HostOS adapter              │  ← real work, retries, timeouts
└────────┬────────────────────────┘
         │ HTTPS + HMAC
         ▼
       HostOS / MCP / substrate adapter
```

Why this shape. If Elixir is compromised, an attacker gets: HMAC-signed
attestations that are already consumed. Replaying them fails at HostOS's
replay guard. Fabricating new ones fails HMAC. Minting grants fails because
the Worker won't sign without an approved approval. The kernel stays small;
the orchestrator stays replaceable; the human never sees machinery.
