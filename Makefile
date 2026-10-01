CARGO ?= cargo
CARGO_TARGET_DIR ?= target
BUNDLE_DIR ?= build/guitar2frequency.lv2

.PHONY: all bundle

all: bundle

bundle:
	CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" $(CARGO) build --release
	mkdir -p "$(BUNDLE_DIR)"
	cp guitar2frequency.lv2/manifest.ttl guitar2frequency.lv2/guitar2frequency.ttl \
		"$(CARGO_TARGET_DIR)/release/libguitar2frequency.so" "$(BUNDLE_DIR)/"
