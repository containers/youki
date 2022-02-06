.PHONY : debug release

TGT = x86_64-unknown-linux-gnu

debug:
	cargo build && cp ./target/$(TGT)/debug/youki ./youki_bin

release:
	cargo build --release && cp ./target/$(TGT)/release/youki ./youki_bin

clean:
	rm ./youki_bin