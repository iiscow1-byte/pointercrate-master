FROM rust:latest AS builder
WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock ./
COPY pointercrate-*/Cargo.toml ./

RUN mkdir -p $(ls -d pointercrate-*) 2>/dev/null; true

RUN cargo fetch

COPY . .
RUN cargo build --release -p pointercrate-example