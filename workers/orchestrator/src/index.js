import { Container } from "@cloudflare/containers";

export class OrchestratorContainer extends Container {
  defaultPort = 4000;
  requiredPorts = [4000];
  sleepAfter = "2h";
  enableInternet = false;
  envVars = {
    PORT: "4000",
    CURATOM_WORKER_URL: "http://curatom.kernel",
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
    const url = new URL(request.url);
    if (url.pathname === "/v1/jobs" && request.method === "POST") {
      const container = env.ORCHESTRATOR.get(
        env.ORCHESTRATOR.idFromName("main"),
      );
      await container.startAndWaitForPorts({
        startOptions: {
          envVars: {
            CURATOM_HMAC_KEY: env.CURATOM_HMAC_KEY,
            CURATOM_WORKER_URL: "http://curatom.kernel",
            PORT: "4000",
          },
        },
      });
      return container.fetch(request);
    }
    return new Response("not found", { status: 404 });
  },
};
