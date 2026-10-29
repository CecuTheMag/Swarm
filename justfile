# Common development tasks. Run `just` to list them.

default:
    @just --list

# Start PostgreSQL and Redis.
services:
    docker compose up -d

# Start monitoring and feature-flag services as well.
services-ops:
    docker compose --profile ops up -d

# Run the coordinator (reads .env).
coordinator:
    cargo run -p coordinator

# Run the web app dev server on :7701 (proxies /api and /ws to :7700).
web:
    cd web && pnpm dev

# Run N simulated workers until the queue is empty.
sim n="4":
    cargo run --release -p sim-worker -- --workers {{n}} --exit-when-idle

# Submit a pi job to the local coordinator.
job units="64" samples="16777216" seed="42":
    curl -sf -X POST localhost:7700/api/jobs -H 'content-type: application/json' \
      -d '{"template":"pi","params":{"units":{{units}},"samples_per_unit":{{samples}},"seed":{{seed}}}}'

# Serve the web app over HTTPS to devices on the tailnet (WebGPU needs a secure context).
share:
    tailscale serve --bg 7701
    @tailscale serve status

# Serve the web app over HTTPS on the public internet, for devices without Tailscale.
# There is no auth yet: anyone with the URL can join or submit jobs. Run `just unshare` after.
share-public:
    tailscale funnel --bg 7701
    @tailscale funnel status

# Stop serving the web app over Tailscale.
unshare:
    tailscale serve reset

# Copy the working tree to a Docker host and (re)start Swarm there on port 7780.
# `server` is user@host; set SWARM_SERVER to skip typing it.
deploy server=env_var_or_default("SWARM_SERVER", ""):
    @test -n "{{server}}" || { echo "usage: just deploy user@host (or set SWARM_SERVER)"; exit 1; }
    git ls-files -z --cached --others --exclude-standard \
      | tar --null --files-from=- --ignore-failed-read -cf - \
      | ssh {{server}} 'rm -rf ~/swarm && mkdir -p ~/swarm && tar -xf - -C ~/swarm'
    ssh {{server}} 'cd ~/swarm && docker compose -f deploy/compose.server.yaml up -d --build --remove-orphans'

# Public HTTPS for the deployed server through Tailscale Funnel. The first run prints a
# login link that adds the server to your tailnet; run it again to get the address.
public server=env_var_or_default("SWARM_SERVER", ""):
    ssh {{server}} 'sh ~/swarm/deploy/tailscale/public.sh'

# Simulated CPU workers on the deployed server (0 stops them).
fleet n="4" server=env_var_or_default("SWARM_SERVER", ""):
    ssh {{server}} 'cd ~/swarm && SIM_WORKERS={{n}} docker compose -f deploy/compose.server.yaml --profile fleet up -d sim {{ if n == "0" { "--scale sim=0" } else { "" } }}'

# Per-device speed of completed pi units.
bench-report:
    docker compose exec -T postgres psql -U swarm swarm < deploy/sql/benchmarks.sql

# Everything CI checks, except browser tests.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo nextest run --workspace
    cd web && pnpm check && pnpm lint && pnpm test:unit --run

# Browser tests against the real coordinator (WebGPU via SwiftShader).
e2e:
    cd web && pnpm exec playwright test
