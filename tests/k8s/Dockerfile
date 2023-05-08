# syntax=docker/dockerfile:1.4

ARG KIND_NODE_VERSION=v1.23.13

FROM kindest/node:${KIND_NODE_VERSION} AS kind-base

FROM kind-base AS shim-build
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup.sh && sh /tmp/rustup.sh -y --profile=minimal
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /shim
COPY ./youki /shim/youki

FROM scratch AS shim
COPY --from=shim-build /shim/youki /

FROM kind-base AS kind-fetch
ARG TARGETARCH
ARG KIND_VERSION=v0.17.0
RUN curl -sSLf https://kind.sigs.k8s.io/dl/${KIND_VERSION}/kind-linux-${TARGETARCH} > /root/kind && chmod +x /root/kind

FROM scratch AS kind-bin
COPY --from=kind-fetch /root/kind /kind

FROM kind-base
RUN <<EOF
set -e
echo '[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.youki]' >> /etc/containerd/config.toml
echo '  runtime_type = "io.containerd.runc.v2"' >> /etc/containerd/config.toml
echo '   [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.youki.options]' >> /etc/containerd/config.toml
echo '     BinaryName = "/usr/local/bin/youki"' >> /etc/containerd/config.toml
sed -i 's,SystemdCgroup = true,,' /etc/containerd/config.toml
EOF
COPY .github/scripts/dependency.sh .github/scripts/dependency.sh
RUN apt-get update && .github/scripts/dependency.sh
COPY --link --from=shim /* /usr/local/bin/

