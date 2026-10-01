# Company workspace — what this PR delivers vs what needs deploy

Vision: (1) company info stored & available, (2) every AI owns a Valhalla
computer, (3) standing CF + GitHub access without constant approve taps,
(4) everything logged.

## Works in code today (after merge + deploy)

| Slice | Surface | Notes |
|-------|---------|-------|
| **A Standing grants** | `POST/GET /organic/standing`, `POST .../revoke` | Operator-issued only. Eligible: `cloudflare.inventory`, `company.whitepaper`, `company.inventory`, `repository.*`. **Not** `valhalla.*` / `hostos.*` — those still need the 88s knock. |
| **Standing skip-knock** | `POST /inorganic/submit` | If a standing grant covers the request, knock is auto-approved, capability is reusable, local/CF outcomes run immediately, `standing.used` is logged. |
| **B Valhalla on key create** | `POST /organic/keys` → Valhalla `?bootstrap=1` | Best-effort. Key create succeeds even if Valhalla is down (`valhalla_error` in response). |
| **C Repo sync hook** | `POST /organic/repositories/sync-all` | Operator (or future cron) syncs every repo. Cron stanza is commented in `wrangler.toml` pending deploy. |
| **D CF inventory adapter** | `cloudflare.inventory` via `execute_locally` | Calls CF API when `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` secrets exist; otherwise returns `credentials_not_configured`. Does not invent secrets. |
| **E Audit projection** | `GET /organic/audit` | Unified organic-safe view: activity + knocks + standing grants. |

## Needs deploy / secrets (blocked in this environment)

- **No wrangler auth** and **Node 20** (wrangler 4 wants Node ≥22) — Workers were **not** deployed from this PR branch.
- Set secrets before CF inventory is live: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.
- Uncomment `[triggers] crons` and wire `#[event(scheduled)]` for continuous sync.
- Dashboard UI for standing grants / audit plate is not in this PR (API-first).

## Safety invariants preserved

- Standing cannot be minted by a machine knock.
- Sensitive scopes stay on the knock path.
- Existing approve/refuse + Turnstile path unchanged for non-standing knocks.
- No Workers/D1 deleted.

## 1 Oct 2026 — wired, deployed, autosave

- The wire patches are applied in the source tree (`crates/key-kernel`,
  `crates/substrate-cloudflare`, `workers/api`). The live kernel already ran
  this code; the repository now matches it. `patches/` stays as the record.
- **Autosave**: Valhalla `close` now saves the workspace as checkpoint
  `autosave-latest` under the key's own hash before destroying the sandbox
  (`snapshot=0` skips it; a failed save never blocks closing and is written
  to the receipt). `provision` with no `checkpoint_id` restores
  `autosave-latest` if it exists (`fresh=1` starts empty).
- Valhalla Worker deployed with `--containers-rollout=none`: the container
  image was not rebuilt (no Docker daemon in the deploying environment).

## 1 Oct 2026 — sandbox actions need the owning key

Before this, `exec`, `write`, `read`, `parity`, `snapshot`, `restore` and
`close` ran for anyone who knew a sandbox id, and sandbox ids appear in logs
and receipts. Each now requires the owning key (`x-curatom-token` header or
`token` query) whose hash matches the session's `key_hash`; otherwise 401
`missing_token` or 403 `not_this_sandbox`. `status` stays open. The kernel's
checkpoint call sends the key. Both Workers deployed and checked: no key →
401, wrong key → 403, owning key → runs.

Agent credentials: Cloudflare KV namespace `claude-workspace-vault` holds the
agent's Curatom key, its Valhalla sandbox id and the current owner
break-glass (`RAJ_TOKEN` was rotated to mint the key).
