FROM rust:latest

WORKDIR /app

COPY ./code/src /app/src
COPY ./code/Cargo.toml /app/Cargo.toml
COPY ./code/log_config.yml /app/log_config.yml
COPY ./code/.env /app/.env
COPY ./docker-entrypoint.sh /root/docker-entrypoint.sh

RUN cargo build --release

CMD ["/app/target/release/code"]