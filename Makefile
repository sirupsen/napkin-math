bench:
	./run --bench napkin_math

test:
	cargo bench --bench napkin_math -- --test

check:
	script/check-readme
	cargo clippy --all-targets --features encodings -- -D warnings
