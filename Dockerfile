FROM rust:latest AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p pointercrate-example

FROM rust:latest
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/pointercrate-example ./server
COPY --from=builder /app/pointercrate-core-pages/static ./pointercrate-core-pages/static
COPY --from=builder /app/pointercrate-demonlist-pages/static ./pointercrate-demonlist-pages/static
COPY --from=builder /app/pointercrate-user-pages/static ./pointercrate-user-pages/static
COPY --from=builder /app/pointercrate-example/static ./pointercrate-example/static
CMD ROCKET_PORT=$PORT ROCKET_ADDRESS=0.0.0.0 ./server