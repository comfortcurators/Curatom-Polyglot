const BASE = process.env.EXPO_PUBLIC_CURATOM_API ?? "http://localhost:8787";

function devHeaders(): Record<string, string> {
  const id = process.env.EXPO_PUBLIC_DEV_ORGANIC_ID;
  return id ? { "x-curatom-dev-organic": id } : {};
}

async function jpost(path: string, body: unknown) {
  const r = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", ...devHeaders() },
    body: JSON.stringify(body),
  });
  const text = await r.text();
  const parsed = text ? JSON.parse(text) : {};
  return { status: r.status, body: parsed };
}

async function jget(path: string) {
  const r = await fetch(`${BASE}${path}`, { headers: devHeaders() });
  const text = await r.text();
  const parsed = text ? JSON.parse(text) : {};
  return { status: r.status, body: parsed };
}

export type Me = {
  id: string;
  display_name: string;
  mode: string;
  enrolled: boolean;
  enrolled_at: string | null;
};

export async function me(): Promise<Me> {
  const { status, body } = await jget("/organic/me");
  if (status !== 200) throw new Error(`me: ${status}`);
  return body as Me;
}

export async function claim(displayName?: string) {
  const { status, body } = await jpost("/organic/claim", { display_name: displayName });
  if (status !== 201) throw new Error(`claim: ${status} ${JSON.stringify(body)}`);
  return body as { owner_id: string; display_name: string; claimed_at: string };
}

export async function createIntent(text: string) {
  const { status, body } = await jpost("/organic/intents", { text });
  if (status !== 201) throw new Error(`intent: ${status}`);
  return body;
}

export async function listApprovals() {
  const { status, body } = await jget("/organic/approvals");
  if (status !== 200) throw new Error(`approvals: ${status}`);
  return body as Array<{
    id: string;
    requester: string;
    reason: string;
    resources: string[];
    permissions: string[];
    duration: string;
  }>;
}

export async function approve(approvalId: string) {
  const { status, body } = await jpost(`/organic/approvals/${approvalId}/approve`, {});
  if (status !== 200) throw new Error(`approve: ${status} ${JSON.stringify(body)}`);
  return body;
}

export async function refuse(approvalId: string) {
  const { status, body } = await jpost(`/organic/approvals/${approvalId}/refuse`, {});
  if (status !== 200) throw new Error(`refuse: ${status}`);
  return body;
}

export async function activity() {
  const { status, body } = await jget("/organic/activity");
  if (status !== 200) throw new Error(`activity: ${status}`);
  return body as { seq: number; at: string; kind: string; actor: string }[];
}
