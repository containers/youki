ROOT = $(shell git rev-parse --show-toplevel)

build:
	./scripts/build.sh -o $(ROOT)

release-build:
	./scripts/build.sh -o $(ROOT) -r

test-all: test oci-integration-test integration-test features-test

test: build
	cd crates && cargo test

features-test: build
	./scripts/features_test.sh

oci-integration-test:
	./scripts/oci_integration_tests.sh $(ROOT)

integration-test:
	./scripts/rust_integration_tests.sh $(ROOT)/youki

validate-rust-tests:
	./scripts/rust_integration_tests.sh runc

clean:
	./scripts/clean.sh $(ROOT)

containerd-test:
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant up
	VAGRANT_VAGRANTFILE=Vagrantfile.containerd2youki vagrant provision --provision-with test


