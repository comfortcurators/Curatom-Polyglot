//! v1 migration class. Not the container.
//! Runtime uses @cloudflare/sandbox `Sandbox` via getSandbox(env.Sandbox, id).
//! This class must stay exported because wrangler v1 created it.

import { DurableObject } from "cloudflare:workers";

export class SandboxDO extends DurableObject {
  async fetch(): Promise<Response> {
    return new Response(JSON.stringify({ error: "deprecated_use_sdk_sandbox" }), {
      status: 410,
      headers: { "content-type": "application/json" },
    });
  }
}
