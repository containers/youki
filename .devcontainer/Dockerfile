ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:1-${VARIANT}

RUN apt-get update
RUN apt-get install -y \
  git \
  make

RUN apt-get install -y \
  pkg-config \
  libsystemd-dev \
  libdbus-glib-1-dev \
  build-essential \
  libelf-dev \
  libseccomp-dev \
  libclang-dev \
  libssl-dev 

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y 
RUN rustup component add rustfmt
RUN rustup component add clippy 

