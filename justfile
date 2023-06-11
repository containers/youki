alias build := youki-release
alias youki := youki-dev

KIND_CLUSTER_NAME := 'youki'

# build

# build all binaries
build-all: youki-release rust-oci-tests-bin runtimetest

# build youki in dev mode
youki-dev:
    {{ justfile_directory() }}/scripts/build.sh -o {{ justfile_directory() }} -c youki

# build youki in release mode
youki-release:
    {{ justfile_directory() }}/scripts/build.sh -o . -r -c youki

# build runtimetest binary
runtimetest:
    {{ justfile_directory() }}/scripts/build.sh -o . -r -c runtimetest

# build rust oci tests binary
rust-oci-tests-bin:
    {{ justfile_directory() }}/scripts/build.sh -o {{ justfile_directory() }} -r -c integration-test

# Tests

# run oci tests
test-oci: oci-tests rust-oci-tests

# run all tests except rust-oci 
test-all: unittest featuretest oci-tests containerd-test # currently not doing rust-oci here

# run cargo unittests
unittest:
    cd ./crates
    LD_LIBRARY_PATH=${HOME}/.wasmedge/lib cargo test --all --all-targets --all-features

# run purmutated feature compilation tests
featuretest:
    ./scripts/features_test.sh

# run oci integration tests
oci-tests: 
    ./scripts/oci_integration_tests.sh {{ justfile_directory() }}

# run rust oci integration tests
rust-oci-tests: youki-release runtimetest rust-oci-tests-bin
    ./scripts/rust_integration_tests.sh ./youki

# validate rust oci integration tests on runc
validate-rust-oci-runc: runtimetest rust-oci-tests-bin
    ./scripts/rust_integration_tests.sh runc

# run containerd integration tests
containerd-test: youki-dev
    VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
    VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant provision --provision-with test

[private]
kind-cluster: bin-kind
    #!/usr/bin/env bash
    set -euxo pipefail

    mkdir -p tests/k8s/_out/
    docker buildx build -f tests/k8s/Dockerfile --iidfile=tests/k8s/_out/img --load .
    image=$(cat tests/k8s/_out/img)
    bin/kind create cluster --name {{ KIND_CLUSTER_NAME }} --image=$image

# run youki with kind
test-kind: kind-cluster
    kubectl --context=kind-{{ KIND_CLUSTER_NAME }} apply -f tests/k8s/deploy.yaml
    kubectl --context=kind-{{ KIND_CLUSTER_NAME }} wait deployment nginx-deployment --for condition=Available=True --timeout=90s
    kubectl --context=kind-{{ KIND_CLUSTER_NAME }} get pods -o wide
    kubectl --context=kind-{{ KIND_CLUSTER_NAME }} delete -f tests/k8s/deploy.yaml

# Bin

[private]
bin-kind:
	docker buildx build --output=bin/ -f tests/k8s/Dockerfile --target kind-bin .

# Clean

# Clean kind test env
clean-test-kind:
	kind delete cluster --name {{ KIND_CLUSTER_NAME }}

# misc

# run bpftrace hack
hack-bpftrace:
    BPFTRACE_STRLEN=120 ./hack/debug.bt

# run linting on project
lint:
    cargo fmt --all -- --check
    cargo clippy --all --all-targets --all-features -- -D warnings

# run spellcheck
spellcheck:
    typos

# run format on project
format:
    cargo fmt --all

# cleans up generated artifacts
clean:
    ./scripts/clean.sh .

# install tools used in dev
dev-prepare:
    cargo install typos-cli

# setup dependencies in CI
ci-prepare:
    #!/usr/bin/env bash
    set -euo pipefail

    # Check if system is Ubuntu
    if [[ -f /etc/lsb-release ]]; then
        source /etc/lsb-release
        if [[ $DISTRIB_ID == "Ubuntu" ]]; then
            echo "System is Ubuntu"
            apt-get -y update
            apt-get install -y \
                pkg-config \
                libsystemd-dev \
                libdbus-glib-1-dev \
                build-essential \
                libelf-dev \
                libseccomp-dev \
                libclang-dev \
                libssl-dev \
                criu
            exit 0
        fi
    fi

    echo "Unknown system. The CI is only configured for Ubuntu. You will need to forge your own path. Good luck!"
    exit 1
