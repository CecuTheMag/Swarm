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

# `url` picks another coordinator, e.g. ws://localhost:7780/ws/worker for `just stack`.
# Run N simulated workers until the queue is empty.
sim n="4" url="ws://localhost:7700/ws/worker":
    cargo run --release -p sim-worker -- --url {{url}} --workers {{n}} --exit-when-idle

# Submit a pi job to the local coordinator.
job units="64" samples="16777216" seed="42":
    curl -sf -X POST localhost:7700/api/jobs -H 'content-type: application/json' \
      -d '{"template":"pi","params":{"units":{{units}},"samples_per_unit":{{samples}},"seed":{{seed}}}}'

# `port` 7780 shares the stack of `just stack` instead of the dev server.
# Serve the web app over HTTPS to devices on the tailnet (WebGPU needs a secure context).
share port="7701":
    tailscale serve --bg {{port}}
    @tailscale serve status

# Submitting jobs needs the password only where the coordinator has one (`just stack`
# does; the dev coordinator opens the public network with SWARM_OPEN_PUBLIC=1 in .env).
# Run `just unshare` after.
# Serve the web app over HTTPS on the public internet, for devices without Tailscale.
share-public port="7701":
    tailscale funnel --bg {{port}}
    @tailscale funnel status

# Stop serving the web app over Tailscale.
unshare:
    tailscale serve reset

# Copy the working tree to a Docker host and (re)start Swarm there on port 7780.
# `server` is user@host; set SWARM_SERVER to skip typing it. Secrets stay on the
# server in ~/swarm.env (the first deploy creates it with a random submit password).
deploy server=env_var_or_default("SWARM_SERVER", ""):
    @test -n "{{server}}" || { echo "usage: just deploy user@host (or set SWARM_SERVER)"; exit 1; }
    # Only files that exist (--cached also lists deleted ones), with flags both GNU tar
    # and macOS bsdtar take; COPYFILE_DISABLE keeps macOS ._* metadata files out.
    git ls-files -z --cached --others --exclude-standard \
      | xargs -0 sh -c 'for f; do if [ -e "$f" ] || [ -L "$f" ]; then printf "%s\0" "$f"; fi; done' sh \
      | COPYFILE_DISABLE=1 tar --null --no-xattrs -T - -cf - \
      | ssh {{server}} 'rm -rf ~/swarm && mkdir -p ~/swarm && tar -xf - -C ~/swarm'
    ssh {{server}} 'test -f ~/swarm.env || (umask 077 && echo "SWARM_SUBMIT_TOKEN=$(openssl rand -hex 12)" > ~/swarm.env)'
    ssh {{server}} 'cd ~/swarm && docker compose --env-file ~/swarm.env -f deploy/compose.server.yaml up -d --build --remove-orphans'

# `--test` fetches the tiny test model the browser tests use instead.
# Model files of the catalog (spec 0017) into models/, each checked against its SHA-256.
models *args:
    python3 scripts/models.py {{args}}

# Only what changed is sent; run `just models` first.
# Copy the model files to the server (the coordinator serves them from ~/swarm-models).
# The files are made world-readable: the coordinator container runs as `nobody`, and the
# downloaded files are mode 600. If Docker created ~/swarm-models as root before the first
# copy, remove that empty folder on the server first (`rmdir ~/swarm-models`), then
# restart the coordinator so it sees the new folder.
deploy-models server=env_var_or_default("SWARM_SERVER", ""):
    @test -n "{{server}}" || { echo "usage: just deploy-models user@host (or set SWARM_SERVER)"; exit 1; }
    rsync -a --delete --exclude 'hf-internal-testing/' models/ {{server}}:swarm-models/
    ssh {{server}} 'chmod -R a+rX ~/swarm-models'

# The platform admin on the deployed server (spec 0010): prints a new random password
# once, on the server's terminal only. Run it again to reset a forgotten password.
admin-create email server=env_var_or_default("SWARM_SERVER", ""):
    @test -n "{{server}}" || { echo "usage: just admin-create EMAIL user@host (or set SWARM_SERVER)"; exit 1; }
    ssh -t {{server}} 'cd ~/swarm && docker compose --env-file ~/swarm.env -f deploy/compose.server.yaml exec coordinator coordinator admin-create {{email}}'

# Copies the documentation, the presentation and the brochure PDFs to the repository root.
materials:
    bash scripts/materials.sh

