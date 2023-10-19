FROM docker.io/library/rust:alpine3.18 as builder

RUN apk add \
  cmake \
  g++ \
  libc-dev \
  make \
  openssl-dev

ADD Cargo.toml Cargo.lock .
ADD src ./src
RUN RUSTFLAGS=-Ctarget-feature=-crt-static cargo install \
  --path . \
  --root /usr/local

FROM docker.io/library/alpine:3.18

RUN apk add \
  tini \
  libgcc \
  libstdc++

COPY --from=builder \
  /usr/local/bin/matrix-remote-closedown \
  /usr/local/bin/matrix-remote-closedown

ENV MATRIX_STORAGE /data/matrix
RUN mkdir /data
VOLUME /data

ENV OBSERVABILITY_ADDRESS "0.0.0.0:9090"
EXPOSE 9090

ENTRYPOINT ["/sbin/tini", "--", "/usr/local/bin/matrix-remote-closedown"]
