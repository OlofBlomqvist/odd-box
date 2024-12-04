# Stage 1: Build the Rust binary
FROM rust:latest AS builder

# Set the working directory inside the container
WORKDIR /usr/src/odd-box

# Copy the entire project into the container
COPY . .

# Build the project in release mode
RUN rustup target add x86_64-unknown-linux-musl 
RUN cargo build --release --target=x86_64-unknown-linux-musl

# Stage 2: Create a minimal runtime image
FROM debian:buster-slim

# Set the working directory for the runtime container
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/odd-box/target/release/odd-box /app/odd-box

# Ensure the binary has execute permissions
RUN chmod +x /app/odd-box

# Specify the default command to run the application
CMD ["/app/odd-box"]