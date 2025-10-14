ARG VITE_APP_VERSION VITE_APP_BUILD

# BUILD CONTAINER

FROM rust:1.90 AS build

ARG VITE_APP_VERSION VITE_APP_BUILD

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && \
    apt-get install -y ca-certificates npm \
    && rm -rf /var/lib/apt/lists/*

RUN USER=root cargo new --bin anifunnel

# Build dependencies separately for layer caching.
WORKDIR /anifunnel
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release

# Clean the temporary project.
RUN rm src/*.rs ./target/release/deps/anifunnel*

# Install npm dependencies separately for layer caching.
WORKDIR /anifunnel/frontend
COPY ./frontend/package.json ./package.json
COPY ./frontend/package-lock.json ./package-lock.json
RUN npm install

WORKDIR /anifunnel
ADD . ./

# Build the front-end for the server build.
WORKDIR /anifunnel/frontend
RUN npm run build

# Build the server.
WORKDIR /anifunnel
RUN cargo build --release --verbose


# RUNTIME CONTAINER

FROM debian:trixie-slim

COPY --from=build /etc/ssl/certs/ /etc/ssl/certs/

COPY --from=build /anifunnel/target/release/anifunnel .

ENV ANIFUNNEL_ADDRESS=0.0.0.0 \
    ANIFUNNEL_DATABASE=/db/anifunnel.db \
    ANIFUNNEL_PORT=8000

EXPOSE 8000

CMD ["./anifunnel"]
