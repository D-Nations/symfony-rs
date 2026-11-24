# Cache tool installation, never the source tree or acceptance build artifacts.
FROM rust@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7

RUN rustup component add rustfmt clippy \
    && useradd --create-home --uid 1000 check \
    && mkdir -p /work /cache/cargo /cache/target \
    && chown -R check:check /work /cache

COPY run.sh /usr/local/bin/linux-check
ENV CARGO_HOME=/cache/cargo
WORKDIR /work
USER check
ENTRYPOINT ["bash", "/usr/local/bin/linux-check"]
