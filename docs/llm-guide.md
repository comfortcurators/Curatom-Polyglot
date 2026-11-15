# Curatom — guide for AI agents and the people who run them

Version: 2026-11-15 (guide v2). This one document is served at
`/llm-guide` (JSON-wrapped), `/llm-guide.md` (plain markdown) and summarised in
`/llms.txt`. The machine-readable contract is `/openapi.json`. If anything here
disagrees with what the live service does, the live service wins — and please
tell the operator (see "Flag problems").

Base URL: `https://curatom.rajvansh.dev`
Computer (Valhalla) URL: `https://valhalla.rajvansh.dev`

## 1. What your key is

Curatom is Comfort Curators' company workspace. A person (the operator) gave
you a **key**: a long string that looks like `<owner_id>.<random>`.

- It identifies you to this one workspace. It is the only credential you need.
- It can have an expiry date. After expiry every call answers `401`.
- The operator can revoke or replace it at any time. A revoked key answers `401`.
- Keep it secret. Never paste it into public places, logs, issues or chats.

What the key can do **by itself**:

- check itself: `GET /organic/keys/verify`
- see the request form and which resources exist: `POST /inorganic/handoff`
- ask for a resource: `POST /inorganic/submit`
- use its own private notepad: `/scratch/*`
- use its own Linux computer in Valhalla (keys made since 30 Sep 2026 get one
  automatically when the key is created; its id is `vh-<workspace_id>`)

What needs the operator:

- Reading a resource needs either the operator's approval of your request
  (they have 88 seconds) **or** a *standing grant* the operator issued to your
  key in advance. With a standing grant your request is answered at once.
- Keys cannot create keys, approve requests, issue grants, or open the
  operator dashboard (`/organic/*` other than `verify` answers `401` for keys).

## 2. How to send the key

Preferred: an HTTP header on every request

    x-curatom-token: YOUR_KEY

Also accepted (older clients):
- `?token=YOUR_KEY` in the URL (works everywhere, but URLs end up in logs — use
  the header when you can);
- `"token": "YOUR_KEY"` in the JSON body of `/inorganic/handoff` and `/inorganic/submit`;
- for the MCP endpoint only: `Authorization: Bearer YOUR_KEY`.

## 3. Ways to connect (pick what your AI supports)

| You are… | Use |
|---|---|
| An MCP client (Claude, ChatGPT apps/connectors, Grok connectors or xAI API remote MCP, Gemini CLI, Cursor, VS Code, Codex, Mistral, Perplexity…) | Remote MCP server `https://curatom.rajvansh.dev/mcp` (Streamable HTTP, JSON responses). Send the key as `Authorization: Bearer` or `x-curatom-token`. If your client only accepts a URL, use `https://curatom.rajvansh.dev/mcp?token=YOUR_KEY` (less safe). |
| A function/tool-calling API (OpenAI, DeepSeek, Gemini, Grok, Mistral, Qwen…) | Import `/openapi.json` as tools (GPT Actions accept it directly), or write small functions that call the HTTP endpoints below. |
| An agent with a shell or HTTP (Codex, Claude Code, Devin, scripts) | Plain HTTP, e.g. `curl -H "x-curatom-token: $KEY" https://curatom.rajvansh.dev/organic/keys/verify` |
| A chat-only model that cannot make HTTP calls | Copy-paste mode: tell your human the exact request to run (method, URL, header, body); they run it and paste the answer back. Never ask them to paste the key into a shared chat. |

## 4. Endpoints for keys (HTTP)

All answers are JSON unless stated. Times are UTC ISO-8601.

### Discover
- `GET /health` → `{"ok":true,...}` (no key)
- `GET /llms.txt`, `GET /llm-guide`, `GET /llm-guide.md`, `GET /openapi.json` (no key)
- Any other unknown path answers `404 {"error":"not_found"}`.

### Check your key
- `GET /organic/keys/verify` → `200 {"ok":true,"workspace_id":"..."}` or `401`

### See what you can ask for
- `POST /inorganic/handoff` with body `{}` (key in header) or `{"token":"..."}`
  → `200 {"form":{...},"available_resources":[...],"available_permissions":["read","write"]}`

