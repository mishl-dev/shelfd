set shell := ["bash", "-cu"]

default:
    #!/usr/bin/env bash
    cmd=$(just --list --unsorted \
        | tail -n +2 \
        | sed 's/^[[:space:]]*//' \
        | grep -Ev '^(menu|default)( |$)' \
        | fzf \
            --prompt="just > " \
            --height=40% \
            --layout=reverse \
            --border \
            --preview "just --show {1}" \
            --preview-window=right:60% \
        | cut -d' ' -f1)
    [ -n "$cmd" ] && just "$cmd"

# Start services (build if needed)
up:
    docker compose up --build

# Stop services
down:
    docker compose down

# Watch for file changes
watch: 
    docker compose up --build --watch

# Restart containers (keeps cache)
rebuild:
    just down
    just up

# Reset volumes + remove local images
reset:
    docker compose down -v --rmi local
    docker compose up --build

# Follow logs
logs:
    docker compose logs -f

# Run tests
test *args="":
    cargo test {{args}}

# Format code
fmt:
    cargo fmt

# Run linter
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Format and lint
check: fmt lint

# Auto-fix clippy warnings and format
fix:
    cargo clippy --fix --allow-dirty
    cargo fmt

# Build project
build:
    cargo build
