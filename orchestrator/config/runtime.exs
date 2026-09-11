import Config

if config_env() == :prod do
  config :curatom_orchestrator,
    hmac_key: System.fetch_env!("CURATOM_HMAC_KEY"),
    worker_url: System.fetch_env!("CURATOM_WORKER_URL")
end
