FROM rust:1.63.0 as build

WORKDIR /mqtt-bench
COPY src /mqtt-bench/src
COPY Cargo.toml /mqtt-bench/
COPY Cargo.lock /mqtt-bench/
RUN cargo build --release

FROM gcr.io/distroless/static-debian11
COPY --from=build /mqtt-bench/target/release/mqtt-bench /mqtt-bench/
CMD ["/mqtt-bench/mqtt-bench -f /mqtt-bench/conf/config.yml"]
