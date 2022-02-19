FROM gitpod/workspace-full

RUN sudo apt-get update && sudo apt-get install -y \
      pkg-config \
      libsystemd-dev \
      libdbus-1-dev \
      build-essential \
      libelf-dev \
      libseccomp-dev