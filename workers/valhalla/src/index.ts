import { ContainerProxy, getSandbox, Sandbox } from "@cloudflare/sandbox";
import { SandboxDO } from "./sandbox-do";

export { ContainerProxy, Sandbox, SandboxDO };

interface Env {
  Sandbox: DurableObjectNamespace<Sandbox>;
  SandboxDO: DurableObjectNamespace;
  CURATOM_ARTIFACTS: R2Bucket;
  CURATOM_LEDGER: D1Database;
  CURATOM_KERNEL: Fetcher;
  VALHALLA_BASE_URL: string;
  CURATOM_KERNEL_HMAC: string;
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
      if (!knock_id) return jsonErr(400, "missing_knock_id");

      const verify = await env.CURATOM_KERNEL.fetch(
        `https://kernel/organic/keys/verify?token=${encodeURIComponent(token)}`
      );
      if (!verify.ok) return jsonErr(401, "token_not_recognized");

      const scope = (params.get("scope") ?? "").split(",").map((s) => s.trim()).filter(Boolean);
      if (scope.length === 0) return jsonErr(400, "missing_scope");

      const authBody =
        `knock_id=${encodeURIComponent(knock_id)}` +
        `&token=${encodeURIComponent(token)}` +
        `&scope=${encodeURIComponent(scope.join(","))}`;
      const authSig = await internalHmac(env, authBody);
      const authResp = await env.CURATOM_KERNEL.fetch(
        `https://kernel/internal/authorize-provision?${authBody}`,
        {
          method: "POST",
          headers: {
            "content-type": "application/x-www-form-urlencoded",
            "x-curatom-hmac": authSig,
          },
          body: authBody,
        }
      );
      if (!authResp.ok) {
        const detail = await authResp.text();
        return jsonErr(403, `provision_not_authorized:${detail}`);
      }

      const sandboxId = `vh-${knock_id}`;
      // SDK: get-or-create. Container starts on first exec, not here.
      const sandbox = getSandbox(env.Sandbox, sandboxId);

      const freezeIds: string[] = [];
      for (const resource of scope) {
        const freezeBody = `scope=${encodeURIComponent(resource)}` +
          `&reason=valhalla_session` +
          `&session_id=${encodeURIComponent(sandboxId)}`;
        const freezeSig = await internalHmac(env, freezeBody);
        const freezeResp = await env.CURATOM_KERNEL.fetch(
          `https://kernel/internal/freeze?${freezeBody}`,
          {
            method: "POST",
            headers: {
              "content-type": "application/x-www-form-urlencoded",
              "x-curatom-hmac": freezeSig,
            },
            body: freezeBody,
          }
        );
        if (!freezeResp.ok) {
          const detail = await freezeResp.text();
          return jsonErr(500, `freeze_failed:${resource}:${detail}`);
        }
        const fb = (await freezeResp.json()) as { freeze_id: string };
        freezeIds.push(fb.freeze_id);
      }

      await env.CURATOM_LEDGER.prepare(
        `INSERT OR REPLACE INTO sessions
         (session_id, key_hash, key_label, opened_at, round_count, intent_count, pattern_count)
         VALUES (?1, ?2, ?3, ?4, 1, 0, 0)`
      ).bind(sandboxId, await sha256(token), label, new Date().toISOString()).run();

