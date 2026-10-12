# diagram build helpers
CARGO_BIN := $(HOME)/.cargo/bin
# Prefer the rustup toolchain (has the wasm32 target) over e.g. Homebrew's rustc.
RUSTUP_BIN := $(shell rustup which rustc 2>/dev/null | xargs dirname 2>/dev/null)
WASM_ENV := PATH="$(RUSTUP_BIN):$(CARGO_BIN):$$PATH" DYLD_FALLBACK_LIBRARY_PATH="$(RUSTUP_BIN)/../lib" CARGO_TARGET_DIR="$(CURDIR)/target/wasm"

.PHONY: all wasm js build release install test e2e site dev embed clean

all: build

wasm:
	$(WASM_ENV) wasm-pack build crates/wasm --release --target no-modules --no-typescript --no-pack --out-dir ../../web/pkg

# npm package in js/: ES-module WASM (Node >= 18 and browsers) into js/pkg
js:
	$(WASM_ENV) CARGO_TARGET_DIR="$(CURDIR)/target/wasm-web" wasm-pack build crates/wasm --release --target web --no-typescript --no-pack --out-dir ../../js/pkg

build: wasm
	DIAGRAM_SKIP_WASM=1 cargo build

release: wasm
	DIAGRAM_SKIP_WASM=1 cargo build --release

install: wasm
	DIAGRAM_SKIP_WASM=1 cargo install --path crates/cli --force

test:
	cargo test --workspace
	node --test web/test/
	@# the js/ package tests need js/pkg (`make js`); skipped when it is not built
	@if [ -f js/pkg/diagram_wasm_bg.wasm ]; then node --test js/test/; else echo "skipping js/ tests: run 'make js' first"; fi

# Browser tests of the editor (Playwright + the installed Chrome; serves web/ from disk)
e2e: build
	cd scripts/e2e && (test -d node_modules || npm install --silent) && node mindmap.mjs

# Browser library: one JS file with the WASM inlined
embed: build
	./target/debug/diagram embed -o diagram.embed.js

# Static GitHub Pages site in ./docs (landing page, playground, examples)
site: release
	./scripts/build-site.sh

# Serve the frontend from disk while hacking on web/ (no rebuild needed)
dev: build
	DIAGRAM_WEB_DIR=web ./target/debug/diagram examples/checkout.diagram

clean:
	cargo clean
	rm -rf web/pkg js/pkg
