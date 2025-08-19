FROM rust:1.87.0 as builder

RUN apt-get update && apt-get install -y musl-tools

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

RUN cargo fetch

COPY . .

RUN cargo run --target x86_64-unknown-linux-musl --release --bin dict_combine -- ./content/ --overwrite

RUN cargo install --target x86_64-unknown-linux-musl --path . --bin bot

FROM scratch

COPY --from=builder /usr/local/cargo/bin/bot .

CMD [ "./bot" ]
