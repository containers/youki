ROOT = $(shell git rev-parse --show-toplevel)

# builds

.PHONY:build
build: youki-release

.PHONY: youki
youki: youki-dev # helper

.PHONY: youki-dev
youki-dev:
	./scripts/build.sh -o $(ROOT) -c youki

.PHONY: youki-release
youki-release:
	./scripts/build.sh -o $(ROOT) -r -c youki

.PHONY: runtimetest
runtimetest:
	./scripts/build.sh -o $(ROOT) -r -c runtimetest

.PHONY: rust-oci-tests-bin
rust-oci-tests-bin:
	./scripts/build.sh -o $(ROOT) -r -c integration-test

.PHONY: all
all: youki-release rust-oci-tests-bin runtimetest

# Tests

.PHONY: unittest
unittest:
	cd ./crates && LD_LIBRARY_PATH=${HOME}/.wasmedge/lib cargo test --all --all-targets --all-features

.PHONY: featuretest
featuretest:
	./scripts/features_test.sh

.PHONY: oci-tests
oci-tests: youki-release
	./scripts/oci_integration_tests.sh $(ROOT)

.PHONY: rust-oci-tests
rust-oci-tests: youki-release runtimetest rust-oci-tests-bin
	./scripts/rust_integration_tests.sh $(ROOT)/youki

.PHONY: validate-rust-oci-runc
validate-rust-oci-runc: runtimetest rust-oci-tests-bin
	./scripts/rust_integration_tests.sh runc

.PHONY: containerd-test
containerd-test: youki-dev
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant provision --provision-with test

.PHONY: test-oci
test-oci: oci-tests rust-oci-tests

.PHONY: test-all
test-all: unittest featuretest oci-tests containerd-test # currently not doing rust-oci here

# Misc

.PHONY: lint
lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: clean
clean:
	./scripts/clean.sh $(ROOT)
