# Shelfie
[![CI](https://github.com/mishl-dev/shelfie/actions/workflows/ci.yml/badge.svg)](https://github.com/mishl-dev/shelfie/actions/workflows/ci.yml)  

A self-hosted API that provides a unified interface for accessing ebooks. designed for integration with readers like Foliate.

## Run with Docker Compose

```bash
docker compose up --build
````

Access:

* OPDS server: `http://localhost:7451/opds`
* OpenSearch: `http://localhost:7451/opds/opensearch.xml`
* Explore feeds: `http://localhost:7451/opds/explore`
* Health: `http://localhost:7451/healthz`
* Metrics: `http://localhost:7451/metrics`

Stop with:

```bash
docker compose down
```

## Run from CLI

```bash
cargo run -- serve --log-style pretty
```

Inspect runtime config:

```bash
cargo run -- print-config
```

## License

Shelfie is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
