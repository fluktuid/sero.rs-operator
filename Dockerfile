# Build Stage
FROM rust:1.68-buster AS builder
RUN apt update && apt install -y pkg-config gcc musl musl-dev pkg-config musl-tools libssl-dev

# Set `SYSROOT` to a dummy path (default is /usr) because pkg-config-rs *always*
# links those located in that path dynamically but we want static linking, c.f.
# https://github.com/rust-lang/pkg-config-rs/blob/54325785816695df031cef3b26b6a9a203bbc01b/src/lib.rs#L613
ENV SYSROOT=/dummy
# The env var tells pkg-config-rs to statically link libpq.
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include/openssl

WORKDIR /home/rust/src/
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src
RUN cargo new main
WORKDIR /usr/src/main
COPY Cargo.toml ./
ENV BUILD_PROFILE="release_container"
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/main/target \
    cargo build --profile ${BUILD_PROFILE}

COPY src ./src

RUN  --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/main/target \
    cargo install --profile ${BUILD_PROFILE} --target x86_64-unknown-linux-musl --path .

# create tmp folder for use in scratch
RUN mkdir /my_tmp

# Bundle Stage
FROM scratch
ARG UID=10001
ARG GID=10001
USER ${UID}:${GID}
COPY --from=builder --chown=${UID}:${GID} /my_tmp /tmp
COPY --from=builder --chown=${UID}:${GID} /usr/local/cargo/bin/sero-operator /main

# Use an unprivileged user.
LABEL org.opencontainers.image.source=https://github.com/fluktuid/sero.rs-operator

ENTRYPOINT ["/main"]
