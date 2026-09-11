import React, { useState } from "react";
import { View, Text, Pressable, StyleSheet, ActivityIndicator, ScrollView } from "react-native";
import SignatureCapture from "./SignatureCapture";
import { enrollStart, enrollSign1, enrollSign2, enrollComplete } from "../lib/api";

type Step = "welcome" | "sign1" | "sign2" | "done";

export default function EnrollmentScreen({ onEnrolled }: { onEnrolled: () => void }) {
  const [step, setStep] = useState<Step>("welcome");
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function start() {
    setBusy(true);
    setError(null);
    try {
      const { session_id } = await enrollStart();
      setSessionId(session_id);
      setStep("sign1");
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  async function submitSign1(b64: string) {
    if (!sessionId) return;
    setBusy(true);
    setError(null);
    try {
      await enrollSign1(sessionId, b64);
      setStep("sign2");
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  async function submitSign2(b64: string) {
    if (!sessionId) return;
    setBusy(true);
    setError(null);
    try {
      await enrollSign2(sessionId, b64);
      await enrollComplete(sessionId, "Rajvansh");
      setStep("done");
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  if (step === "sign1") {
    return (
      <SignatureCapture
        title="Recovery signature"
        subtitle="Sign on paper. This is your foundational proof. Store it safely; it never changes."
        onDone={submitSign1}
        onCancel={() => setStep("welcome")}
      />
    );
  }

  if (step === "sign2") {
    return (
      <SignatureCapture
        title="Daily approval signature"
        subtitle="Sign again, a fresh capture. This is what you will do every time you approve an action."
        onDone={submitSign2}
      />
    );
  }

  if (step === "done") {
    return (
      <View style={s.root}>
        <Text style={s.h1}>You are set up.</Text>
        <Text style={s.sub}>
          Two signatures captured. Your account is bound to both. From here on, when a machine asks you to do something, you will sign again.
        </Text>
        <Pressable onPress={onEnrolled} style={s.btn}>
          <Text style={s.btnText}>ENTER</Text>
        </Pressable>
      </View>
    );
  }

  return (
    <ScrollView contentContainerStyle={s.root}>
      <Text style={s.h1}>Set up your account.</Text>
      <Text style={s.sub}>
        Two signatures. One for recovery. One for daily approvals.
        The machine never sees them as passwords — it keeps them as records of your intent.
      </Text>
      {error ? <Text style={s.error}>{error}</Text> : null}
      <Pressable onPress={start} style={[s.btn, busy && s.btnDim]} disabled={busy}>
        {busy ? <ActivityIndicator color="#000" /> : <Text style={s.btnText}>BEGIN</Text>}
      </Pressable>
    </ScrollView>
  );
}

const s = StyleSheet.create({
  root: { flexGrow: 1, backgroundColor: "#0b0b0c", padding: 24, justifyContent: "center" },
  h1: { color: "#fff", fontSize: 26, fontWeight: "700", marginBottom: 12 },
  sub: { color: "#999", fontSize: 16, lineHeight: 24, marginBottom: 24 },
  error: { color: "#f66", marginBottom: 16 },
  btn: { backgroundColor: "#eee", padding: 16, borderRadius: 10, alignItems: "center" },
  btnDim: { opacity: 0.5 },
  btnText: { color: "#000", fontWeight: "700" },
});
