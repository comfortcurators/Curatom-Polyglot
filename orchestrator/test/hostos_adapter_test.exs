defmodule CuratomOrchestrator.HostOSAdapter.MockTest do
  use ExUnit.Case, async: true

  alias CuratomOrchestrator.HostOSAdapter.Mock

  test "inventory read" do
    assert {:ok, %{healthy: true, services: 7}} =
             Mock.execute(%{resource: "hostos.inventory", operation: "read", requester_id: "x", grant_id: "g"})
  end

  test "write refused" do
    assert {:error, "mock provider only supports read"} =
             Mock.execute(%{resource: "hostos.inventory", operation: "write", requester_id: "x", grant_id: "g"})
  end
end
