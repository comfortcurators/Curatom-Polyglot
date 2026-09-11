export const TURNSTILE_SITE_KEY = process.env.EXPO_PUBLIC_TURNSTILE_SITE_KEY ?? "";

export const TURNSTILE_HTML = `
<!DOCTYPE html>
<html>
<head>
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1">
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
