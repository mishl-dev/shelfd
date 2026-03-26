FROM rust:1.94-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release
RUN mkdir -p /data

FROM gcr.io/distroless/cc

COPY --from=builder /data /data
COPY --from=builder /app/target/release/shelfd /usr/local/bin/shelfd

ENV DATABASE_URL=sqlite:///data/opds.db?mode=rwc
ENV BIND_ADDR=0.0.0.0:7451

EXPOSE 7451

CMD ["/usr/local/bin/shelfd"]
