ROOT = $(shell git rev-parse --show-toplevel)

build:
	./scripts/build.sh -o $(ROOT)

release-build:
	./scripts/build.sh -o $(ROOT) -r

test-all: test oci-integration-test integration-test

test: build
	cd crates && cargo test

oci-integration-test: build
	./scripts/oci_integration_tests.sh $(ROOT)

integration-test: build
	./scripts/rust_integration_tests.sh $(ROOT)/youki

clean:
	./scripts/clean.sh $(ROOT)
