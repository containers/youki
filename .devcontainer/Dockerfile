ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:1-${VARIANT}

# For installing just
RUN <<EOF
  wget -qO - 'https://proget.makedeb.org/debian-feeds/prebuilt-mpr.pub' | gpg --dearmor | tee /usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg 1> /dev/null
  echo "deb [arch=all,$(dpkg --print-architecture) signed-by=/usr/share/keyrings/prebuilt-mpr-archive-keyring.gpg] https://proget.makedeb.org prebuilt-mpr $(lsb_release -cs)" | tee /etc/apt/sources.list.d/prebuilt-mpr.list
  apt update
EOF

# For building
RUN apt-get install -y \
  git \
  pkg-config \
  libsystemd-dev \
  build-essential \
  libelf-dev \
  libseccomp-dev \
  libclang-dev \
  libssl-dev \
  just

# For debugging
RUN apt install -y \
  podman \
  bpftrace

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

