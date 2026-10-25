# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM ghcr.io/thebearingedge/rust-builder:default AS rust-builder

ARG TARGETARCH

COPY --chown=65534:65534 . .

RUN \
  T="$(rust-target "$TARGETARCH")" \
  && cargo build --release --locked --target "$T" -p todos-api \
  && cp "target/$T/release/todos-api" /app/todos-api


FROM gcr.io/distroless/cc-debian12

COPY --from=rust-builder /app/todos-api /todos-api

USER 65534:65534

ENTRYPOINT ["/todos-api"]
