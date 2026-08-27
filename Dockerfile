FROM node:22.14.0-alpine AS web-build
WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.91.0-bookworm AS rust-build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY migrations/ ./migrations/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 centaur-os \
    && useradd --uid 10001 --gid 10001 --no-create-home centaur-os
COPY --from=rust-build /build/target/release/centaur-os /usr/local/bin/centaur-os
COPY --from=web-build /build/web/dist /app/web
USER 10001:10001
ENV STATIC_DIR=/app/web \
    HUMAN_ADDR=0.0.0.0:8080 \
    AGENT_ADDR=0.0.0.0:8081
EXPOSE 8080 8081
ENTRYPOINT ["/usr/local/bin/centaur-os"]
