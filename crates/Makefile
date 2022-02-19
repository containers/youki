.PHONY : debug release

debug:
	cargo build && cp ./target/debug/youki ./youki_bin

release:
	cargo build --release && cp ./target/release/youki ./youki_bin

clean:
	rm ./youki_bin