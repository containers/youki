ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:1-${VARIANT}

RUN apt-get update

# For building
RUN apt-get install -y \
  git \
  pkg-config \
  libsystemd-dev \
  build-essential \
  libelf-dev \
  libseccomp-dev \
  libclang-dev \
  libssl-dev

# For debugging
RUN apt install -y \
  podman \
  bpftrace

# Install just
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/bin

# Install docker with youki
COPY <<EOF /etc/docker/daemon.json
  {
    "runtimes": {
      "youki": {
        "path": "/workspaces/youki/youki"
      }
    }
  }
EOF

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y 
RUN rustup component add rustfmt
RUN rustup component add clippy 

