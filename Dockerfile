FROM rust:1.96.0-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libsqlite3-dev git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --locked --release \
    -p aml-api \
    -p aml-wallet \
    -p aml-alipay \
    -p aml-paypay \
    -p aml-metrics

FROM debian:bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends libsqlite3-0 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /app/crates/aml-api /app/data

COPY --from=builder /app/target/release/aml-api /usr/local/bin/aml-api
COPY --from=builder /app/target/release/aml-wallet /usr/local/bin/aml-wallet
COPY --from=builder /app/target/release/aml-alipay /usr/local/bin/aml-alipay
COPY --from=builder /app/target/release/aml-paypay /usr/local/bin/aml-paypay
COPY --from=builder /app/target/release/aml-metrics /usr/local/bin/aml-metrics
COPY --from=builder /app/web /app/web

ENV PORT=8880
ENV AML_AUDIT_DB_URL=sqlite:/app/data/aml_audit.sqlite
ENV WALLET_PORT=8891
ENV ALIPAY_PORT=8892
ENV PAYPAY_PORT=8893
ENV ALIPAY_BASE_URL=http://alipay:8892
ENV PAYPAY_BASE_URL=http://paypay:8893

EXPOSE 8880 8891 8892 8893
CMD ["aml-api"]
