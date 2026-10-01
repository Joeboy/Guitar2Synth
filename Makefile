CARGO ?= cargo
CARGO_TARGET_DIR ?= target
BUNDLE_DIR ?= build/guitar2frequency.lv2
PICO_BUNDLE_ROOT ?= build/picolv2
PICOLV2_SDK_DIR ?= ../../sdk
STRIP ?= 1
PICO_BUILD_DIR := build/pico-strip$(STRIP)
PICO_OBJECT := $(PICO_BUILD_DIR)/plugin.o
PICO_LIBRARY := $(PICO_BUILD_DIR)/plugin.so
PICO_BUNDLE := $(PICO_BUNDLE_ROOT)/guitar2frequency.lv2
PICO_STRIP_FLAG := $(if $(filter 0,$(STRIP)),,-Wl,--strip-debug)

.PHONY: all bundle bundle-pico clean

all: bundle

bundle:
	CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" $(CARGO) build --release
	mkdir -p "$(BUNDLE_DIR)"
	cp guitar2frequency.lv2/manifest.ttl guitar2frequency.lv2/guitar2frequency.ttl \
		"$(CARGO_TARGET_DIR)/release/libguitar2frequency.so" "$(BUNDLE_DIR)/"

$(PICO_BUILD_DIR):
	mkdir -p "$@"

$(PICO_OBJECT): src/lib.rs src/frequency.rs | $(PICO_BUILD_DIR)
	rustc --edition=2021 --target thumbv8m.main-none-eabihf --crate-type lib --emit=obj \
		-C panic=abort -C opt-level=2 -C overflow-checks=no -C relocation-model=pic \
		-o "$@" src/lib.rs

$(PICO_LIBRARY): $(PICO_OBJECT) $(PICOLV2_SDK_DIR)/src/runtime.c
	arm-none-eabi-gcc -mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16 \
		-shared -nostdlib -Wl,-Bsymbolic -Wl,-z,undefs \
		-Wl,-z,max-page-size=0x1000 -Wl,--no-warnings $(PICO_STRIP_FLAG) \
		-o "$@" $^ -Wl,--start-group -lc -lm -lnosys -lgcc -Wl,--end-group

bundle-pico: $(PICO_LIBRARY)
	mkdir -p "$(PICO_BUNDLE)"
	sed 's/libguitar2frequency.so/plugin.so/' guitar2frequency.lv2/manifest.ttl > "$(PICO_BUNDLE)/manifest.ttl"
	cp guitar2frequency.lv2/guitar2frequency.ttl "$(PICO_LIBRARY)" "$(PICO_BUNDLE)/"

clean:
	rm -rf build
