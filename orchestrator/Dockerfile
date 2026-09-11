FROM hexpm/elixir:1.17.3-erlang-27.2-debian-bookworm-20241223-slim AS build

RUN apt-get update && apt-get install -y build-essential git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
ENV MIX_ENV=prod

# Context is this directory (orchestrator/), not the repo root.
# wrangler image = "../../orchestrator/Dockerfile" from workers/orchestrator/.
COPY mix.exs mix.lock ./
RUN mix local.hex --force && mix local.rebar --force && mix deps.get --only prod

COPY . ./
RUN mix deps.compile && mix compile && mix release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libstdc++6 openssl libncurses6 locales ca-certificates \
    && rm -rf /var/lib/apt/lists/*
ENV LANG=C.UTF-8

WORKDIR /app
COPY --from=build /app/_build/prod/rel/curatom_orchestrator ./

ENV PORT=4000
EXPOSE 4000
CMD ["bin/curatom_orchestrator", "start"]
