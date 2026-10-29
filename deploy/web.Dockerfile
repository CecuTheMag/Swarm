# The SvelteKit site (adapter-node). Build context: repository root.
#   docker build -f deploy/web.Dockerfile .
# The build compiles crates/kernels-wasm (the CPU fallback), so it needs Rust and Node.

FROM rust:1.95-slim AS build
COPY --from=node:24-slim /usr/local/ /usr/local/
RUN npm install --global pnpm@10.33.2
WORKDIR /src
# The toolchain rust-toolchain.toml pins, in its own layer so code changes reuse it.
COPY rust-toolchain.toml .
RUN rustup toolchain install
COPY . .
WORKDIR /src/web
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    pnpm build
# Runtime needs only production dependencies; `prepare` (svelte-kit sync) is a dev tool.
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    rm -rf node_modules && pnpm install --frozen-lockfile --prod --ignore-scripts

FROM node:24-slim
WORKDIR /app
COPY --from=build /src/web/package.json ./
COPY --from=build /src/web/node_modules ./node_modules
COPY --from=build /src/web/build ./build
USER node
# Behind the Caddy proxy in compose.server.yaml.
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000 \
    PROTOCOL_HEADER=x-forwarded-proto HOST_HEADER=x-forwarded-host
EXPOSE 3000
CMD ["node", "build"]
