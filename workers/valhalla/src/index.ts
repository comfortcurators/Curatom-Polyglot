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
      const verifyBody = (await verify.json()) as { workspace_id?: string };

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

      // Keyed on this *key's* own workspace, not the one-off knock id --
      // so every knock approved against the same key reaches the same
      // persistent container and its filesystem, matching "compute & data
      // under their workspace id" rather than a fresh throwaway sandbox
      // per approval. `workspace_id` carries the same owner-prefix shape
      // as `token` (see key-kernel's `create_key`), which is also what
      // lets `/internal/freeze` and `/internal/release-session` below
      // recover the right owner from `session_id` alone. Falls back to
      // the old per-knock id only for a token minted before this field
      // existed (`workspace_id` empty via `serde(default)`).
      const sandboxId = verifyBody.workspace_id ? `vh-${verifyBody.workspace_id}` : `vh-${knock_id}`;
      // SDK: get-or-create. Container starts on first exec, not here.
      const sandbox = getSandbox(env.Sandbox, sandboxId);

      // If a checkpoint was named, restore it *before* the repository
      // materialization below -- the checkpoint represents the state
      // the operator deliberately saved, so it wins over a fresh
      // repository pull if both are requested. The manifest ref comes
      // from the Worker, not from the caller: the caller supplies an
      // opaque `checkpoint_id` and the Worker resolves it against the
      // token's own kernel, so a checkpoint belonging to a different
      // key cannot be restored here.
      const checkpointId = params.get("checkpoint_id");
      let restored: string[] = [];
      if (checkpointId) {
        const resolveBody =
          `token=${encodeURIComponent(token)}` +
          `&checkpoint_id=${encodeURIComponent(checkpointId)}`;
        const resolveSig = await internalHmac(env, resolveBody);
        const resolveResp = await env.CURATOM_KERNEL.fetch(
          `https://kernel/internal/checkpoint-resolve?${resolveBody}`,
          {
            method: "POST",
            headers: {
              "content-type": "application/x-www-form-urlencoded",
              "x-curatom-hmac": resolveSig,
            },
            body: resolveBody,
          }
        );
        if (!resolveResp.ok) {
          const detail = await resolveResp.text();
          return jsonErr(400, `checkpoint_resolve_failed:${detail}`);
        }
        const resolved = (await resolveResp.json()) as {
          manifest_ref: string | null;
          file_count: number;
        };
        if (resolved.manifest_ref) {
          restored = await restoreFromManifest(env, sandbox, resolved.manifest_ref);
          await appendLog(
            env,
            sandboxId,
            "checkpoint_restore",
            checkpointId,
            `${restored.length} files`
          );
        }
      }

      // If this knock also named a repository, materialize what's
      // actually stored for it (the manifest `h_sync_repository` or
      // `h_upload_repository` wrote to R2) into the workspace before
      // handing it back -- "data under their workspace id" made literal,
      // not just a claim. Best-effort: a sandbox with no repository
      // requested, or one whose manifest read fails, still provisions.
      const repositoryId = params.get("repository_id");
      let materialized: string[] = [];
      if (repositoryId) {
        materialized = await materializeRepository(env, sandbox, token, repositoryId);
        await appendLog(env, sandboxId, "materialize", repositoryId, `${materialized.length} files`);
      }

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
        workspace_id: verifyBody.workspace_id ?? null,
        url: `${env.VALHALLA_BASE_URL}/valhalla/${sandboxId}`,
        frozen: freezeIds,
        materialized,
        restored,
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
        // Cloudflare Sandbox may return either a string or a result object
        // containing `content`; normalize both shapes without serializing the
        // SDK wrapper itself into the file body.
        const text = raw && typeof raw === "object" && "content" in raw
          ? String((raw as { content: unknown }).content ?? "")
          : typeof raw === "string"
            ? raw
            : JSON.stringify(raw);
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

    if (path.match(/^\/valhalla\/[^/]+\/snapshot$/) && request.method === "GET") {
      const sandboxId = path.split("/")[2];
      const keyHash = params.get("key_hash") ?? "";
      const checkpointId = params.get("checkpoint_id") ?? "";
      if (!keyHash) return jsonErr(400, "missing_key_hash");
      if (!checkpointId) return jsonErr(400, "missing_checkpoint_id");

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        const result = await snapshotWorkspace(env, sandbox, keyHash, checkpointId, sandboxId);
        await appendLog(
          env,
          sandboxId,
          "snapshot",
          checkpointId,
          `${result.file_count} files`
        );
        return jsonOk(result);
      } catch (e) {
        return jsonErr(500, `snapshot_failed: ${String(e)}`);
      }
    }

    if (path.match(/^\/valhalla\/[^/]+\/restore$/) && request.method === "GET") {
      // Direct restore without the round-trip through provision. Used
      // for a manual re-apply against a running session; provision
      // calls `restoreFromManifest` inline instead.
      const sandboxId = path.split("/")[2];
      const manifestRef = params.get("manifest_ref") ?? "";
      if (!manifestRef) return jsonErr(400, "missing_manifest_ref");
      if (manifestRef.includes("..")) return jsonErr(400, "bad_manifest_ref");

      try {
        const sandbox = getSandbox(env.Sandbox, sandboxId);
        const written = await restoreFromManifest(env, sandbox, manifestRef);
        await appendLog(env, sandboxId, "restore", manifestRef, `${written.length} files`);
        return jsonOk({ manifest_ref: manifestRef, written });
      } catch (e) {
        return jsonErr(500, `restore_failed: ${String(e)}`);
      }
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

/// Writes a repository's stored manifest into the sandbox's own
/// filesystem, so "compute & data under their workspace id" is literal
/// rather than a claim. Reads R2 directly at the exact path
/// `h_sync_repository`/`h_upload_repository` write to
/// (`repos/{owner_id}/{id}/manifest.json`) instead of calling a route on
/// `CURATOM_KERNEL`, because every organic repository route requires a
/// session cookie and this call carries only a bearer token -- the same
/// reason `workspace_id` was given the token's own owner-prefix shape.
/// `owner_id` here comes from the *verified* token's own prefix, never
/// from anything the caller can otherwise choose, so this cannot be
/// asked to materialize a different owner's repository.
async function materializeRepository(
  env: Env,
  sandbox: ReturnType<typeof getSandbox>,
  token: string,
  repositoryId: string,
): Promise<string[]> {
  const ownerId = token.split(".")[0];
  if (!ownerId) return [];
  const manifestRef = `repos/${ownerId}/${repositoryId}/manifest.json`;
  const obj = await env.CURATOM_ARTIFACTS.get(manifestRef);
  if (!obj) return [];
  let manifest: { manifests?: { path: string; content: string }[] };
  try {
    manifest = JSON.parse(await obj.text());
  } catch {
    return [];
  }
  const written: string[] = [];
  for (const f of (manifest.manifests ?? []).slice(0, 200)) {
    if (!f.path || f.path.includes("..")) continue;
    try {
      await sandbox.writeFile(f.path, f.content);
      written.push(f.path);
    } catch {
      // One bad file must not fail provisioning for the rest.
    }
  }
  return written;
}

/// Walk `/workspace` in the sandbox, upload each file's content to a
/// content-addressed R2 key, and write a manifest listing every file
/// plus its hash and size. Returns the manifest's R2 key and the number
/// of files captured.
///
/// Bounds mirror `materializeRepository`'s but are larger because a
/// working sandbox legitimately holds more than a curated repository:
/// 500 files, 2 MB/file, 50 MB total. Over those, the snapshot is
/// truncated rather than failing -- a partial snapshot that restores
/// most of a workspace is more useful than no snapshot at all, and the
/// manifest's own `truncated` flag says which happened.
async function snapshotWorkspace(
  env: Env,
  sandbox: ReturnType<typeof getSandbox>,
  keyHash: string,
  checkpointId: string,
  sandboxId: string,
): Promise<{ manifest_ref: string; file_count: number; truncated: boolean }> {
  const MAX_FILES = 500;
  const MAX_FILE_BYTES = 2_000_000;
  const MAX_TOTAL_BYTES = 50_000_000;

  // `.git` is excluded because a workspace snapshot is about files the
  // operator cares about, not the repository's internal object store,
  // which is both huge and reproducible from the working tree.
  //
  // Newline-separated, not `-print0`: verified against a real sandbox
  // container (Phase 4 verification, 19 Sep 2026) that NUL bytes do
  // not survive `exec`'s stdout on its way through this SDK's JSON
  // transport -- `-print0`'s output arrived with every path's NUL
  // separator silently stripped, concatenating every result into one
  // unsplittable string and producing a snapshot with 0 files. Plain
  // `find` (default `-print`, newline-terminated) round-trips intact.
  // This assumes no path under `/workspace` contains a literal
  // newline, which is true of every real filename this tool will ever
  // see and is the same assumption `materializeRepository` already
  // makes about its own manifest paths.
  const findResult = await sandbox.exec(
    "find /workspace -type f -not -path '*/.git/*' -not -path '*/.cache/*'"
  );
  const raw = (findResult as { stdout?: string }).stdout ?? "";
  const allPaths = raw
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean)
    // Path-traversal guard. `find` should never produce one, but the
    // value is about to be used as an R2 key suffix and as a
    // `writeFile` target on restore, so it's checked at the source.
    .filter((p) => !p.includes(".."));

  const truncated = allPaths.length > MAX_FILES;
  const paths = allPaths.slice(0, MAX_FILES);

  const entries: { path: string; sha256: string; size: number }[] = [];
  let total = 0;
  let stoppedBySize = false;

  for (const filePath of paths) {
    let content: string;
    try {
      const read = await sandbox.readFile(filePath);
      // The SDK's `readFile` returns a result object
      // (`{success, path, content, ...}`), not a raw string -- unlike
      // what an earlier `typeof read === "string"` assumption expected.
      // Verified against the local sandbox container's real output
      // (Phase 4 verification, 19 Sep 2026): serializing the whole object
      // instead of extracting `.content` silently wrote wrapper metadata
      // into every snapshot blob rather than the file's actual bytes. The
      // public `/read` endpoint uses the same normalization in rv0.4.0.
      if (read && typeof read === "object" && "content" in read) {
        content = String((read as { content: unknown }).content ?? "");
      } else {
        content = typeof read === "string" ? read : JSON.stringify(read);
      }
    } catch {
      continue;
    }
    if (content.length > MAX_FILE_BYTES) continue;
    if (total + content.length > MAX_TOTAL_BYTES) {
      stoppedBySize = true;
      break;
    }
    const hash = await sha256(content);
    await env.CURATOM_ARTIFACTS.put(`checkpoints/blobs/${keyHash}/${hash}`, content);
    entries.push({ path: filePath, sha256: hash, size: content.length });
    total += content.length;
  }

  const manifest = {
    sandbox_id: sandboxId,
    key_hash: keyHash,
    created_at: new Date().toISOString(),
    file_count: entries.length,
    total_bytes: total,
    truncated: truncated || stoppedBySize,
    files: entries,
  };
  const manifestRef = `checkpoints/manifests/${keyHash}/${checkpointId}.json`;
  await env.CURATOM_ARTIFACTS.put(manifestRef, JSON.stringify(manifest));

  return {
    manifest_ref: manifestRef,
    file_count: entries.length,
    truncated: manifest.truncated,
  };
}

