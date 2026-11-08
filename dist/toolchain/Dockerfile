FROM rust:1.98-trixie

ARG VERSION

LABEL org.opencontainers.image.source="https://github.com/ignite-lang/ignite" \
      org.opencontainers.image.title="ignite-toolchain" \
      org.opencontainers.image.description="Ignite ${VERSION}: ignite, its std, clang and lld for --target llvm, Rust and the tag's substrate with its crates fetched for --target rust, offline" \
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
 && cp -R "${name}/share/ignite/std" /usr/local/share/ignite/std \
 && rm -rf "${name}" "${name}.tar.gz" "${name}.tar.gz.sha256" \
 && curl -fsSL -o source.tar.gz "https://github.com/ignite-lang/ignite/archive/refs/tags/v${VERSION}.tar.gz" \
 && gzip -dc source.tar.gz | git get-tar-commit-id > COMMIT \
 && grep -qxE '[0-9a-f]{40}' COMMIT \
 && tar -xzf source.tar.gz \
 && rm source.tar.gz \
 && mv "ignite-${VERSION}" ignite \
 && mv COMMIT ignite/COMMIT \
 && ignite --version | grep -qx "ignite ${VERSION}"

RUN root="$(ignite emit /usr/local/src/ignite/examples/todo/todo.ignite \
      | sed -n 's|^ignite-io = { path = "\(.*\)/crates/ignite/\.\./ignite-io" }$|\1|p' | head -n 1)" \
 && test -n "${root}" \
 && if [ "${root}" != /usr/local/src/ignite ]; then mkdir -p "$(dirname "${root}")" && ln -s /usr/local/src/ignite "${root}"; fi \
 && test -f "${root}/crates/ignite/../ignite-io/Cargo.toml" \
 && cd /usr/local/src/ignite \
 && cargo fetch --locked

ENV CARGO_NET_OFFLINE=true
WORKDIR /work
