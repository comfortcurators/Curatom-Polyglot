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

export type TokenInfo = { token: string; file_text: string; created_at: string };

export async function getToken(): Promise<TokenInfo> {
  const { status, body } = await jget("/organic/token");
  if (status !== 200) throw new Error(`token: ${status}`);
  return body as TokenInfo;
}

export async function rotateToken(): Promise<{ token: string; created_at: string }> {
  const { status, body } = await jpost("/organic/token/rotate", {});
  if (status !== 200) throw new Error(`rotate: ${status} ${JSON.stringify(body)}`);
  return body as { token: string; created_at: string };
}

export type Knock = {
  id: string;
  name: string;
  reason: string;
  resources: string[];
  permissions: string[];
  duration: any;
  expires_at: string;
  seconds_remaining: number;
};

export async function listKnocks(): Promise<Knock[]> {
  const { status, body } = await jget("/organic/knocks");
  if (status !== 200) throw new Error(`knocks: ${status}`);
  return body as Knock[];
}

export async function approveKnock(knockId: string, turnstileToken: string) {
  const { status, body } = await jpost(
    `/organic/knocks/${knockId}/approve`,
    { turnstile_token: turnstileToken }
  );
  if (status !== 200) throw new Error(`approve: ${status} ${JSON.stringify(body)}`);
  return body;
}

export async function refuseKnock(knockId: string) {
  const { status, body } = await jpost(`/organic/knocks/${knockId}/refuse`, {});
  if (status !== 200) throw new Error(`refuse: ${status}`);
  return body;
}

export async function activity() {
  const { status, body } = await jget("/organic/activity");
  if (status !== 200) throw new Error(`activity: ${status}`);
  return body as { seq: number; at: string; kind: string; actor: string }[];
}
