const BASE = process.env.EXPO_PUBLIC_CURATOM_API ?? "http://localhost:8787";

function devHeaders(): Record<string, string> {
  const id = process.env.EXPO_PUBLIC_DEV_ORGANIC_ID;
  return id ? { "x-curatom-dev-organic": id } : {};
}

async function parseBody(text: string): Promise<Record<string, unknown> | unknown> {
  if (!text) return {};
  try {
    const parsed = JSON.parse(text);
    if (parsed && typeof parsed === "object") return parsed;
    return { __value: parsed };
  } catch {
    return {
      __error: "non_json_response",
      __raw: text.slice(0, 300),
    };
  }
}

async function jpost(path: string, body: unknown) {
  const r = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", ...devHeaders() },
    body: JSON.stringify(body),
  });
  const text = await r.text();
  return { status: r.status, body: await parseBody(text) };
}

async function jget(path: string) {
  const r = await fetch(`${BASE}${path}`, { headers: devHeaders() });
  const text = await r.text();
  return { status: r.status, body: await parseBody(text) };
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

export type KeyInfo = {
  token: string;
  label: string;
  created_at: string;
  last_used_at: string | null;
  activity_count: number;
};

export type KeyLogEntry = {
  kind: string;
  at: string;
  knock_id: string | null;
  name: string | null;
  reason: string | null;
};

export async function listKeys(): Promise<KeyInfo[]> {
  const { status, body } = await jget("/organic/keys");
  if (status !== 200) throw new Error(`keys: ${status} ${JSON.stringify(body)}`);
  return body as KeyInfo[];
}

export async function createKey(label: string) {
  const { status, body } = await jpost("/organic/keys", { label });
  if (status !== 201) throw new Error(`createKey: ${status} ${JSON.stringify(body)}`);
  return body as { token: string; label: string; created_at: string; file_text: string };
}

export async function revokeKey(token: string) {
  const { status, body } = await jpost(`/organic/keys/${encodeURIComponent(token)}/revoke`, {});
  if (status !== 200) throw new Error(`revokeKey: ${status} ${JSON.stringify(body)}`);
  return body;
}

export async function keyLog(token: string): Promise<KeyLogEntry[]> {
  const { status, body } = await jget(`/organic/keys/${encodeURIComponent(token)}/log`);
  if (status !== 200) throw new Error(`keyLog: ${status} ${JSON.stringify(body)}`);
  return body as KeyLogEntry[];
}

export type BillboardEntry = {
  id: string;
  key_hash: string;
  key_label: string;
  session_id: string;
  round: number;
  kind: string;
  body_ref: string;
  knock_id: string;
  created_at: string;
  seq: number;
};

export async function billboard(params?: {
  key_hash?: string;
  kind?: string;
  limit?: number;
}): Promise<BillboardEntry[]> {
  const q = new URLSearchParams();
  if (params?.key_hash) q.set("key_hash", params.key_hash);
  if (params?.kind) q.set("kind", params.kind);
  if (params?.limit) q.set("limit", String(params.limit));
  const suffix = q.toString() ? `?${q.toString()}` : "";
  const { status, body } = await jget(`/organic/billboard${suffix}`);
  if (status !== 200) throw new Error(`billboard: ${status} ${JSON.stringify(body)}`);
  return (body as { entries: BillboardEntry[] }).entries ?? [];
}

export async function billboardBlob(ref: string): Promise<string> {
  const { status, body } = await jget(`/organic/billboard/blob?ref=${encodeURIComponent(ref)}`);
  if (status !== 200) throw new Error(`blob: ${status} ${JSON.stringify(body)}`);
  return (body as { body: string }).body ?? "";
}