/// Inverse of `snapshotWorkspace`. Reads the manifest, fetches each
/// blob by hash, **verifies the hash on read**, and writes the file to
/// the sandbox. A blob whose content no longer matches its hash is
/// skipped rather than written -- corruption or a hash-lookup collision
/// would otherwise silently restore the wrong bytes into a workspace the
/// operator believes is a faithful restore.
///
/// One bad file does not fail the restore; the same discipline as
/// `materializeRepository`. The return value lists what was actually
/// written, so a caller can compare against the manifest's `file_count`
/// and see whether anything was skipped.
async function restoreFromManifest(
  env: Env,
  sandbox: ReturnType<typeof getSandbox>,
  manifestRef: string,
): Promise<string[]> {
  const obj = await env.CURATOM_ARTIFACTS.get(manifestRef);
  if (!obj) return [];

  let manifest: {
    files?: { path: string; sha256: string; size: number }[];
  };
  try {
    manifest = JSON.parse(await obj.text());
  } catch {
    return [];
  }

  // The key_hash prefix on blob keys is recoverable from the manifest
  // ref itself: `checkpoints/manifests/<key_hash>/<id>.json`. Reading it
  // back from the ref rather than trusting a second parameter keeps the
  // two lookups (manifest, blobs) provably scoped to the same owner.
  const keyHash = manifestRef.split("/")[2];
  if (!keyHash) return [];

  const written: string[] = [];
  for (const f of (manifest.files ?? []).slice(0, 500)) {
    if (!f.path || f.path.includes("..")) continue;
    const blobRef = `checkpoints/blobs/${keyHash}/${f.sha256}`;
    const blob = await env.CURATOM_ARTIFACTS.get(blobRef);
    if (!blob) continue;
    const content = await blob.text();
    const check = await sha256(content);
    if (check !== f.sha256) continue;
    try {
      await sandbox.writeFile(f.path, content);
      written.push(f.path);
    } catch {
      // One bad file must not fail the restore for the rest.
    }
  }
  return written;
}
