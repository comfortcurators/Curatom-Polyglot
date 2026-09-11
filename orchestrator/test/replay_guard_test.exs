defmodule CuratomOrchestrator.ReplayGuardTest do
  use ExUnit.Case

  setup do
    # ReplayGuard is a named GenServer started by the app in mix test if we start it.
    case Process.whereis(CuratomOrchestrator.ReplayGuard) do
      nil -> start_supervised!(CuratomOrchestrator.ReplayGuard)
      _ -> :ok
    end
    :ok
  end

  test "marks and sees a nonce" do
    nonce = "nonce_test_#{System.unique_integer([:positive])}"
    refute CuratomOrchestrator.ReplayGuard.seen?(nonce)
    :ok = CuratomOrchestrator.ReplayGuard.mark(nonce)
    assert CuratomOrchestrator.ReplayGuard.seen?(nonce)
  end
end
