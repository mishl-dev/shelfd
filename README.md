<p align="center">
  <img src="assets/logo.svg" alt="shelfd logo" width="132">
</p>

<h1 align="center">shelfd</h1>

<p align="center">
  A small self-hosted OPDS server for ebook archives.
</p>

<p align="center">
  Browse by subject, search from reader apps, and fill in covers and metadata automatically.
</p>

## What It Does

- turns an ebook archive into an OPDS catalog
- gives you an explore-first browsing feed instead of search-only UX
- still supports OPDS search for clients like Foliate
- pulls metadata and covers from Open Library when available
- generates fallback covers when nothing better exists

## Quick Start

Run with `just`:

```bash
just up
```

Or without `just`:

```bash
docker compose up --build
```

Then open:

- `http://localhost:7451/opds`

Stop:

```bash
just down
```

Or:

```bash
docker compose down
```

<details>
  <summary><strong>Config</strong></summary>

  <br>

  The defaults are enough to get started, but these are the settings you will most likely care about:

  - `ARCHIVE_URLS`: comma-separated archive base URLs for round-robin racing
  - `ARCHIVE_BASE`: fallback single archive URL (used if `ARCHIVE_URLS` is not set)
  - `ARCHIVE_NAME`: display name shown in feeds
  - `APP_NAME`: app name shown to OPDS clients
  - `PUBLIC_BASE_URL`: public base URL for generated links
  - `DATABASE_URL`: SQLite database location
  - `RUST_LOG`: log level and filters
</details>

<details>
  <summary><strong>Endpoints</strong></summary>

  <br>

  - `GET /opds`
  - `GET /opds/search?q=...`
  - `GET /healthz`
</details>

<details>
  <summary><strong>Dev</strong></summary>

  <br>

  ```bash
  just up
  just test
  just lint
  just fmt
  ```
</details>

## License

MIT. See [LICENSE](LICENSE).
