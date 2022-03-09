.PHONY: runtimetest integration-test


TGT = x86_64-unknown-linux-gnu
FLAG = 
ifeq ("$(FLAG)","--release")
DIR = release
else
DIR = debug
endif


all: runtimetest integration-test

runtimetest:
	cd ./runtimetest && cargo build $(FLAG) && cp ./target/$(TGT)/$(DIR)/runtimetest ../runtimetest_bin

integration-test:
	cd ./integration_test && cargo build $(FLAG) && cp ./target/$(DIR)/integration_test ../integration_test_bin

clean:
	rm ./integration_test_bin && rm ./runtimetest_bin