FROM rust:1.88-slim AS builder

# install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --release

# runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ffmpeg \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/video-service .

EXPOSE 3000
CMD ["./video-service"]