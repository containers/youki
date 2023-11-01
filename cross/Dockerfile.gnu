ARG CROSS_BASE_IMAGE
ARG CROSS_DEB_ARCH
FROM $CROSS_BASE_IMAGE

ARG CROSS_DEB_ARCH
RUN dpkg --add-architecture ${CROSS_DEB_ARCH} && \
    apt-get -y update && \
    apt-get install -y pkg-config libseccomp-dev:${CROSS_DEB_ARCH}
