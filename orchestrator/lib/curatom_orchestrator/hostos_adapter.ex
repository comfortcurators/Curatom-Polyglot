defmodule CuratomOrchestrator.HostOSAdapter do
  @callback execute(%{
              requester_id: String.t(),
              resource: String.t(),
              operation: String.t(),
              grant_id: String.t()
            }) :: {:ok, map()} | {:error, term()}
end
