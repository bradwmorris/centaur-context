FROM node:22.14.0-alpine@sha256:9bef0ef1e268f60627da9ba7d7605e8831d5b56ad07487d24d1aa386336d1944 AS web-build
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.91.0-bookworm@sha256:e187887ec511b3d93e45c0231d2f0fd59f1347526c58aa86343aa83c74f3e1a9 AS rust-build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY migrations/ ./migrations/
RUN cargo build --release --locked

FROM debian:bookworm-slim@sha256:88200866dfff7ea7f5cbcb6ec7c8a701889efe6fe859fe64d6990e4b07ea4171
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 centaur-os \
    && useradd --uid 10001 --gid 10001 --no-create-home centaur-os
COPY --from=rust-build /build/target/release/centaur-os /usr/local/bin/centaur-os
COPY --from=web-build /build/web/dist /app/web
LABEL org.opencontainers.image.title="Centaur OS" \
      org.opencontainers.image.description="Local-first shared context layer for Centaur" \
      org.opencontainers.image.version="0.1.0"
USER 10001:10001
ENV STATIC_DIR=/app/web \
    HUMAN_ADDR=0.0.0.0:8080 \
    AGENT_ADDR=0.0.0.0:8081 \
    INGEST_ADDR=0.0.0.0:8082 \
    CURATOR_ADDR=0.0.0.0:8083
EXPOSE 8080 8081 8082 8083
ENTRYPOINT ["/usr/local/bin/centaur-os"]
