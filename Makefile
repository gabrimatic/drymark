.PHONY: build check clean desktop fmt fuzz lint public-check runtime-smoke-macos setup-check test verify

build:
	npm run build
	cargo build --locked --workspace --all-targets

check: fmt lint test

clean:
	cargo clean
	rm -rf apps/desktop/dist fuzz/target mutants.out

desktop:
	npm run tauri -- build -- --locked

fmt:
	cargo fmt --all -- --check

fuzz:
	cd fuzz && cargo +nightly-2026-08-10 metadata --locked --no-deps --format-version 1
	cd fuzz && cargo +nightly-2026-08-10 fuzz run sanitize -- -max_total_time=60
	cd fuzz && cargo +nightly-2026-08-10 fuzz run transaction -- -max_total_time=60

lint:
	cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
	npm run lint:ui

runtime-smoke-macos:
	bash scripts/macos-package-smoke.sh

setup-check:
	bash scripts/setup.test.sh
	./setup.sh --check

public-check:
	bash scripts/check-public-tree.test.sh
	node scripts/check-readme-media.test.mjs
	bash scripts/check-public-tree.sh

test:
	cargo test --locked --workspace --all-targets
	npm run test:ui

verify: check public-check setup-check
	npm run build
	npm audit --audit-level=high
	cargo audit
	cargo deny check