      return jsonOk({
        sandbox_id: sandboxId,
        url: `${env.VALHALLA_BASE_URL}/valhalla/${sandboxId}`,
        frozen: freezeIds,
      });
    }

    if (path.match(/^\/valhalla\/[^/]+\/exec$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const command = params.get("cmd");
      if (!command) return jsonErr(400, "missing_cmd");

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        const result = await sandbox.exec(command);
        const stdout = (result as { stdout?: string }).stdout ?? "";
        const stderr = (result as { stderr?: string }).stderr ?? "";
        const exitCode = (result as { exitCode?: number }).exitCode ?? 0;
        await appendLog(env, sandboxId, "exec", command, stdout.slice(0, 2000));
        return jsonOk({ stdout, stderr, exit_code: exitCode });
      } catch (e) {
        return jsonErr(500, `exec_failed: ${String(e)}`);
      }
    }

    if (path.match(/^\/valhalla\/[^/]+\/write$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const filePath = params.get("path");
      const content = params.get("body") ?? "";
      if (!filePath) return jsonErr(400, "missing_path");

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        await sandbox.writeFile(filePath, content);
        await appendLog(env, sandboxId, "write", filePath, `${content.length} bytes`);
        return jsonOk({ written: filePath, bytes: content.length });
      } catch (e) {
        return jsonErr(500, `write_failed: ${String(e)}`);
      }
    }

    if (path.match(/^\/valhalla\/[^/]+\/read$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const filePath = params.get("path");
      if (!filePath) return jsonErr(400, "missing_path");

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        const raw = await sandbox.readFile(filePath);
        const text = typeof raw === "string" ? raw : JSON.stringify(raw);
        await appendLog(env, sandboxId, "read", filePath, `${text.length} bytes`);
        return jsonOk({ path: filePath, body: text });
      } catch (e) {
        return jsonErr(500, `read_failed: ${String(e)}`);
      }
    }

    if (path.match(/^\/valhalla\/[^/]+\/parity$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const scope = params.get("scope") ?? "";
      const operation = params.get("op") ?? "read";
      const claimedDigest = params.get("digest") ?? "";
      const claimedBody = params.get("claim") ?? "";

      if (!scope) return jsonErr(400, "missing_scope");
      if (!claimedDigest && !claimedBody) return jsonErr(400, "missing_digest_or_claim");

      const effectiveDigest = claimedDigest || (await sha256(claimedBody));
      const now = new Date().toISOString();
      await env.CURATOM_LEDGER.prepare(
        `INSERT INTO drift_log (at, kind, subject, detail, severity)
         VALUES (?1, 'parity_claim', ?2, ?3, 'info')`
      ).bind(
        now,
        sandboxId,
        JSON.stringify({ scope, operation, digest: effectiveDigest })
      ).run();

      return jsonOk({
        scope,
        operation,
        claimed_digest: effectiveDigest,
        recorded_at: now,
        note: "Parity claim recorded. To release freezes and close, call close with parity_ok=true and this digest as parity_note.",
      });
    }

    if (path.match(/^\/valhalla\/[^/]+\/close$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const report = params.get("report") ?? "no report";
      const parityOk = params.get("parity_ok") === "true";
      const parityNote = params.get("parity_note") ?? "";

      if (!parityOk) {
        return jsonErr(409, "parity_not_confirmed");
      }

      const releaseBody =
        `session_id=${encodeURIComponent(sandboxId)}` +
        `&parity_ok=true` +
        `&reason=${encodeURIComponent(parityNote)}`;
      const releaseSig = await internalHmac(env, releaseBody);
      const releaseResp = await env.CURATOM_KERNEL.fetch(
        `https://kernel/internal/release-session?${releaseBody}`,
        {
          method: "POST",
          headers: {
            "content-type": "application/x-www-form-urlencoded",
            "x-curatom-hmac": releaseSig,
          },
          body: releaseBody,
        }
      );
      if (!releaseResp.ok) {
        return jsonErr(500, "release_failed");
      }
      const releaseJson = (await releaseResp.json()) as {
        released: number;
        frozen?: string[];
      };
      const frozen = releaseJson.frozen ?? [];
      const entries = await sessionLog(env, sandboxId);

      const receipt = {
        sandbox_id: sandboxId,
        closed_at: new Date().toISOString(),
        report,
        parity_note: parityNote,
        freezes_released: releaseJson.released,
        frozen,
        log: entries,
      };

      const receiptRef = `valhalla/${sandboxId}/receipt.json`;
      await env.CURATOM_ARTIFACTS.put(receiptRef, JSON.stringify(receipt, null, 2));

      await env.CURATOM_LEDGER.prepare(
        `UPDATE sessions SET closed_at = ?1, receipt_ref = ?2 WHERE session_id = ?3`
      ).bind(new Date().toISOString(), receiptRef, sandboxId).run();

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        await sandbox.destroy();
      } catch {
        // already gone
      }

      return jsonOk({
        closed: true,
        receipt_ref: receiptRef,
        freezes_released: releaseJson.released,
        frozen,
      });
    }

    if (path.match(/^\/valhalla\/[^/]+\/status$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const row = await env.CURATOM_LEDGER.prepare(
        `SELECT session_id, key_label, opened_at, closed_at FROM sessions WHERE session_id = ?1`
      ).bind(sandboxId).first<{ session_id: string; key_label: string; opened_at: string; closed_at: string | null }>();
      const entries = await sessionLog(env, sandboxId);
      return jsonOk({
        sandbox_id: sandboxId,
        label: row?.key_label ?? "",
        opened_at: row?.opened_at ?? null,
        entry_count: entries.length,
        alive: row ? row.closed_at == null : false,
      });
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

function hmacKeyBytes(secret: string): Uint8Array {
  try {
    const bin = atob(secret.trim());
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
  } catch {
    return new TextEncoder().encode(secret);
  }
}

async function internalHmac(env: Env, body: string): Promise<string> {
  const key = hmacKeyBytes(env.CURATOM_KERNEL_HMAC);
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const sig = await crypto.subtle.sign("HMAC", cryptoKey, new TextEncoder().encode(body));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function appendLog(
  env: Env,
  sandboxId: string,
  kind: string,
  detail: string,
  result?: string,
): Promise<void> {
  await env.CURATOM_LEDGER.prepare(
    `INSERT INTO drift_log (at, kind, subject, detail, severity)
     VALUES (?1, ?2, ?3, ?4, 'info')`
  ).bind(
    new Date().toISOString(),
    `valhalla_${kind}`,
    sandboxId,
    JSON.stringify({ detail, result }),
  ).run();
}

async function sessionLog(env: Env, sandboxId: string): Promise<unknown[]> {
  const res = await env.CURATOM_LEDGER.prepare(
    `SELECT at, kind, detail FROM drift_log
     WHERE subject = ?1 AND kind LIKE 'valhalla_%'
     ORDER BY at ASC`
  ).bind(sandboxId).all();
  return res.results ?? [];
}
