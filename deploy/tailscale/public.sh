#!/bin/sh
# Public HTTPS for the deployed stack through Tailscale Funnel (`just public`).
#
# The tailscale image's own interactive login gives up after 60 s and restarts with a
# new link, so the first login runs in a one-off container without a time limit and
# saves the node's state for the `tailscale` service. Run it again after approving the
# link: it then starts the service and prints the public address.
set -eu
cd "$(dirname "$0")/../.."

compose="docker compose --env-file $HOME/swarm.env -f deploy/compose.server.yaml --profile public"
service=swarm-tailscale-1
login=swarm-tailscale-login
sock=--socket=/tmp/tailscaled.sock

if docker exec "$service" tailscale status >/dev/null 2>&1; then
    docker exec "$service" tailscale funnel status
    exit 0
fi

if [ -n "$(docker ps -q --filter "name=^$login\$")" ]; then
    if docker exec "$login" tailscale $sock status >/dev/null 2>&1; then
        docker rm -f "$login" >/dev/null
        $compose up -d tailscale
        sleep 5
        echo "Logged in. The certificate can take a minute on first start."
        docker exec "$service" tailscale funnel status
        exit 0
    fi
else
    $compose stop tailscale >/dev/null 2>&1 || true
    docker run -d --name "$login" -v swarm_tsstate:/var/lib/tailscale tailscale/tailscale:stable \
        tailscaled --tun=userspace-networking --statedir=/var/lib/tailscale $sock >/dev/null
    sleep 2
    docker exec -d "$login" tailscale $sock up --hostname="${SWARM_PUBLIC_NAME:-swarm}" --timeout=0
fi

for _ in $(seq 1 60); do
    url=$(docker logs "$login" 2>&1 | grep -o 'https://login.tailscale.com/a/[0-9a-z]*' | tail -1)
    if [ -n "$url" ]; then
        echo "Open $url to add the server to your tailnet, then run just public again."
        exit 0
    fi
    sleep 1
done
echo "No login link yet; see: docker logs $login" >&2
exit 1
