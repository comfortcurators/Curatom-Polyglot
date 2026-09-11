defmodule CuratomOrchestrator.KernelClient do
  @moduledoc "HMAC POST back to the Worker. No token, no scope."

  def report_outcome(body_map) do
    body = Jason.encode!(body_map)
    key = System.fetch_env!("CURATOM_HMAC_KEY") |> CuratomOrchestrator.Attestation.key_bytes()
    sig = :crypto.mac(:hmac, :sha256, key, body) |> Base.encode16(case: :lower)
    url = System.fetch_env!("CURATOM_WORKER_URL") <> "/internal/outcome"

    Finch.build(:post, url, [{"content-type", "application/json"}, {"x-curatom-hmac", sig}], body)
    |> Finch.request(CuratomOrchestrator.Finch)
  end
end
