defmodule CuratomOrchestrator.Job do
  @moduledoc "A unit of work handed off by the Worker."
  defstruct [
    :job_id,
    :approval_id,
    :intent_id,
    :requester_id,
    # Which Worker Durable Object this job's owner lives in. Carried
    # verbatim from the handoff envelope so the outcome callback can be
    # routed back to the right instance -- before this field existed,
    # every outcome POST landed on the founder's DO regardless of whose
    # knock produced it. See `workers/api/src/lib.rs`'s `fetch()` and its
    # `/internal/outcome` routing block.
    :owner_id,
    :actions
  ]

  def from_map(m) do
    %__MODULE__{
      job_id: m["job_id"],
      approval_id: m["approval_id"],
      intent_id: m["intent_id"],
      requester_id: m["requester_id"],
      owner_id: m["owner_id"],
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
