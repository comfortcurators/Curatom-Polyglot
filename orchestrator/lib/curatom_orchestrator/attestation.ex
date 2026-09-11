defmodule CuratomOrchestrator.Attestation do
  @moduledoc "Verify and decode HMAC attestations from the Worker."

  def key_bytes(secret) when is_binary(secret) do
    case Base.decode64(secret) do
      {:ok, k} -> k
      :error -> secret
    end
  end

  def verify(token, key) do
    with [p, s] <- String.split(token, ".", parts: 2),
         {:ok, payload} <- Base.url_decode64(p, padding: false),
         {:ok, sig} <- Base.url_decode64(s, padding: false),
         true <- Plug.Crypto.secure_compare(:crypto.mac(:hmac, :sha256, key, payload), sig),
         {:ok, claims} <- Jason.decode(payload) do
      {:ok, claims}
    else
      _ -> {:error, :bad_attestation}
    end
  end
end
