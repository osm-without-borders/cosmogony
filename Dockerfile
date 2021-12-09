FROM rust:1-slim-buster as builder

WORKDIR /srv/cosmogony

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get install -y libgeos-c1v5 libgeos-dev && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY . ./

RUN cargo build --profile production

FROM debian:buster-slim

WORKDIR /srv

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get update && apt-get install -y libgeos-c1v5 libgeos-dev && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=builder /srv/cosmogony/target/production/cosmogony /usr/bin/cosmogony

ENTRYPOINT ["cosmogony"]
