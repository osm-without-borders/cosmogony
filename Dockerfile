FROM rust:1-slim-buster as builder

WORKDIR /srv/cosmogony

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get install -y libgeos-c1v5 libgeos-dev pkg-config && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY . ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/srv/cosmogony/target \
    cargo build --profile production

RUN --mount=type=cache,target=/srv/cosmogony/target  \
    cp target/production/cosmogony cosmogony.bin

FROM debian:buster-slim

WORKDIR /srv

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get install -y libgeos-c1v5 libgeos-dev pkg-config && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=builder /srv/cosmogony/cosmogony.bin /usr/bin/cosmogony

ENTRYPOINT ["cosmogony"]
