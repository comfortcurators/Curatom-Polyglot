# Agent tools: work in Valhalla, not in the session container

A coding session's own container is small and disposable. Valhalla is the
agent's own Linux box (Rust, C, Python, 4 cores, 16 GB) whose files are saved on
close and restored on the next open. These three commands are all an agent
needs; copy them to `/usr/local/bin` at the start of a session.

| Command | Does |
|---|---|
| `vh "<cmd>"` | run a shell command in my sandbox, print its output |
| `vhpush <local repo> <remote dir>` | copy a repo's committed tree into the sandbox (private repos too; no credentials leave the session) |
| `vhstate` / `vhstate set FILE` | read / replace my working-state note, so the next session starts where this one stopped |

They need only `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`: the Curatom key
and sandbox id are read from the `claude-workspace-vault` KV namespace.

Typical build: `vhpush ~/curator /workspace/curator && vh "cd /workspace/curator && scripts/gate.sh"`.

Firewall: zone rajvansh.dev has one custom rule, "Valhalla sandbox actions with
an owner key", that skips the managed attack rules for `/valhalla/vh-*` requests
carrying `x-curatom-token` (shell commands otherwise look like attacks). Valhalla
itself still rejects any request whose key does not own the sandbox.
