# Third-party notices

Curatom Polyglot's company-authored code is licensed under `AGPL-3.0-only`. Third-party software keeps its own upstream license; inclusion in this source distribution does not relicense that software under the Curatom license.

## Vendored Elixir dependencies

The orchestrator intentionally carries a resolved dependency tree so its container can be built without fetching Hex packages at build time. Versions come from `orchestrator/mix.lock`; the upstream license text is preserved inside each dependency directory.

| Package | Version | License in this source tree |
|---|---:|---|
| cowboy | 2.19.0 | ISC |
| cowboy_telemetry | 0.4.0 | Apache-2.0 |
| cowlib | 2.20.0 | ISC |
| finch | 0.23.0 | MIT |
| hpax | 1.0.4 | Apache-2.0 |
| jason | 1.4.5 | Apache-2.0 |
| mime | 2.0.7 | Apache-2.0 |
| mint | 1.10.0 | Apache-2.0 |
| nimble_options | 1.1.1 | Apache-2.0 |
| nimble_pool | 1.1.0 | Apache-2.0 |
| plug | 1.20.3 | Apache-2.0 |
| plug_cowboy | 2.9.0 | Apache-2.0 |
| plug_crypto | 2.2.0 | Apache-2.0 |
| ranch | 2.3.0 | ISC |
| telemetry | 1.4.2 | Apache-2.0 |

The downloaded `nimble_pool` directory did not originally contain a standalone license file even though its package metadata identifies Apache-2.0. For this source release, the canonical Apache-2.0 text is included at `orchestrator/deps/nimble_pool/LICENSE` alongside the package.

## Vendored build tools

`orchestrator/vendor/hex.ez` is Hex 2.2.1 and `orchestrator/vendor/rebar3` is Rebar3 3.24.0. These upstream build tools are distributed under Apache-2.0. A copy of the Apache-2.0 license is included at `orchestrator/vendor/LICENSES/Apache-2.0.txt`. Rebar3 is a self-contained upstream escript and may itself contain bundled upstream components; their notices remain part of that upstream artifact.

Release-candidate SHA-256 values:

- `hex.ez`: `268c07230109cfcb33c9291d1e0b82dbc2c9dc1fa8dd72c6ae8e4dde842cefb3`
- `rebar3`: `d2d31cfb98904b8e4917300a75f870de12cb5167cd6214d1043e973a56668a54`

## Package-manager-resolved dependencies

Rust and JavaScript dependency source is not vendored into this archive; lock/manifests identify the resolved dependency graph and package managers obtain those components under their upstream licenses. Binary redistributors should perform the license/notice review appropriate to the exact artifacts they ship.

This file is a source-distribution inventory, not legal advice and not a claim that every possible downstream binary combination has been audited.