### Ask for a resource
- `POST /inorganic/submit` body:
  `{"name":"<who you are>","reason":"<one sentence>","resources":["company.whitepaper"],"permissions":["read"],"duration":{"kind":"single_use"}}`
  (`duration` optional: `{"kind":"single_use"}` or `{"kind":"ttl","seconds":N}`)
  - `200 {"standing":true,"outcomes":[{"resource","ok","data","error"}],...}`
    — a standing grant covered it; the data is in `outcomes`.
  - `201 {"standing":false,"knock_id","expires_at",...}` — a pending request.
    The operator sees it and has 88 seconds to approve. **There is currently no
    key-facing endpoint to read the result of an approved pending request**;
    an approval is used to open/provision your computer (`knock_id`), and the
    result is recorded for the operator. For data you need repeatedly, ask the
    operator for a standing grant.
  - `400 missing_token|missing_name|missing_reason|missing_resources|missing_permissions|bad_permission|invalid_json`,
    `401 token_not_recognized` (wrong, expired or revoked key).
- Image fallback for clients that can only load an image URL:
  `GET /inorganic/knock?token=..&name=..&reason=..&resources=a,b&permissions=read`
  → always a 1×1 GIF; it creates the same pending request.

Resources (the handoff form is the authoritative list for your key):

| Resource | What it holds |
|---|---|
| `company.whitepaper` | Operator-written company context |
| `company.inventory` | This workspace's contents: key labels, connectors, repositories, checkpoint count |
| `repository.inventory` | Repositories registered in Curatom |
| `repository.<id>` | One registered repository |
| `cloudflare.inventory` | Live Cloudflare account inventory (real API, read-only) |
| `hostos.inventory`, `hostos.metadata` | HostOS targets — mock adapters in this release |
| `compute.<connector_id>` | Operator-defined connectors, if any |

`read` returns data. `write` exists in the protocol but a request carries no
write payload, so it does not change built-in resources.

### Your notepad (private to your key)
- `GET /scratch/write?kind=intent|pattern|note&body=TEXT[&new_round=1][&knock_id=..]` → `{"note_id","round","at","seq"}`
- `GET /scratch/notes` → current session's notes
- `GET /scratch/history` → past sessions
- `GET /scratch/open` → session info
- `GET /scratch/close` → closes the session; notes are kept permanently in the operator's ledger
- Limits: `body` up to 16,000 bytes; the whole URL must stay under about 16 KB
  after URL-encoding, so long text should be split into several notes.
  URL-encode the body. Encode a literal plus sign as `%2B` (a bare `+` means space).
- Errors: `400 missing_token|empty_body`, `401 token_not_recognized`, `413 body_too_large`, `404 unknown_scratch_action`.
- The operator can read your notes. Write as if they will be read — they will.

### Your computer (Valhalla)
Base `https://valhalla.rajvansh.dev/valhalla/<sandbox_id>/…`; every call needs
your key in `x-curatom-token` (or `?token=`), and only the key that owns the
sandbox is accepted (`403 not_this_sandbox` otherwise).
- `GET …/status` → `{"sandbox_id","label","opened_at","entry_count","alive"}`
- `GET …/exec?cmd=COMMAND` → command output
- `GET …/write?path=/workspace/f&body=TEXT`, `GET …/read?path=/workspace/f`
- `GET …/snapshot?key_hash=..&checkpoint_id=..`, `GET …/restore?manifest_ref=..` —
  save / restore workspace files against a checkpoint the operator created
  (errors `400 missing_key_hash|missing_checkpoint_id|missing_manifest_ref`)
- `GET …/parity?scope=RESOURCE&op=read&digest=SHA256` then
  `GET …/close?report=TEXT&parity_ok=true&parity_note=DIGEST` — check out.
  Closing without `parity_ok=true` answers `409 parity_not_confirmed`.
  Closing ends the sandbox; most agents should simply leave it open.
- `GET /valhalla/provision?token=..&knock_id=..` — (re)open a computer after an approved request

It is an isolated Linux box; nothing in it touches production by itself.
Files survive a close only if they were snapshotted (an autosave is attempted on close).

## 5. MCP tools (at `/mcp`)
`curatom_guide`, `verify_key`, `list_resources`, `request_access`,
`notes_write`, `notes_read`, `notes_history`, `computer_status`.
Running commands on the computer is not exposed over MCP; use HTTP for that.

## 6. Flag problems — please do
Be honest with the operator. If something fails, looks wrong, looks unsafe, or
this guide does not match what you see, say so plainly: write a `note` in your
notepad starting with `PROBLEM:` and tell the person you are working for.
Do not try to get around a refusal, a `401`/`403`, or a missing approval, and
do not retry the same request in a loop (the operator sees every request).
Use only the endpoints in this guide.

## 7. Revoke, expiry, contact
- The operator manages keys in the Curatom dashboard (Keys): revoke, replace,
  set validity. A key holder cannot revoke or extend its own key.
- If you think your key leaked, tell the operator at once so they revoke it.
- Contact: the person who gave you the key. Company: Comfort Curators,
  https://comfortcurator.com
