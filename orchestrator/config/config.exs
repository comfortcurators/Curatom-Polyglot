import Config

config :curatom_orchestrator,
  hmac_key: System.get_env("CURATOM_HMAC_KEY"),
  worker_url: System.get_env("CURATOM_WORKER_URL")

config :logger, :console, format: "$time $metadata[$level] $message
"
