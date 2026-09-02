# Multi-stage build for tag1bot: a long-running Rust Slack bot (Tokio async)
# connected via Socket Mode (outbound-only WebSocket, no inbound ports/ingress
# needed). Provides karma tracking, "seen" tracking, currency conversion
# alerts, and ChatGPT integration.
#
# tag1bot depends on a crate pulled from a personal GitHub fork
# (github.com/jeremyandrews/slack-rust), pinned via the committed Cargo.lock.
# The build stage needs outbound git/https access to github.com to fetch it.

FROM rust:1-bookworm AS builder

WORKDIR /build

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked

# Runtime image: no Rust toolchain, just the release binary, CA certs for
# outbound HTTPS to Slack/XE.com/OpenAI, and libcurl4 -- the surf HTTP
# client pulls in curl-sys, which dynamically links libcurl.so.4 (confirmed
# via `docker run`: the binary fails to start without it).
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libcurl4 \
    && rm -rf /var/lib/apt/lists/*

# tag1bot opens ./state.sqlite3 relative to its working directory
# (see src/db.rs: const DATABASE_FILE: &str = "./state.sqlite3") -- WORKDIR
# is set to the Lagoon persistent-volume mount path so the database lands on
# durable storage instead of ephemeral container storage. See .lagoon.yml.
RUN useradd --create-home --shell /usr/sbin/nologin tag1bot
WORKDIR /data
COPY --from=builder /build/target/release/tag1bot /usr/local/bin/tag1bot
RUN chown tag1bot:tag1bot /data

USER tag1bot

ENTRYPOINT ["/usr/local/bin/tag1bot"]
