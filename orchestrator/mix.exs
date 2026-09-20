defmodule CuratomOrchestrator.MixProject do
  use Mix.Project

  def project do
    [
      app: :curatom_orchestrator,
      version: "0.4.0",
      elixir: "~> 1.16",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      releases: [curatom_orchestrator: [include_executables_for: [:unix]]]
    ]
  end

  def application do
    [
      extra_applications: [:logger, :crypto],
      mod: {CuratomOrchestrator.Application, []}
    ]
  end

  defp deps do
    [
      {:plug_cowboy, "~> 2.7"},
      {:jason, "~> 1.4"},
      {:finch, "~> 0.18"},
      {:telemetry, "~> 1.2"}
    ]
  end
end