# One test account per role for the jury (owner, admin and viewer of a network), made
# through the server's API and filled into the documentation outside the repository
# (scripts/test-accounts.sh explains what it needs).
test-accounts:
    bash scripts/test-accounts.sh

# The documentation PDF with the authors' details and the test logins filled in, built
# outside the repository (scripts/build-documentation.sh says where it reads them from).
documentation:
    bash scripts/build-documentation.sh

# The public copy the repository keeps (no phone numbers, no passwords); `just materials` after.
documentation-public:
    PUBLIC=1 OUT_DIR="$(mktemp -d)" bash scripts/build-documentation.sh

# Public HTTPS for the deployed server through Tailscale Funnel. The first run prints a
# login link that adds the server to your tailnet; run it again to get the address.
public server=env_var_or_default("SWARM_SERVER", ""):
    ssh {{server}} 'sh ~/swarm/deploy/tailscale/public.sh'

# The fallback for a demo when the server or its connection is down (docs/competition/demo-day.md).
# Kept apart from the dev services; the submit password is in ~/swarm.env, created on
# first use.
# The whole server stack on this machine, on port 7780.
stack:
    test -f ~/swarm.env || (umask 077 && echo "SWARM_SUBMIT_TOKEN=$(openssl rand -hex 12)" > ~/swarm.env)
    docker compose -p swarm-stack --env-file ~/swarm.env -f deploy/compose.server.yaml up -d --build --remove-orphans

# Stop the stack of `just stack` (its data stays).
stack-down:
    docker compose -p swarm-stack --env-file ~/swarm.env -f deploy/compose.server.yaml down

# Simulated CPU workers on the deployed server (0 stops them).
fleet n="4" server=env_var_or_default("SWARM_SERVER", ""):
    ssh {{server}} 'cd ~/swarm && SIM_WORKERS={{n}} docker compose --env-file ~/swarm.env -f deploy/compose.server.yaml --profile fleet up -d sim {{ if n == "0" { "--scale sim=0" } else { "" } }}'

# For the live demo (docs/competition/demo-day.md); it stops by itself once blocked.
# A device that makes up every result joins the server's grid and is caught (spec 0004).
cheater server=env_var_or_default("SWARM_SERVER", ""):
    ssh {{server}} 'docker run --rm --network swarm_default --label com.centurylinklabs.watchtower.enable=false swarm-coordinator sim-worker --url ws://coordinator:7700/ws/worker --cheat-rate 1 --delay-ms 1000'

# Per-device speed of completed pi units.
bench-report:
    docker compose exec -T postgres psql -U swarm swarm < deploy/sql/benchmarks.sql

# Everything CI checks, except browser tests.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo nextest run --workspace --all-features
    cd web && pnpm check && pnpm lint && pnpm test:unit --run

# Build the example programs (spec 0012) and check each as an upload would.
programs:
    cd examples/programs && cargo build --release
    for module in examples/programs/target/wasm32-unknown-unknown/release/*.wasm; do \
      cargo run -q -p programs --features cli -- check "$module"; done

# Check a program and run one unit of it, e.g. `just run-program primes.wasm --index 3`.
run-program module *args:
    cargo run -q --release -p programs --features cli -- run {{module}} {{args}}

# Browser tests against the real coordinator: Chromium with WebGPU (SwiftShader) and
# without, and Firefox. WebKit needs Ubuntu's libraries: see `e2e-webkit`.
e2e:
    cd web && pnpm exec playwright test --project webgpu --project cpu --project firefox

# The browser tests in WebKit (Safari's engine). macOS runs WebKit natively; on Linux it
# needs Ubuntu's libraries, so the tests connect to Playwright's container.
[macos]
e2e-webkit:
    cd web && pnpm exec playwright test --project webkit

[linux]
e2e-webkit:
    docker rm -f swarm-pw-webkit >/dev/null 2>&1 || true
    docker run -d --name swarm-pw-webkit --network host --init \
      mcr.microsoft.com/playwright:v1.64.1-noble \
      npx -y playwright@1.64.1 run-server --port 7790 --host 127.0.0.1
    sleep 10
    cd web && PW_TEST_CONNECT_WS_ENDPOINT=ws://127.0.0.1:7790/ pnpm exec playwright test --project webkit; \
      status=$?; docker rm -f swarm-pw-webkit >/dev/null; exit $status
