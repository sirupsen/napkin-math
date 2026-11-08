bench:
	cargo bench

test:
	cargo bench -- --test

check:
	script/check-readme
	cargo clippy --all-targets --features encodings -- -D warnings
