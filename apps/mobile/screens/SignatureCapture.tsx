import React, { useState } from "react";
import { View, Text, Pressable, StyleSheet, ActivityIndicator, Image } from "react-native";
import { captureSignature } from "../lib/signature";

type Props = {
  title: string;
  subtitle: string;
  onDone: (imageB64: string) => void;
  onCancel?: () => void;
};

export default function SignatureCapture({ title, subtitle, onDone, onCancel }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);

  async function capture() {
    setBusy(true);
    setError(null);
    try {
      const b64 = await captureSignature();
      if (!b64) { setBusy(false); return; }
      setPreview(`data:image/jpeg;base64,${b64}`);
      setTimeout(() => {
        onDone(b64);
        setBusy(false);
      }, 600);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <View style={s.root}>
      <Text style={s.h1}>{title}</Text>
      <Text style={s.sub}>{subtitle}</Text>

      {preview ? (
        <Image source={{ uri: preview }} style={s.preview} resizeMode="contain" />
      ) : (
        <View style={s.placeholder}>
          <Text style={s.placeholderText}>
            Sign on paper. Photograph the page. Your signature is a record, not a password.
          </Text>
        </View>
      )}

      {error ? <Text style={s.error}>{error}</Text> : null}

      <View style={s.row}>
        {onCancel ? (
          <Pressable onPress={onCancel} style={[s.btn, s.btnGhost]} disabled={busy}>
            <Text style={s.btnGhostText}>BACK</Text>
          </Pressable>
        ) : null}
        <Pressable onPress={capture} style={[s.btn, busy && s.btnDim]} disabled={busy}>
          {busy ? <ActivityIndicator color="#000" /> : <Text style={s.btnText}>CAPTURE</Text>}
        </Pressable>
      </View>
    </View>
  );
}

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: "#0b0b0c", padding: 24, justifyContent: "center" },
  h1: { color: "#fff", fontSize: 22, fontWeight: "700", marginBottom: 8 },
  sub: { color: "#999", fontSize: 15, marginBottom: 24, lineHeight: 22 },
  preview: { width: "100%", height: 320, backgroundColor: "#111", borderRadius: 12, marginBottom: 24 },
  placeholder: { width: "100%", height: 320, backgroundColor: "#111", borderRadius: 12, marginBottom: 24, alignItems: "center", justifyContent: "center", padding: 24 },
  placeholderText: { color: "#666", textAlign: "center", lineHeight: 22 },
  error: { color: "#f66", marginBottom: 16 },
  row: { flexDirection: "row", gap: 12 },
  btn: { flex: 1, backgroundColor: "#eee", padding: 16, borderRadius: 10, alignItems: "center" },
  btnDim: { opacity: 0.5 },
  btnGhost: { backgroundColor: "transparent", borderWidth: 1, borderColor: "#444" },
  btnText: { color: "#000", fontWeight: "700" },
  btnGhostText: { color: "#ccc", fontWeight: "700" },
});
