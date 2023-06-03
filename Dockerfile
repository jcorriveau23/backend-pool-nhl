# build the src code.
FROM rust:1.69 as builder
WORKDIR /src
COPY . .
RUN cargo build


# Run the executable file.
FROM debian:stable-slim
WORKDIR /

EXPOSE 8000

COPY --from=builder /src/target/debug/server ./

CMD ["/server"]