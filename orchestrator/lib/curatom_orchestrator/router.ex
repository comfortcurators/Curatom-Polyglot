defmodule CuratomOrchestrator.Router do
  use Plug.Router
  require Logger

  plug :match

  plug Plug.Parsers,
    parsers: [:json],
    pass: ["application/json"],
    json_decoder: Jason,
    body_reader: {__MODULE__, :cache_body, []}

  plug :verify_hmac
  plug :dispatch

  get "/health" do
    send_resp(conn, 200, Jason.encode!(%{status: "ok"}))
  end

  post "/v1/jobs" do
    job = CuratomOrchestrator.Job.from_map(conn.body_params)
    :ok = CuratomOrchestrator.JobQueue.submit(job)
    send_resp(conn, 202, Jason.encode!(%{accepted: job.job_id}))
  end

  match _ do
    send_resp(conn, 404, Jason.encode!(%{error: "not found"}))
  end

  @doc "Read the body once so HMAC and Plug.Parsers see the same bytes."
  def cache_body(conn, opts) do
    {:ok, body, conn} = Plug.Conn.read_body(conn, opts)
    conn = Plug.Conn.put_private(conn, :raw_body, body)
    {:ok, body, conn}
  end

  defp verify_hmac(conn, _opts) do
    if conn.method == "GET" and conn.request_path == "/health" do
      conn
    else
      key = System.fetch_env!("CURATOM_HMAC_KEY") |> CuratomOrchestrator.Attestation.key_bytes()
      provided = get_req_header(conn, "x-curatom-hmac") |> List.first()
      body = conn.private[:raw_body] || ""
      expected = :crypto.mac(:hmac, :sha256, key, body) |> Base.encode16(case: :lower)

      if provided && Plug.Crypto.secure_compare(provided, expected) do
        conn
      else
        conn
        |> Plug.Conn.send_resp(401, Jason.encode!(%{error: "bad hmac"}))
        |> Plug.Conn.halt()
      end
    end
  end
end
