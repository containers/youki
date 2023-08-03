alias build := youki-release
alias youki := youki-dev

KIND_CLUSTER_NAME := 'youki'

cwd := justfile_directory()

# build

# build all binaries
build-all: youki-release rust-oci-tests-bin runtimetest

# build youki in dev mode
youki-dev:
    {{ cwd }}/scripts/build.sh -o {{ cwd }} -c youki

# build youki in release mode
youki-release:
    {{ cwd }}/scripts/build.sh -o {{ cwd }} -r -c youki

# build runtimetest binary
runtimetest:
    {{ cwd }}/scripts/build.sh -o {{ cwd }} -r -c runtimetest

# build rust oci tests binary
rust-oci-tests-bin:
    {{ cwd }}/scripts/build.sh -o {{ cwd }} -r -c integration-test

# Tests

# run oci tests
test-oci: test-oci rust-oci-tests

# run all tests except rust-oci 
test-all: test-basic test-features oci-tests containerd-test # currently not doing rust-oci here

# run basic tests
test-basic: test-unit test-doc

# run cargo unit tests
test-unit:
    cargo test --lib --bins --all --all-targets --all-features

# run cargo doc tests
test-doc:
    cargo test --doc

# run permutated feature compilation tests
test-features:
    {{ cwd }}/scripts/features_test.sh

# run test against musl target
test-musl:
    {{ cwd }}/scripts/musl_test.sh

# run oci integration tests
test-oci: 
    {{ cwd }}/scripts/oci_integration_tests.sh {{ cwd }}

# run rust oci integration tests
rust-oci-tests: youki-release runtimetest rust-oci-tests-bin
    {{ cwd }}/scripts/rust_integration_tests.sh {{ cwd }}/youki

# validate rust oci integration tests on runc
validate-rust-oci-runc: runtimetest rust-oci-tests-bin
    {{ cwd }}/scripts/rust_integration_tests.sh runc

# run containerd integration tests
containerd-test: youki-dev
    VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
    VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant provision --provision-with test

[private]
kind-cluster: bin-kind
    #!/usr/bin/env bash
    set -euo pipefail

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
test-kind-clean:
	kind delete cluster --name {{ KIND_CLUSTER_NAME }}

# misc

# run bpftrace hack
hack-bpftrace:
    BPFTRACE_STRLEN=120 ./hack/debug.bt

# a hacky benchmark method we have been using casually to compare performance
hack-benchmark:
    #!/usr/bin/env bash
    set -euo pipefail

    hyperfine \
        --prepare 'sudo sync; echo 3 | sudo tee /proc/sys/vm/drop_caches' \
        --warmup 10 \
        --min-runs 100 \
        'sudo {{ cwd }}/youki create -b tutorial a && sudo {{ cwd }}/youki start a && sudo {{ cwd }}/youki delete -f a'

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
    {{ cwd }}/scripts/clean.sh {{ cwd }}

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

ci-musl-prepare: ci-prepare
    #!/usr/bin/env bash
    set -euo pipefail

    # Check if system is Ubuntu
    if [[ -f /etc/lsb-release ]]; then
        source /etc/lsb-release
        if [[ $DISTRIB_ID == "Ubuntu" ]]; then
            echo "System is Ubuntu"
            apt-get -y update
            apt-get install -y \
                musl-dev \
                musl-tools
            exit 0
        fi
    fi

    echo "Unknown system. The CI is only configured for Ubuntu. You will need to forge your own path. Good luck!"
    exit 1
