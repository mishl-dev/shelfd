<p align="center">
  <img src="assets/logo.svg" alt="shelfie logo">
</p>

# shelfie

Self-hosted OPDS bridge for ebook archives.

- OPDS root at `/opds`
- explore feed as the default browsing flow
- OPDS search still available for clients like Foliate
- Open Library metadata and cover enrichment
- generated fallback covers when no real cover exists

## Run

With Docker Compose:

```bash
docker compose up --build
```

Endpoints:

- `http://localhost:7451/opds`
- `http://localhost:7451/opds/explore`
- `http://localhost:7451/opds/opensearch.xml`
- `http://localhost:7451/healthz`
- `http://localhost:7451/metrics`

Stop:

```bash
docker compose down
```

Locally:

```bash
cargo run -- serve
```

Pretty logs:

```bash
cargo run -- serve --log-style pretty
```

Print effective config:

```bash
cargo run -- print-config
```

## Config

Main env vars:

- `DATABASE_URL`
- `BIND_ADDR`
- `ARCHIVE_BASE`
- `ARCHIVE_NAME`
- `APP_NAME`
- `METADATA_BASE_URL`
- `FLARESOLVERR_URL`
- `FLARESOLVERR_SESSION`
- `PUBLIC_BASE_URL`
- `RUST_LOG`
- `LOG_STYLE`

Performance and cache:

- `SEARCH_CACHE_TTL_SECS`
- `BOOK_CACHE_TTL_SECS`
- `LINK_CACHE_TTL_SECS`
- `LINK_FAILURE_TTL_SECS`
- `EXPLORE_CACHE_TTL_SECS`
- `COVER_NEGATIVE_TTL_SECS`
- `SEARCH_RESULT_LIMIT`
- `EXPLORE_PAGE_SIZE`
- `COVER_LOOKUP_LIMIT`
- `INLINE_INFO_CONCURRENCY`
- `COVER_LOOKUP_CONCURRENCY`
- `SEARCH_PREWARM_COUNT`
- `UPSTREAM_RETRY_ATTEMPTS`
- `UPSTREAM_RETRY_BACKOFF_MS`
- `CACHE_CLEANUP_INTERVAL_SECS`
- `EXPLORE_SUBJECTS`

## Endpoints

- `GET /opds`
- `GET /opds/explore`
- `GET /opds/explore/top`
- `GET /opds/explore/subject/{subject}`
- `GET /opds/search?q=...`
- `GET /opds/opensearch.xml`
- `GET /opds/cover/{md5}`
- `GET /opds/download/{md5}`
- `GET /healthz`
- `GET /readyz`
- `GET /metrics`

## Dev

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

## License

MIT. See [LICENSE](LICENSE).
