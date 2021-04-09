FROM rust:alpine as cargo-build

RUN apk add --no-cache -U libc-dev
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

COPY . .

RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM alpine

WORKDIR /app

COPY --from=cargo-build /build/target/x86_64-unknown-linux-musl/release/prosafe_exporter .

EXPOSE 9493/tcp
ENTRYPOINT ["/app/prosafe_exporter"]
