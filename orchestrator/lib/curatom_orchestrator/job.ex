defmodule CuratomOrchestrator.Job do
  @moduledoc "A unit of work handed off by the Worker."
  defstruct [
    :job_id,
    :approval_id,
    :intent_id,
    :requester_id,
    :actions
  ]

  def from_map(m) do
    %__MODULE__{
      job_id: m["job_id"],
      approval_id: m["approval_id"],
      intent_id: m["intent_id"],
      requester_id: m["requester_id"],
      actions:
        Enum.map(m["actions"] || [], fn a ->
          %{
            resource: a["resource"],
            operation: a["operation"],
            attestation: a["attestation"]
          }
        end)
    }
  end
end
