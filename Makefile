ROOT = $(shell git rev-parse --show-toplevel)

build:
	./scripts/build.sh -o $(ROOT)

release-build:
	./scripts/build.sh -o $(ROOT) -r

test-all: test oci-integration-test integration-test

test: build
	cd crates && cargo test

oci-integration-test: release-build
	./scripts/oci_integration_tests.sh $(ROOT)

oci-test-cov: build
	./scripts/oci_integration_tests.sh $(ROOT)

integration-test: release-build
	./scripts/rust_integration_tests.sh $(ROOT)/youki

validate-rust-tests: release-build
	./scripts/rust_integration_tests.sh runc

clean:
	./scripts/clean.sh $(ROOT)
