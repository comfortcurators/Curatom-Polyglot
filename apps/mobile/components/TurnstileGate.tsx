import React from "react";
import { View, StyleSheet } from "react-native";
import { WebView } from "react-native-webview";
import { TURNSTILE_HTML } from "../lib/turnstile";

export default function TurnstileGate({ onToken }: { onToken: (token: string) => void }) {
  return (
    <View style={s.wrap}>
      <WebView
        originWhitelist={["*"]}
        source={{ html: TURNSTILE_HTML }}
        javaScriptEnabled
        domStorageEnabled
        onMessage={(e) => {
          try {
            const data = JSON.parse(e.nativeEvent.data);
            if (data.token) onToken(data.token);
          } catch {}
        }}
        style={s.webview}
        scrollEnabled={false}
      />
    </View>
  );
}

const s = StyleSheet.create({
  wrap: { height: 90, borderRadius: 10, overflow: "hidden", backgroundColor: "#0b0b0c" },
  webview: { backgroundColor: "#0b0b0c" },
});
