# BUILD CONTAINER

FROM rust:1.65 as build

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN USER=root cargo new --bin plex-anihook

# Build dependencies separately for layer caching.
WORKDIR ./plex-anihook
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release

# Clean the temporary project.
RUN rm src/*.rs ./target/release/deps/plex_anihook*

ADD . ./
RUN cargo build --release --verbose


# RUNTIME CONTAINER

FROM debian:bullseye-slim

COPY --from=build /plex-anihook/target/release/plex-anihook .

ENV ANILIST_TOKEN= \
    BIND_ADDRESS=0.0.0.0 \
    PORT=8000

EXPOSE 8000

CMD ["./plex-anihook"]
