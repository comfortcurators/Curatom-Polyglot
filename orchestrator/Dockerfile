FROM hexpm/elixir:1.17.3-erlang-27.2-debian-bookworm-20241223-slim AS build

RUN apt-get update && apt-get install -y build-essential git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
ENV MIX_ENV=prod \
    MIX_HOME=/app/.mix \
    MIX_REBAR3=/app/vendor/rebar3

# Context is this directory (orchestrator/), not the repo root.
# wrangler image = "../../orchestrator/Dockerfile" from workers/orchestrator/.
# Vendored deps/ is the resolved tree. Do not run mix deps.get — that is
# the step that talks to a Hex registry the image does not have.
COPY mix.exs mix.lock ./
COPY vendor/hex.ez vendor/rebar3 /app/vendor/
RUN chmod +x /app/vendor/rebar3 \
    && mix archive.install --force /app/vendor/hex.ez

COPY . ./
RUN mix compile && mix release

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
