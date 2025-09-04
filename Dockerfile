FROM rust:1.89-alpine3.20 as builder

RUN apk add musl-dev

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

RUN cargo fetch

COPY . .

RUN cargo run --release --bin dict_combine -- ./content/ --overwrite

RUN cargo install --path . --bin bot

FROM scratch

COPY --from=builder /usr/local/cargo/bin/bot .

CMD [ "./bot" ]
