ROOT = $(shell git rev-parse --show-toplevel)

# builds

build: youki-release

youki: youki-dev # helper

youki-dev:
	./scripts/build.sh -o $(ROOT) -c youki

youki-release:
	./scripts/build.sh -o $(ROOT) -r -c youki

runtimetest:
	./scripts/build.sh -o $(ROOT) -r -c runtimetest

rust-oci-tests-bin:
	./scripts/build.sh -o $(ROOT) -r -c integration-test

all: youki-release rust-oci-tests-bin runtimetest

# Tests
unittest:
	cd ./crates && LD_LIBRARY_PATH=${HOME}/.wasmedge/lib cargo test --all --all-targets --all-features

featuretest:
	./scripts/features_test.sh

oci-tests: youki-release
	./scripts/oci_integration_tests.sh $(ROOT)

rust-oci-tests: youki-release runtimetest rust-oci-tests-bin
	./scripts/rust_integration_tests.sh $(ROOT)/youki

validate-rust-oci-runc: runtimetest rust-oci-tests-bin
	./scripts/rust_integration_tests.sh runc

containerd-test: youki-dev
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant provision --provision-with test

test-oci: oci-tests rust-oci-tests

test-all: unittest featuretest oci-tests containerd-test # currently not doing rust-oci here

# Misc

lint:
	cargo clippy --all-targets --all-features

clean:
	./scripts/clean.sh $(ROOT)
