ARG VITE_APP_VERSION
ARG VITE_APP_BUILD

# BUILD CONTAINER

FROM rust:1.93 AS build

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN apt-get update && \
    apt-get install -y npm \
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

ARG VITE_APP_BUILD
ARG VITE_APP_VERSION

# Build the front-end for the server build.
WORKDIR /anifunnel/frontend
RUN npm run build

# Build the server.
WORKDIR /anifunnel
RUN cargo build --release --verbose


# RUNTIME CONTAINER

FROM gcr.io/distroless/cc-debian13

WORKDIR /db

COPY --from=build /anifunnel/target/release/anifunnel /anifunnel

ENV ANIFUNNEL_ADDRESS=0.0.0.0
ENV ANIFUNNEL_DATABASE=/db/anifunnel.db
ENV ANIFUNNEL_LOG_LEVEL=info
ENV ANIFUNNEL_PORT=8000

EXPOSE 8000

CMD ["/anifunnel"]
