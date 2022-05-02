FROM docker.io/library/rust:alpine3.15 as builder

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

FROM docker.io/library/alpine:3.15

RUN apk add \
  tini \
  libgcc \
  libstdc++

COPY --from=builder \
  /usr/local/bin/matrix-remote-closedown \
  /usr/local/bin/matrix-remote-closedown

ENTRYPOINT ["/sbin/tini", "--", "/usr/local/bin/matrix-remote-closedown"]
