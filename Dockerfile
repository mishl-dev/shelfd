FROM rust:1.94-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Deps layer — only busts when Cargo.toml/Cargo.lock change
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/shelfie /usr/local/bin/shelfie

ENV DATABASE_URL=sqlite:///data/opds.db?mode=rwc
ENV BIND_ADDR=0.0.0.0:7070

RUN mkdir -p /data

EXPOSE 7070

CMD ["shelfie"]