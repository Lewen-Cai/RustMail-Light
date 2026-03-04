FROM rust:latest AS builder
WORKDIR /app

COPY Cargo.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN cargo build --release -p rustmaild

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --uid 10001 --create-home --home-dir /var/lib/rustmail rustmail

WORKDIR /opt/rustmail

COPY --from=builder /app/target/release/rustmaild /usr/local/bin/rustmaild

RUN mkdir -p /etc/rustmail /var/lib/rustmail \
    && chown -R rustmail:rustmail /etc/rustmail /var/lib/rustmail

USER rustmail

EXPOSE 25 465 587 993 995 8080

CMD ["rustmaild", "--config", "/etc/rustmail/rustmaild.toml"]
