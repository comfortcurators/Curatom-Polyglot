import React, { useEffect, useRef, useState } from "react";
import { View, StyleSheet, Platform } from "react-native";
import { TURNSTILE_SITE_KEY } from "../lib/turnstile";

declare global {
  interface Window {
    turnstile?: {
      render: (
        el: HTMLElement,
        opts: {
          sitekey: string;
          callback: (token: string) => void;
          "error-callback"?: () => void;
          "expired-callback"?: () => void;
          theme?: "light" | "dark" | "auto";
        }
      ) => string;
      remove: (id: string) => void;
    };
  }
}

export default function TurnstileGate({ onToken }: { onToken: (token: string) => void }) {
  if (Platform.OS === "web") {
    return <TurnstileWeb onToken={onToken} />;
  }
  return <TurnstileNative onToken={onToken} />;
}

// ---------- WEB ----------
// Loads the real Cloudflare script into the page and renders the widget
// natively in the DOM. This is the correct path for the Expo web export.

function TurnstileWeb({ onToken }: { onToken: (token: string) => void }) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const widgetIdRef = useRef<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const SCRIPT_SRC = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

    function renderWidget() {
      if (!containerRef.current || !window.turnstile) return;
      if (widgetIdRef.current) return;

      widgetIdRef.current = window.turnstile.render(containerRef.current, {
        sitekey: TURNSTILE_SITE_KEY,
        callback: (token: string) => onToken(token),
        "error-callback": () => setError("Turnstile failed. Try again."),
        "expired-callback": () => setError("Turnstile expired. Tap again."),
        theme: "dark",
      });
    }

    if (window.turnstile) {
      renderWidget();
      return;
    }

    const existing = document.querySelector(`script[src="${SCRIPT_SRC}"]`);
    if (existing) {
      existing.addEventListener("load", renderWidget);
      return () => existing.removeEventListener("load", renderWidget);
    }

    const s = document.createElement("script");
    s.src = SCRIPT_SRC;
    s.async = true;
    s.defer = true;
    s.addEventListener("load", renderWidget);
    document.head.appendChild(s);

    return () => {
      s.removeEventListener("load", renderWidget);
      if (widgetIdRef.current && window.turnstile) {
        try { window.turnstile.remove(widgetIdRef.current); } catch {}
        widgetIdRef.current = null;
      }
    };
  }, [onToken]);

  return (
    <View style={s.wrap}>
      <div
        ref={containerRef}
        style={{ display: "flex", justifyContent: "center", minHeight: 66 }}
      />
      {error ? <div style={{ color: "#f66", textAlign: "center", marginTop: 8 }}>{error}</div> : null}
    </View>
  );
}

// ---------- NATIVE (iOS / Android) ----------
// Falls back to the WebView-based version. This is unchanged from what
// already works on mobile.

let WebView: any = null;
try {
  WebView = require("react-native-webview").WebView;
} catch {
  WebView = null;
}

function TurnstileNative({ onToken }: { onToken: (token: string) => void }) {
  if (!WebView) {
    return (
      <View style={s.wrap}>
        <View style={s.fallback}>
          <FallbackText />
        </View>
      </View>
    );
  }

  const html = `
    <!DOCTYPE html>
    <html>
    <head>
      <meta name="viewport" content="width=device-width, initial-scale=1">
      <script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>
      <style>
        body { margin:0; background:#0b0b0c; display:flex; align-items:center;
               justify-content:center; min-height:100vh; }
      </style>
    </head>
    <body>
      <div class="cf-turnstile"
           data-sitekey="${TURNSTILE_SITE_KEY}"
           data-callback="onToken"
           data-theme="dark"></div>
      <script>
        function onToken(token) {
          if (window.ReactNativeWebView) {
            window.ReactNativeWebView.postMessage(JSON.stringify({ token }));
          }
        }
      </script>
    </body>
    </html>
  `;

  return (
    <View style={s.wrap}>
      <WebView
        originWhitelist={["*"]}
        source={{ html }}
        javaScriptEnabled
        domStorageEnabled
        onMessage={(e: any) => {
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

function FallbackText() {
  return null;
}

const s = StyleSheet.create({
  wrap: {
    minHeight: 90,
    borderRadius: 10,
    overflow: "hidden",
    backgroundColor: "#0b0b0c",
    justifyContent: "center",
    alignItems: "center",
  },
  webview: { backgroundColor: "#0b0b0c", width: "100%", height: 90 },
  fallback: { height: 90, justifyContent: "center", alignItems: "center" },
});
