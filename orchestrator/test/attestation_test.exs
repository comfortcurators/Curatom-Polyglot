defmodule CuratomOrchestrator.AttestationTest do
  use ExUnit.Case, async: true

  alias CuratomOrchestrator.Attestation

  test "roundtrip with raw key" do
    key = Attestation.key_bytes("not-b64!!")
    claims = %{
      "v" => 1,
      "job_id" => "job_1",
      "grant_id" => "grt_1",
      "requester_id" => "fleet.curatom",
      "resource" => "hostos.inventory",
      "operation" => "read",
      "issued_unix" => 1000,
      "expires_unix" => 1300,
      "nonce" => "nonce_1"
    }

    payload = Jason.encode!(claims)
    sig = :crypto.mac(:hmac, :sha256, key, payload)
    token = Base.url_encode64(payload, padding: false) <> "." <> Base.url_encode64(sig, padding: false)

    assert {:ok, ^claims} = Attestation.verify(token, key)
  end

  test "bad signature" do
    assert {:error, :bad_attestation} = Attestation.verify("nope.nope", "key")
  end

  test "key_bytes accepts base64 or raw" do
    assert Attestation.key_bytes("YWI=") == "ab"
    assert Attestation.key_bytes("not-b64!!") == "not-b64!!"
  end
end
