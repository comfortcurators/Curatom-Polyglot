import { DurableObject } from "cloudflare:workers";
import { getSandbox, type Sandbox as SandboxType } from "@cloudflare/sandbox";

interface Env {
  CURATOM_ARTIFACTS: R2Bucket;
}

interface LogEntry {
  at: string;
  kind: "exec" | "write" | "read";
  detail: string;
  result?: string;
}

export class SandboxDO extends DurableObject<Env> {
  private sandbox: SandboxType | null = null;
  private log: LogEntry[] = [];
  private label: string = "";
  private knockId: string = "";

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    if (path === "/init" && request.method === "POST") {
      const body = await request.json() as { label: string; knock_id: string };
      this.label = body.label;
      this.knockId = body.knock_id;

      const id = this.ctx.id.toString();
      this.sandbox = getSandbox(this.ctx, "valhalla");

      await this.ctx.storage.put("label", this.label);
      await this.ctx.storage.put("knock_id", this.knockId);
      await this.ctx.storage.put("opened_at", new Date().toISOString());

      return new Response(JSON.stringify({ ok: true, id }), {
        headers: { "content-type": "application/json" },
      });
    }

    if (path === "/exec" && request.method === "POST") {
      const body = await request.json() as { command: string };
      if (!this.sandbox) return jsonErr(500, "sandbox_not_initialized");

      try {
        const result = await this.sandbox.exec(body.command);
        this.log.push({
          at: new Date().toISOString(),
          kind: "exec",
          detail: body.command,
          result: result.stdout?.slice(0, 2000) ?? "",
        });
        await this.ctx.storage.put("log", this.log);
        return new Response(JSON.stringify({
          stdout: result.stdout,
          stderr: result.stderr,
          exit_code: result.exitCode,
        }), { headers: { "content-type": "application/json" } });
      } catch (e) {
        return jsonErr(500, `exec_failed: ${String(e)}`);
      }
    }

    if (path === "/write" && request.method === "POST") {
      const body = await request.json() as { path: string; content: string };
      if (!this.sandbox) return jsonErr(500, "sandbox_not_initialized");
      try {
        await this.sandbox.writeFile(body.path, body.content);
        this.log.push({
          at: new Date().toISOString(),
          kind: "write",
          detail: body.path,
          result: `${body.content.length} bytes`,
        });
        await this.ctx.storage.put("log", this.log);
        return new Response(JSON.stringify({ ok: true }), {
          headers: { "content-type": "application/json" },
        });
      } catch (e) {
        return jsonErr(500, `write_failed: ${String(e)}`);
      }
    }

    if (path === "/read" && request.method === "POST") {
      const body = await request.json() as { path: string };
      if (!this.sandbox) return jsonErr(500, "sandbox_not_initialized");
      try {
        const content = await this.sandbox.readFile(body.path);
        this.log.push({
          at: new Date().toISOString(),
          kind: "read",
          detail: body.path,
          result: `${content?.length ?? 0} bytes`,
        });
        await this.ctx.storage.put("log", this.log);
        return new Response(content ?? "", {
          headers: { "content-type": "text/plain" },
        });
      } catch (e) {
        return jsonErr(500, `read_failed: ${String(e)}`);
      }
    }

    if (path === "/log" && request.method === "GET") {
      const stored = await this.ctx.storage.get<LogEntry[]>("log") ?? [];
      return new Response(JSON.stringify({ entries: stored }), {
        headers: { "content-type": "application/json" },
      });
    }

    if (path === "/status" && request.method === "GET") {
      const opened = await this.ctx.storage.get<string>("opened_at");
      const label = await this.ctx.storage.get<string>("label");
      const count = (await this.ctx.storage.get<LogEntry[]>("log") ?? []).length;
      return new Response(JSON.stringify({
        sandbox_id: this.ctx.id.toString(),
        label,
        opened_at: opened,
        entry_count: count,
        alive: this.sandbox !== null,
      }), { headers: { "content-type": "application/json" } });
    }

    if (path === "/destroy" && request.method === "POST") {
      if (this.sandbox) {
        try { await this.sandbox.destroy(); } catch {}
        this.sandbox = null;
      }
      await this.ctx.storage.deleteAll();
      return new Response(JSON.stringify({ destroyed: true }), {
        headers: { "content-type": "application/json" },
      });
    }

    return jsonErr(404, "unknown_sandbox_action");
  }
}

function jsonErr(status: number, msg: string): Response {
  return new Response(JSON.stringify({ error: msg }), {
    status,
    headers: { "content-type": "application/json" },
  });
}
