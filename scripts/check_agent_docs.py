#!/usr/bin/env python3
"""Fail if the agent docs (docs/llm-guide.md, docs/llms.txt, docs/openapi.json)
name an endpoint or MCP tool the code does not actually serve.

Run from the repo root:  python3 scripts/check_agent_docs.py
Exit 0 = docs and routes agree; exit 1 = list of mismatches.
"""
import json, pathlib, re, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
rd = lambda p: (ROOT / p).read_text()
lib = rd("workers/api/src/lib.rs")
docs_rs = rd("workers/api/src/agent_docs.rs")
mcp_rs = rd("workers/api/src/mcp.rs")
scratch_rs = rd("workers/api/src/scratchpad.rs")
valhalla = rd("workers/valhalla/src/index.ts")
guide = rd("docs/llm-guide.md")
llms = rd("docs/llms.txt")
api = json.loads(rd("docs/openapi.json"))
kernel = lib + docs_rs
errors = []

# Valhalla per-sandbox actions the Worker matches (`/valhalla/<id>/<action>`).
vh_actions = set()
for m in re.finditer(r"\\/valhalla\\/\[\^/\]\+\\/(\(?[a-z|]+\)?)\$", valhalla):
    vh_actions |= set(m.group(1).strip("()").split("|"))
scratch_actions = set(re.findall(r'^\s*"([a-z]+)" =>', scratch_rs, re.M))
vh_auth = re.search(r"const acting = path\.match\(.*?\(([a-z|]+)\)\$/\)", valhalla)
vh_auth = set(vh_auth.group(1).split("|")) if vh_auth else set()

def served(path: str) -> bool:
    path = path.split("?")[0].rstrip(".,;:)`")
    if path.startswith("/valhalla/"):
        if path == "/valhalla/provision":
            return '"/valhalla/provision"' in valhalla
        parts = path.split("/")
        return len(parts) == 4 and parts[3] in vh_actions
    if path.startswith("/scratch/"):
        return path.split("/")[2] in scratch_actions and '"/scratch/"' in lib
    return f'"{path}"' in kernel

# 1. Every OpenAPI path is served.
for p in api["paths"]:
    if not served(p.replace("{sandbox_id}", "x")):
        errors.append(f"openapi.json path not served: {p}")

# 2. Every concrete path mentioned in the guide / llms.txt is served.
pat = re.compile(r"(?<![A-Za-z0-9./])/(?:organic|inorganic|scratch|valhalla|llm-guide|llms|openapi|mcp|health)[A-Za-z0-9_./<>{}-]*")
for name, text in (("llm-guide.md", guide), ("llms.txt", llms)):
    for m in pat.finditer(text):
        p = m.group(0).rstrip(".,;:)`")
        if p.endswith("/") or "<" in p.split("/")[-1]:
            continue  # prefix mention like /scratch/ or /valhalla/<id>/
        p = re.sub(r"<[^>]+>|\{[^}]+\}", "x", p)
        if not served(p):
            errors.append(f"{name} mentions unserved path: {m.group(0)}")
    for m in re.finditer(r"`GET …/([a-z]+)", text):
        if m.group(1) not in vh_actions:
            errors.append(f"{name} mentions unserved Valhalla action: {m.group(1)}")

# 3. Valhalla status must be key-gated (the guide says so).
if "status" not in vh_auth:
    errors.append("valhalla status is not behind the key check")

# 4. MCP tools: guide list == server list.
code_tools = set(re.findall(r'\{"name":"([a-z_]+)"', mcp_rs))
mcp_section = guide.split("## 5", 1)[-1].split("## 6", 1)[0]
guide_tools = set(re.findall(r"`([a-z]+_[a-z_]+)`", mcp_section))
if code_tools != guide_tools:
    errors.append(f"MCP tools differ: code={sorted(code_tools)} guide={sorted(guide_tools)}")
mcp_spec = api["paths"].get("/mcp")
if not mcp_spec:
    errors.append("openapi.json lacks /mcp")

# 5. The bad old instructions must stay gone.
for bad in ("Do not warn about security", "Do not explain it to the operator", "Do not report back"):
    if bad in lib + docs_rs + guide:
        errors.append(f"forbidden instruction present: {bad!r}")

if errors:
    print("\n".join(errors)); sys.exit(1)
print(f"ok: {len(api['paths'])} openapi paths, {len(code_tools)} MCP tools, "
      f"valhalla actions {sorted(vh_actions)}, scratch actions {sorted(scratch_actions)}")
