defmodule CuratomOrchestrator.HostOSAdapter.Mock do
  @behaviour CuratomOrchestrator.HostOSAdapter

  @impl true
  def execute(%{operation: op}) when op != "read", do: {:error, "mock provider only supports read"}

  def execute(%{resource: "hostos.inventory"}),
    do: {:ok, %{healthy: true, services: 7, degraded: 0, _mock: true}}

  def execute(%{resource: "hostos.metadata"}),
    do: {:ok, %{version: "0.1.0-mock", region: "local", _mock: true}}

  def execute(%{resource: "cloudflare.inventory"}),
    do: {:ok, %{workers: 1, buckets: 1, _mock: true}}

  def execute(%{resource: other}), do: {:error, "unknown_mock_target:#{other}"}
end
