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
