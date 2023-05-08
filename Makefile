ROOT = $(shell git rev-parse --show-toplevel)

DOCKER_BUILD ?= docker buildx build

KIND_CLUSTER_NAME ?= youki

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

.PHONY: test/k3s
test/k3s: bin/k3s
	sudo cp /var/lib/rancher/k3s/agent/etc/containerd/config.toml /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo 'default_runtime_name = "youki"' | sudo tee -a /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo '[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.youki]' | sudo tee -a /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo '  runtime_type = "io.containerd.runc.v2"' | sudo tee -a /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo '  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.youki.options]' | sudo tee -a /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo '    BinaryName = "$(PWD)/youki"' | sudo tee -a /var/lib/rancher/k3s/agent/etc/containerd/config.toml.tmpl && \
	echo "CONTAINERD_NAMESPACE='default'" | sudo tee /etc/systemd/system/k3s-runwasi.service.env && \
	echo "NO_PROXY=192.168.0.0/16" | sudo tee -a /etc/systemd/system/k3s-runwasi.service.env && \
	sudo systemctl daemon-reload && \
	sudo systemctl restart k3s-youki && \
	sudo bin/k3s kubectl apply -f tests/k8s/deploy.yaml
	sudo bin/k3s kubectl wait deployment nginx-deployment --for condition=Available=True --timeout=90s && \
	sudo bin/k3s kubectl get pods -o wide

.PHONY: test/k3s/clean
test/k3s/clean:
	sudo bin/k3s-youki-uninstall.sh

.PHONY: test/k8s/cluster
test/k8s/cluster: bin/kind tests/k8s/_out/img bin/kind
	bin/kind create cluster --name $(KIND_CLUSTER_NAME) --image="$(shell cat tests/k8s/_out/img)"

.PHONY: test/k8s/deploy
test/k8s/deploy: test/k8s/cluster
	kubectl --context=kind-$(KIND_CLUSTER_NAME) apply -f tests/k8s/deploy.yaml
	kubectl --context=kind-$(KIND_CLUSTER_NAME) wait deployment nginx-deployment --for condition=Available=True --timeout=90s
	kubectl --context=kind-$(KIND_CLUSTER_NAME) get pods -o wide

# Bin

.PHONY: bin/k3s
bin/k3s:
	mkdir -p bin && \
	curl -sfL https://get.k3s.io | INSTALL_K3S_BIN_DIR=$(PWD)/bin INSTALL_K3S_SYMLINK=skip INSTALL_K3S_NAME=youki sh -

.PHONY: bin/kind
bin/kind: tests/k8s/Dockerfile
	$(DOCKER_BUILD) --output=bin/ -f tests/k8s/Dockerfile --target kind-bin .

.PHONY: test/k8s/clean
test/k8s/clean:
	kind delete cluster --name $(KIND_CLUSTER_NAME)
	rm -r tests/k8s/_out

tests/k8s/_out/img: tests/k8s/Dockerfile Cargo.toml Cargo.lock $(shell find . -type f -name '*.rs')
	mkdir -p $(@D) && $(DOCKER_BUILD) -f tests/k8s/Dockerfile --iidfile=$(@) --load .

# Misc

.PHONY: lint
lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: hack/bpftrace
hack/bpftrace:
	BPFTRACE_STRLEN=125 ./hack/debug.bt

.PHONY: clean
clean:
	./scripts/clean.sh $(ROOT)
