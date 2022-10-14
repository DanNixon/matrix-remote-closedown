# matrix-remote-closedown

[![CI](https://github.com/DanNixon/matrix-remote-closedown/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/DanNixon/matrix-remote-closedown/actions/workflows/ci.yml)
[![dependency status](https://deps.rs/repo/github/dannixon/matrix-remote-closedown/status.svg)](https://deps.rs/repo/github/dannixon/matrix-remote-closedown)

A [Matrix](https://matrix.org/) bot that provides a nice interface to [remote-closedown](https://github.com/DanNixon/remote-closedown).

## Usage

See `matrix-remote-closedown --help`.

Note that the bot user must already be a member of the rooms specified via the `--room` flag.

## Deployment

E.g. via Podman:
```sh
podman run \
  --rm -it \
  -e RUST_LOG=debug \
  ghcr.io/DanNixon/matrix-remote-closedown:latest \
  --station-name 'mb7pmf' \
  --mqtt-broker 'tcp://broker.hivemq.com' \
  --status-topic 'mb7pmf' \
  --command-topic 'mb7pmf/command' \
  --matrix-username '@mb7pmf:matrix.org' \
  --matrix-password 'super_secret' \
  --room '!some_room:matrix.org' \
  --room '!some_other_room:matrix.org' \
  --operator '@dannixon:matrix.org'
  --operator '@someone_else:matrix.org'
```
