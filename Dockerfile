FROM rust:latest

# Install musl-tools (needed for statically linking)
RUN apt-get update && apt-get install -y musl-tools

# Copy the application files
COPY . .

# Build the application
RUN cargo build --release

# Set the command to run the application
CMD ["./target/release/docker-app"]