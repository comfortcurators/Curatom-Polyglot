defmodule CuratomOrchestrator.JobQueue do
  @moduledoc "Accepts jobs from the Worker, fans out per action."
  use GenServer

  def start_link(_ \\ []), do: GenServer.start_link(__MODULE__, :ok, name: __MODULE__)

  def submit(job), do: GenServer.call(__MODULE__, {:submit, job})

  @impl true
  def init(:ok), do: {:ok, %{}}

  @impl true
  def handle_call({:submit, job}, _from, state) do
    for action <- job.actions do
      Task.Supervisor.start_child(CuratomOrchestrator.ExecutionSupervisor, fn ->
        CuratomOrchestrator.ExecutionWorker.run(job, action)
      end)
    end

    {:reply, :ok, state}
  end
end
