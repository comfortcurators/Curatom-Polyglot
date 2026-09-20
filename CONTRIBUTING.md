# Contributing

Curatom Polyglot welcomes focused bug fixes, tests, documentation corrections and narrowly scoped features that preserve its trust boundaries.

Before submitting a change, run the checks relevant to the component you touched. At minimum, Rust kernel changes should pass `cargo test --workspace`; Elixir changes should pass `mix test`; Worker/dashboard changes should also be checked for their WASM targets. Cross-boundary protocol changes should update `docs/contracts.md` and include a behavioral test rather than relying only on diff review.

Security-sensitive changes should preserve three invariants: owner state is scoped to the correct owner, secrets do not cross presentation boundaries, and remote execution receives spent/signed authority rather than a reusable owner credential.

Please keep commits explicit about what was actually verified. If a test used a stand-in, mock, local emulator or unconfigured production path, say so. `DEFERRED.md` exists so unfinished work is visible rather than implied complete.

By contributing, you agree that your contribution is licensed under the repository's `AGPL-3.0-only` license.
