import { Container } from "@cloudflare/containers";
import { env } from "cloudflare:workers";

export class OrchestratorContainer extends Container {
  defaultPort = 4000;
  requiredPorts = [4000];
  sleepAfter = "2h";
  enableInternet = false;
  envVars = {
    PORT: "4000",
    CURATOM_WORKER_URL: "http://curatom.kernel",
    CURATOM_HMAC_KEY: env.CURATOM_HMAC_KEY,
  };

  outboundByHost = {
    "curatom.kernel": async (request, env) => {
      const url = new URL(request.url);
      url.protocol = "https:";
      url.host = "internal";
      const forwardReq = new Request(url, request);
      return env.CURATOM_KERNEL_INTERNAL.fetch(forwardReq);
    },
  };
}

export default {
  async fetch(request, env, ctx) {
    console.log("ORCH_FETCH_ENTERED", Date.now(), request.method, request.url);
    console.log("ORCH_PROBE", request.headers.get("x-curatom-probe"));
    const url = new URL(request.url);
    if (url.pathname === "/v1/jobs" && request.method === "POST") {
      const container = env.ORCHESTRATOR.get(
        env.ORCHESTRATOR.idFromName("main"),
      );
      console.log("ORCH_BEFORE_CONTAINER_FETCH", Date.now());
      let resp;
      try {
        resp = await container.fetch(request);
        console.log("ORCH_AFTER_CONTAINER_FETCH", Date.now(), resp.status);
      } catch (e) {
        console.log("ORCH_CONTAINER_FETCH_THREW", String(e), e?.stack);
        return new Response(
          JSON.stringify({ error: "container_fetch_failed", detail: String(e) }),
          { status: 502, headers: { "content-type": "application/json" } },
        );
      }
      return resp;
    }
    return new Response("not found", { status: 404 });
  },
};
