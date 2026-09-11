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

export async function enrollStart(): Promise<{ session_id: string; expires_at: string }> {
  const { status, body } = await jpost("/organic/enroll/start", {});
  if (status !== 201) throw new Error(`enroll/start: ${status} ${JSON.stringify(body)}`);
  return body;
}

export async function enrollSign1(session_id: string, image_b64: string) {
  const { status, body } = await jpost("/organic/enroll/sign1", { session_id, image_b64 });
  if (status !== 200) throw new Error(`enroll/sign1: ${status} ${JSON.stringify(body)}`);
  return body as { digest: string; ref: string; complete: boolean };
}

export async function enrollSign2(session_id: string, image_b64: string) {
  const { status, body } = await jpost("/organic/enroll/sign2", { session_id, image_b64 });
  if (status !== 200) throw new Error(`enroll/sign2: ${status} ${JSON.stringify(body)}`);
  return body as { digest: string; ref: string; complete: boolean };
}

export async function enrollComplete(session_id: string, display_name?: string) {
  const { status, body } = await jpost("/organic/enroll/complete", { session_id, display_name });
  if (status !== 201) throw new Error(`enroll/complete: ${status} ${JSON.stringify(body)}`);
  return body as { owner_id: string; enrolled_at: string };
}

export async function createIntent(text: string) {
  const { status, body } = await jpost("/organic/intents", { text });
  if (status !== 201) throw new Error(`intent: ${status}`);
  return body;
}

export async function listApprovals() {
  const { status, body } = await jget("/organic/approvals");
  if (status !== 200) throw new Error(`approvals: ${status}`);
  return body;
}

export async function approve(approvalId: string, image_b64: string) {
  const { status, body } = await jpost(`/organic/approvals/${approvalId}/approve`, { image_b64 });
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
