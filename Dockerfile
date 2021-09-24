FROM rust as build

RUN rustup default nightly

WORKDIR /usr/src
RUN USER=root cargo new shorty
WORKDIR /usr/src/shorty

# Caches build dependencies by writing placeholder lib and main files.
COPY Cargo.toml Cargo.lock ./

COPY src ./src

RUN cargo install --path .

FROM debian:buster-slim

RUN apt-get update
RUN apt-get install -y libpq-dev libsqlite3-dev

COPY --from=build /usr/local/cargo/bin/shorty /usr/local/bin/shorty
COPY Rocket.toml ./

EXPOSE 8000
CMD ["shorty"]

