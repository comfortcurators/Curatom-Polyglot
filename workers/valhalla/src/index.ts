import { Sandbox } from "@cloudflare/sandbox";
import { SandboxDO } from "./sandbox-do";

export { Sandbox, SandboxDO };

interface Env {
  Sandbox: DurableObjectNamespace;
  CURATOM_ARTIFACTS: R2Bucket;
  CURATOM_LEDGER: D1Database;
  CURATOM_KERNEL: Fetcher;
  VALHALLA_BASE_URL: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;
    const params = url.searchParams;

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders() });
    }

    if (path === "/valhalla/provision" && request.method === "GET") {
      const token = params.get("token");
      const knock_id = params.get("knock_id");
      const label = params.get("label") ?? "unnamed";
      if (!token) return jsonErr(400, "missing_token");

      const verify = await env.CURATOM_KERNEL.fetch(
        `https://kernel/organic/keys/verify?token=${encodeURIComponent(token)}`
      );
      if (!verify.ok) return jsonErr(401, "token_not_recognized");

      const sandboxId = `vh-${knock_id ?? crypto.randomUUID().slice(0, 8)}`;
      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);

      await stub.fetch("https://sandbox/init", {
        method: "POST",
        body: JSON.stringify({ label, knock_id }),
      });

      await env.CURATOM_LEDGER.prepare(
        `INSERT OR REPLACE INTO sessions
         (session_id, key_hash, key_label, opened_at, round_count, intent_count, pattern_count)
         VALUES (?1, ?2, ?3, ?4, 1, 0, 0)`
      ).bind(sandboxId, await sha256(token), label, new Date().toISOString()).run();

      return jsonOk({
        sandbox_id: sandboxId,
        url: `${env.VALHALLA_BASE_URL}/valhalla/${sandboxId}`,
      });
    }

    if (path.match(/^\/valhalla\/[^/]+\/exec$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const command = params.get("cmd");
      if (!command) return jsonErr(400, "missing_cmd");

      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);
      const resp = await stub.fetch("https://sandbox/exec", {
        method: "POST",
        body: JSON.stringify({ command }),
      });
      return new Response(resp.body, { status: resp.status, headers: corsHeaders() });
    }

    if (path.match(/^\/valhalla\/[^/]+\/write$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const filePath = params.get("path");
      const content = params.get("body") ?? "";
      if (!filePath) return jsonErr(400, "missing_path");

      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);
      await stub.fetch("https://sandbox/write", {
        method: "POST",
        body: JSON.stringify({ path: filePath, content }),
      });
      return jsonOk({ written: filePath, bytes: content.length });
    }

    if (path.match(/^\/valhalla\/[^/]+\/read$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const filePath = params.get("path");
      if (!filePath) return jsonErr(400, "missing_path");

      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);
      const resp = await stub.fetch("https://sandbox/read", {
        method: "POST",
        body: JSON.stringify({ path: filePath }),
      });
      const text = await resp.text();
      return jsonOk({ path: filePath, body: text });
    }

    if (path.match(/^\/valhalla\/[^/]+\/close$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const report = params.get("report") ?? "no report";

      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);

      const logResp = await stub.fetch("https://sandbox/log");
      const logJson = await logResp.json() as { entries: unknown[] };

      const receipt = {
        sandbox_id: sandboxId,
        closed_at: new Date().toISOString(),
        report,
        log: logJson.entries,
      };

      const receiptRef = `valhalla/${sandboxId}/receipt.json`;
      await env.CURATOM_ARTIFACTS.put(receiptRef, JSON.stringify(receipt, null, 2));

      await env.CURATOM_LEDGER.prepare(
        `UPDATE sessions SET closed_at = ?1, receipt_ref = ?2 WHERE session_id = ?3`
      ).bind(new Date().toISOString(), receiptRef, sandboxId).run();

      await stub.fetch("https://sandbox/destroy", { method: "POST" });

      return jsonOk({ closed: true, receipt_ref: receiptRef });
    }

    if (path.match(/^\/valhalla\/[^/]+\/status$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const id = env.Sandbox.idFromName(sandboxId);
      const stub = env.Sandbox.get(id);
      const resp = await stub.fetch("https://sandbox/status");
      return new Response(resp.body, { status: resp.status, headers: corsHeaders() });
    }

    return jsonErr(404, "not_found");
  },
};

function corsHeaders(): Record<string, string> {
  return {
    "access-control-allow-origin": "*",
    "access-control-allow-methods": "GET, OPTIONS",
    "access-control-allow-headers": "content-type",
  };
}

function jsonOk(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json", ...corsHeaders() },
  });
}

function jsonErr(status: number, msg: string): Response {
  return new Response(JSON.stringify({ error: msg }), {
    status,
    headers: { "content-type": "application/json", ...corsHeaders() },
  });
}

async function sha256(input: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(input));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
