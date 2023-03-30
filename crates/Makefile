.PHONY: debug
debug:
	cargo build && cp ./target/debug/youki ./youki_bin

.PHONY: release
release:
	cargo build --release && cp ./target/release/youki ./youki_bin

.PHONY: clean
clean:
	rm ./youki_bin
