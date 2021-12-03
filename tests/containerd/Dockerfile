FROM ubuntu:latest

ENV GOPATH=/root/go
ENV PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/root/go/bin

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get -y upgrade \
    && apt-get install -y bash golang-1.16 libbtrfs-dev libnl-3-dev libnet1-dev \
    protobuf-c-compiler libcap-dev libaio-dev \
    curl libprotobuf-c-dev libprotobuf-dev socat libseccomp-dev \
    pigz lsof make git gcc build-essential pkgconf libtool \
    libdbus-glib-1-dev libsystemd-dev libcap-dev libyajl-dev \
    go-md2man libtool autoconf python3 automake sudo \
    && update-alternatives --install /usr/bin/go go /usr/lib/go-1.16/bin/go 0 \
    && mkdir -p /root/go/src/github.com/containerd \
    && chmod 755 /root \
    && (cd /root/go/src/github.com/containerd \
    && git clone https://github.com/containerd/containerd \
    && cd containerd \
    && git reset --hard v1.5.7 \
    && make \
    && make binaries \
    && make install \
    && script/setup/install-cni \
    && script/setup/install-critools) \
    && rm -rf /bin/runc /sbin/runc /usr/sbin/runc /usr/bin/runc

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH $PATH:/root/.cargo/bin
RUN rustc --version

COPY run.sh /usr/local/bin

ENTRYPOINT /usr/local/bin/run.sh /root/go/src/github.com/containerd/containerd