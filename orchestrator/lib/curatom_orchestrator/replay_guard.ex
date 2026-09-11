defmodule CuratomOrchestrator.ReplayGuard do
  @moduledoc "Rejects attestations seen before. ETS-backed, single-node v0."
  use GenServer

  def start_link(_ \\ []), do: GenServer.start_link(__MODULE__, :ok, name: __MODULE__)

  def seen?(nonce), do: GenServer.call(__MODULE__, {:seen?, nonce})
  def mark(nonce), do: GenServer.call(__MODULE__, {:mark, nonce})

  @impl true
  def init(:ok), do: {:ok, :ets.new(:replay_guard, [:set, :protected])}

  @impl true
  def handle_call({:seen?, nonce}, _from, tab), do: {:reply, :ets.member(tab, nonce), tab}

  def handle_call({:mark, nonce}, _from, tab) do
    :ets.insert(tab, {nonce, System.system_time(:second)})
    {:reply, :ok, tab}
  end
end
