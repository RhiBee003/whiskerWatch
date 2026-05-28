FROM rust:1.85-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY static ./static
COPY templates ./templates
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/whiskerWatchWeb .
COPY static ./static
COPY templates ./templates
RUN mkdir -p data
ENV PORT=8080
EXPOSE 8080
CMD ["./whiskerWatchWeb"]
