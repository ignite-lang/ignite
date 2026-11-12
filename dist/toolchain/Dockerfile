FROM rust:1.98-trixie

ARG VERSION

LABEL org.opencontainers.image.source="https://github.com/ignite-lang/ignite" \
      org.opencontainers.image.title="ignite-toolchain" \
      org.opencontainers.image.description="Ignite ${VERSION}: ignite, its std, clang and lld for --target llvm, Rust and the archive's substrate with its crates fetched for --target rust, offline" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.documentation="https://ignite-lang.github.io/ignite/book/guide/install.html"

RUN apt-get update \
 && apt-get install -y --no-install-recommends clang lld procps \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/src
RUN test -n "${VERSION}" \
 && base="https://github.com/ignite-lang/ignite/releases/download/v${VERSION}" \
 && name="ignite-${VERSION}-x86_64-linux" \
 && curl -fsSLO "${base}/${name}.tar.gz" \
 && curl -fsSLO "${base}/${name}.tar.gz.sha256" \
 && sha256sum -c "${name}.tar.gz.sha256" \
 && tar -xzf "${name}.tar.gz" \
 && install -m 0755 "${name}/bin/ignite" /usr/local/bin/ignite \
 && mkdir -p /usr/local/share/ignite \
 && cp -R "${name}/share/ignite/std" "${name}/share/ignite/substrate" /usr/local/share/ignite/ \
 && rm -rf "${name}" "${name}.tar.gz" "${name}.tar.gz.sha256" \
 && curl -fsSL -o source.tar.gz "https://github.com/ignite-lang/ignite/archive/refs/tags/v${VERSION}.tar.gz" \
 && gzip -dc source.tar.gz | git get-tar-commit-id > COMMIT \
 && grep -qxE '[0-9a-f]{40}' COMMIT \
 && tar -xzf source.tar.gz \
 && rm source.tar.gz \
 && mv "ignite-${VERSION}" ignite \
 && mv COMMIT ignite/COMMIT \
 && ignite --version | grep -qx "ignite ${VERSION}"

RUN ignite emit /usr/local/src/ignite/examples/todo/todo.ignite \
      | grep -qxF 'ignite-io = { path = "/usr/local/share/ignite/substrate/crates/ignite-io" }' \
 && cargo fetch --locked --manifest-path /usr/local/share/ignite/substrate/Cargo.toml

ENV CARGO_NET_OFFLINE=true
WORKDIR /work
