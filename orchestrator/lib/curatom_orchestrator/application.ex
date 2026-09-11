defmodule CuratomOrchestrator.Application do
  @moduledoc false
  use Application

  @impl true
  def start(_type, _args) do
    children = [
      {Finch, name: CuratomOrchestrator.Finch},
      {Task.Supervisor, name: CuratomOrchestrator.ExecutionSupervisor},
      CuratomOrchestrator.ReplayGuard,
      CuratomOrchestrator.JobQueue,
      {Plug.Cowboy,
       scheme: :http,
       plug: CuratomOrchestrator.Router,
       options: [port: String.to_integer(System.get_env("PORT") || "4000")]}
    ]

    opts = [strategy: :one_for_one, name: CuratomOrchestrator.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
