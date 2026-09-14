/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_CURATOM_API?: string;
  readonly VITE_DEV_ORGANIC_ID?: string;
  readonly VITE_TURNSTILE_SITE_KEY?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
