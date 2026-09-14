import { Container, ContainerProxy } from "@cloudflare/containers";
import { env } from "cloudflare:workers";

export { ContainerProxy };

export class OrchestratorContainer extends Container {
  defaultPort = 4000;
  requiredPorts = [4000];
  sleepAfter = "2h";
  enableInternet = false;
  allowedHosts = ["curatom.kernel", "hostos-mcp.internal"];
  envVars = {
    PORT: "4000",
    CURATOM_WORKER_URL: "http://curatom.kernel",
    HOSTOS_MCP_URL: "http://hostos-mcp.internal/mcp",
    CURATOM_HMAC_KEY: env.CURATOM_HMAC_KEY,
  };
}

// Static. Instance-field outboundByHost is ignored; the SDK stores handlers
// in outboundByHostRegistry keyed by class name. Without ContainerProxy
// exported above, intercept never installs and Finch to curatom.kernel dies
// under enableInternet = false — which is 202 then no outcome.recorded.
OrchestratorContainer.outboundByHost = {
  "curatom.kernel": async (request, env) => {
    const url = new URL(request.url);
    url.protocol = "https:";
    url.host = "internal";
    return env.CURATOM_KERNEL_INTERNAL.fetch(new Request(url, request));
  },
  "hostos-mcp.internal": async (request, env) => {
    const proxied = new Request(request);
    proxied.headers.set("x-internal-caller", "curatom-orchestrator");
    return env.HOSTOS_MCP.fetch(proxied);
  },
};

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
          JSON.stringify({
            ok: false,
            gateway_version: "ORCH_HANDLER_V2",
            gateway_seen: request.headers.get("x-curatom-probe") ?? "no_probe",
            container_status: "no_container_call",
            error: String(e),
          }),
          { status: 200, headers: { "content-type": "application/json" } },
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          gateway_version: "ORCH_HANDLER_V2",
          gateway_seen: request.headers.get("x-curatom-probe") ?? "no_probe",
          container_status:
            typeof resp !== "undefined" ? resp.status : "no_container_call",
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }
    return new Response("not found", { status: 404 });
  },
};
