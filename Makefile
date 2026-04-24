# Gravital Sound — targets de build reproducibles.
# Uso: make <target>. Ejecuta `make help` para ver la lista.

CARGO        ?= cargo
TARGET_DIR   ?= target
WORKSPACE_FLAGS := --workspace --all-targets

.PHONY: help
help:
	@awk 'BEGIN{FS=":.*##"; printf "Gravital Sound — targets disponibles:\n\n"} \
		/^[a-zA-Z0-9_.-]+:.*?##/ {printf "  \033[36m%-24s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: build
build: ## Compila todo el workspace en modo release
	$(CARGO) build $(WORKSPACE_FLAGS) --release

.PHONY: dev
dev: ## Compila todo el workspace en modo debug
	$(CARGO) build $(WORKSPACE_FLAGS)

.PHONY: test
test: ## Ejecuta la suite completa de tests
	$(CARGO) test $(WORKSPACE_FLAGS)

.PHONY: fmt
fmt: ## Formatea el código con rustfmt
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Verifica formato sin modificar
	$(CARGO) fmt --all -- --check

.PHONY: clippy
clippy: ## Lints estrictos (-D warnings, +perf, +nursery)
	$(CARGO) clippy $(WORKSPACE_FLAGS) -- -D warnings -W clippy::perf -W clippy::nursery

.PHONY: bench
bench: ## Ejecuta todos los benchmarks con criterion
	$(CARGO) bench $(WORKSPACE_FLAGS)

.PHONY: check-all
check-all: fmt-check clippy test ## Gate completo (fmt + clippy + test)

.PHONY: cross-linux-arm64
cross-linux-arm64: ## Cross-compile a aarch64-unknown-linux-gnu
	cross build --release --target aarch64-unknown-linux-gnu \
		-p gravital-sound-core -p gravital-sound-transport -p gravital-sound-ffi

.PHONY: cross-wasm
cross-wasm: ## Verifica que el core compila a wasm32 (no_std check)
	$(CARGO) check --target wasm32-unknown-unknown -p gravital-sound-core --no-default-features

.PHONY: ffi-header
ffi-header: ## Regenera gravital_sound.h con cbindgen
	$(CARGO) build -p gravital-sound-ffi --release
	@echo "Header generado en crates/gravital-sound-ffi/include/gravital_sound.h"

.PHONY: ffi-smoke
ffi-smoke: ffi-header ## Compila y ejecuta el smoke test en C
	gcc -fsyntax-only crates/gravital-sound-ffi/include/gravital_sound.h
	gcc -O2 -I crates/gravital-sound-ffi/include \
		-o $(TARGET_DIR)/c_smoke crates/gravital-sound-ffi/tests/c_smoke.c \
		-L $(TARGET_DIR)/release -lgravital_sound_ffi -lpthread -ldl -lm
	LD_LIBRARY_PATH=$(TARGET_DIR)/release $(TARGET_DIR)/c_smoke

.PHONY: python-sdk
python-sdk: ## Compila e instala el SDK Python con maturin
	cd sdks/python && maturin develop --release

.PHONY: python-test
python-test: python-sdk ## Ejecuta la suite de pytest del SDK Python
	cd sdks/python && pytest -v

.PHONY: web-sdk
web-sdk: ## Compila el SDK Web a WASM con wasm-pack
	cd sdks/web && wasm-pack build --target web --release --out-dir pkg

.PHONY: loopback
loopback: ## Ejecuta el benchmark de loopback end-to-end
	$(CARGO) run --release --example loopback

.PHONY: pgo-build
pgo-build: ## Build con Profile-Guided Optimization (requiere nightly)
	./scripts/build-pgo.sh

.PHONY: clean
clean: ## Limpia artefactos de build
	$(CARGO) clean
	rm -rf sdks/web/pkg sdks/web/target sdks/python/target sdks/python/dist

.PHONY: doc
doc: ## Genera la documentación rustdoc
	$(CARGO) doc $(WORKSPACE_FLAGS) --no-deps --open
