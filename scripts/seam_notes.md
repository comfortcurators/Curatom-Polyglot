# Seam test (next commit)

A script that boots the Worker, sends a real job with a real attestation to
the Elixir orchestrator, and asserts the outcome lands back in the kernel
ledger — across a restart of the orchestrator.

That is the one test that will prove this three-language split actually
works. Not in this commit. The pieces it will glue:

1. `cargo test -p curatom-key-kernel` — consume-before-handoff, TTL, single-use
2. `cargo test -p curatom-attestation` — envelope roundtrip
3. `mix test` in `orchestrator/` — HMAC, replay, mock HostOS

Do not fake a green end-to-end by looping the kernel in-process and calling
it a seam.
