# BUILD CONTAINER

FROM rust:1.65 as build

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN USER=root cargo new --bin anifunnel

# Build dependencies separately for layer caching.
WORKDIR ./anifunnel
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release

# Clean the temporary project.
RUN rm src/*.rs ./target/release/deps/anifunnel*

ADD . ./
RUN cargo build --release --verbose


# RUNTIME CONTAINER

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
&& rm -rf /var/lib/apt/lists/*

COPY --from=build /anifunnel/target/release/anifunnel .

ENV ANILIST_TOKEN= \
    ANIFUNNEL_ADDRESS=0.0.0.0 \
    ANIFUNNEL_PORT=8000

EXPOSE 8000

CMD ["./anifunnel"]
