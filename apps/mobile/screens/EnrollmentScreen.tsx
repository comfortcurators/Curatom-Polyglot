import React, { useState } from "react";
import { View, Text, Pressable, StyleSheet, ActivityIndicator } from "react-native";
import * as api from "../lib/api";

export default function EnrollmentScreen({ onEnrolled }: { onEnrolled: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function begin() {
    setBusy(true);
    setError(null);
    try {
      await api.claim();
      onEnrolled();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <View style={s.root}>
      <Text style={s.h1}>You are the operator.</Text>
      <Text style={s.sub}>
        Curatom is a system where you say what you want in plain language, and machines do the
        work without you touching infrastructure. You are not the operator of the machine. You
        are the one who says what should happen.
      </Text>
      <Text style={s.sub}>
        From here on, when a machine asks for access, you will see it here. You tap approve or
        refuse. That is the whole ceremony.
      </Text>
      {error ? <Text style={s.error}>{error}</Text> : null}
      <Pressable onPress={begin} style={[s.btn, busy && s.btnDim]} disabled={busy}>
        {busy ? <ActivityIndicator color="#000" /> : <Text style={s.btnText}>BEGIN</Text>}
      </Pressable>
    </View>
  );
}

const s = StyleSheet.create({
  root: { flex: 1, backgroundColor: "#0b0b0c", padding: 24, justifyContent: "center" },
  h1: { color: "#fff", fontSize: 26, fontWeight: "700", marginBottom: 16 },
  sub: { color: "#999", fontSize: 16, lineHeight: 24, marginBottom: 16 },
  error: { color: "#f66", marginBottom: 16 },
  btn: { backgroundColor: "#eee", padding: 16, borderRadius: 10, alignItems: "center", marginTop: 12 },
  btnDim: { opacity: 0.5 },
  btnText: { color: "#000", fontWeight: "700" },
});
