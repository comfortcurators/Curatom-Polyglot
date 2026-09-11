defmodule CuratomOrchestrator.ExecutionWorker do
  @moduledoc "Runs one action. Consumes attestation once, calls adapter, reports back."
  require Logger

  def run(job, action) do
    key = System.fetch_env!("CURATOM_HMAC_KEY") |> CuratomOrchestrator.Attestation.key_bytes()
    nonce = nonce_of(action.attestation)

    cond do
      CuratomOrchestrator.ReplayGuard.seen?(nonce) ->
        Logger.warning("replay rejected nonce=#{inspect(nonce)}")
        :ok

      true ->
        case CuratomOrchestrator.Attestation.verify(action.attestation, key) do
          {:error, reason} ->
            Logger.error("bad attestation reason=#{inspect(reason)}")
            :ok

          {:ok, claims} ->
            if claims["expires_unix"] < System.system_time(:second) do
              Logger.warning("attestation expired nonce=#{inspect(nonce)}")
            else
              CuratomOrchestrator.ReplayGuard.mark(nonce)
              execute(job, claims)
            end
        end
    end
  end

  defp execute(job, claims) do
    adapter = adapter_for(claims["resource"])

    result =
      try do
        adapter.execute(%{
          requester_id: claims["requester_id"],
          resource: claims["resource"],
          operation: claims["operation"],
          grant_id: claims["grant_id"]
        })
      rescue
        e -> {:error, Exception.message(e)}
      end

    report(job, claims, result)
  end

  defp adapter_for("hostos." <> _), do: CuratomOrchestrator.HostOSAdapter.Mock
  defp adapter_for("cloudflare." <> _), do: CuratomOrchestrator.HostOSAdapter.Mock
  defp adapter_for(_), do: raise "unknown adapter target"

  defp report(job, claims, result) do
    {ok, data, error, mock} =
      case result do
        {:ok, d} -> {true, d, nil, true}
        {:error, e} -> {false, nil, to_string(e), true}
      end

    CuratomOrchestrator.KernelClient.report_outcome(%{
      job_id: job.job_id,
      intent_id: job.intent_id,
      grant_id: claims["grant_id"],
      resource: claims["resource"],
      operation: claims["operation"],
      ok: ok,
      data: data,
      error: error,
      mock: mock
    })
  end

  defp nonce_of(token) do
    with [p, _] <- String.split(token, ".", parts: 2),
         {:ok, payload} <- Base.url_decode64(p, padding: false),
         {:ok, claims} <- Jason.decode(payload) do
      claims["nonce"]
    else
      _ -> :crypto.hash(:sha256, token) |> Base.encode16()
    end
  end
end
