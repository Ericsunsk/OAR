# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /workspace

COPY . .
RUN cargo build --release --locked -p oar-http-facade

FROM debian:bookworm-slim AS runtime

ENV OAR_HTTP_BIND_ADDR=0.0.0.0:8080
EXPOSE 8080

COPY --from=builder /workspace/target/release/oar-http-facade /usr/local/bin/oar-http-facade

USER 65532:65532
ENTRYPOINT ["/usr/local/bin/oar-http-facade"]
