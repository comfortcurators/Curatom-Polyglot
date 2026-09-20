# Vocabulary

Use these terms consistently in public writing.

**Operator** — the human owner of a Curatom account. **Machine** — software holding a Curatom key; when cryptographic identity specifically matters, say **machine identity**. **Key** — the bearer credential used by a machine to identify its owner/key context and request authority. **Knock** — a request for a resource, permission, reason and duration; pending knocks expire after 88 seconds. **Capability** — bounded spendable authority, consumed once per resource/permission pair. A live capability does not leave the kernel. **Grant** — the kernel record that a capability was issued. **Approval** — an operator decision bound to the request digest. **Attestation** — the signed claims envelope used for remote handoff. **Outcome** — the recorded result of execution.

**Contract** means a trust boundary. Contract 1 is browser ↔ Worker, Contract 2 Worker ↔ orchestrator, Contract 3 orchestrator ↔ execution target. **Valhalla** is the sandbox subsystem. **Checkpoint** is a named point in key history and may carry a workspace snapshot. **Freeze** locks a resource while a session owns it. **Connector** is an operator-supplied execution endpoint. **Sketchpad** is machine working memory; **Billboard** and **Activity** are operator-visible records.

Infrastructure terms retain their Cloudflare meanings: **Worker**, **Durable Object**, **D1**, **R2**, and **Container**.

Avoid using *user* when *operator* or *machine* is meant. Avoid *agent* when *machine* is precise. Do not call a Curatom key an OAuth-style authorization token: possessing a key still requires a knock and operator approval for authority.
