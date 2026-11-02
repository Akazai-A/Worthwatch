# Stage 1: build the UI
FROM node:22-alpine AS ui
WORKDIR /ui
COPY ui/package.json ui/pnpm-lock.yaml* ./
RUN corepack enable && pnpm install --frozen-lockfile || pnpm install
COPY ui/ .
RUN pnpm build

# Stage 2: build the server with the UI embedded
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates ./crates
COPY server ./server
COPY data/ledger.example.yaml ./data/ledger.example.yaml
COPY --from=ui /ui/dist ./ui/dist
RUN cargo build --release --bin worthwatch

# Stage 3: minimal runtime
FROM gcr.io/distroless/cc-debian12
COPY --from=build /src/target/release/worthwatch /worthwatch
ENV WORTHWATCH_DATA=/data WORTHWATCH_BIND=0.0.0.0:7777
VOLUME /data
EXPOSE 7777
ENTRYPOINT ["/worthwatch"]
