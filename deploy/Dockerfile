# Worthwatch in a container: built from source, runs as a non-root user, needs nothing
# writable but /data. Run it with deploy/container/run.sh, which adds the locked-down flags
# (read-only, no capabilities, port on 127.0.0.1 only). See docs/run/security.md.

# Stage 1: build the UI. The lockfile is the contract: no silent upgrades.
FROM node:26-alpine AS ui
WORKDIR /ui
COPY ui/package.json ui/pnpm-lock.yaml ui/pnpm-workspace.yaml ./
# Node 25+ no longer ships corepack, so install the pnpm that package.json pins.
RUN npm install --global "$(node -p 'require("./package.json").packageManager')" \
    && pnpm install --frozen-lockfile
COPY ui/ .
RUN pnpm build

# Stage 2: build the server with the UI embedded, from Cargo.lock exactly.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY server ./server
COPY data/ledger.example.yaml ./data/ledger.example.yaml
COPY --from=ui /ui/dist ./ui/dist
RUN cargo build --release --locked --bin worthwatch && mkdir -p /out/data

# Stage 3: a minimal runtime with no shell and no package manager, as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/worthwatch /worthwatch
COPY --from=build --chown=65532:65532 /out/data /data
USER 65532:65532
ENV WORTHWATCH_DATA=/data WORTHWATCH_BIND=0.0.0.0:7777 WORTHWATCH_IN_CONTAINER=1
VOLUME /data
EXPOSE 7777
ENTRYPOINT ["/worthwatch"]
