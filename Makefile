CARGO ?= cargo
CARGO_TARGET_DIR ?= target
BUNDLE_ROOT ?= build
FREQUENCY_BUNDLE_DIR ?= $(BUNDLE_ROOT)/guitar2frequency.lv2
GATE_TRIGGER_BUNDLE_DIR ?= $(BUNDLE_ROOT)/guitar2gatetrigger.lv2
MIDI_BUNDLE_DIR ?= $(BUNDLE_ROOT)/guitar2midi.lv2

PICO_BUNDLE_ROOT ?= build/picolv2
PICOLV2_SDK_DIR ?= ../../sdk
STRIP ?= 1
PICO_BUILD_DIR := build/pico-strip$(STRIP)
PICO_FREQUENCY_DIR := $(PICO_BUILD_DIR)/guitar2frequency
PICO_GATE_TRIGGER_DIR := $(PICO_BUILD_DIR)/guitar2gatetrigger
PICO_MIDI_DIR := $(PICO_BUILD_DIR)/guitar2midi
PICO_FREQUENCY_OBJECT := $(PICO_FREQUENCY_DIR)/plugin.o
PICO_GATE_TRIGGER_OBJECT := $(PICO_GATE_TRIGGER_DIR)/plugin.o
PICO_MIDI_OBJECT := $(PICO_MIDI_DIR)/plugin.o
PICO_FREQUENCY_LIBRARY := $(PICO_FREQUENCY_DIR)/plugin.so
PICO_GATE_TRIGGER_LIBRARY := $(PICO_GATE_TRIGGER_DIR)/plugin.so
PICO_MIDI_LIBRARY := $(PICO_MIDI_DIR)/plugin.so
PICO_FREQUENCY_BUNDLE := $(PICO_BUNDLE_ROOT)/guitar2frequency.lv2
PICO_GATE_TRIGGER_BUNDLE := $(PICO_BUNDLE_ROOT)/guitar2gatetrigger.lv2
PICO_MIDI_BUNDLE := $(PICO_BUNDLE_ROOT)/guitar2midi.lv2
PICO_STRIP_FLAG := $(if $(filter 0,$(STRIP)),,-Wl,--strip-debug)

.PHONY: all bundle bundle-pico bundle-pico-frequency bundle-pico-gatetrigger bundle-pico-midi clean

all: bundle

bundle:
	CARGO_TARGET_DIR="$(CARGO_TARGET_DIR)" $(CARGO) build --release --workspace
	mkdir -p "$(FREQUENCY_BUNDLE_DIR)" "$(GATE_TRIGGER_BUNDLE_DIR)" "$(MIDI_BUNDLE_DIR)"
	cp plugins/guitar2frequency/guitar2frequency.lv2/*.ttl \
		"$(CARGO_TARGET_DIR)/release/libguitar2frequency.so" "$(FREQUENCY_BUNDLE_DIR)/"
	cp plugins/guitar2gatetrigger/guitar2gatetrigger.lv2/*.ttl \
		"$(CARGO_TARGET_DIR)/release/libguitar2gatetrigger.so" "$(GATE_TRIGGER_BUNDLE_DIR)/"
	cp plugins/guitar2midi/guitar2midi.lv2/*.ttl \
		"$(CARGO_TARGET_DIR)/release/libguitar2midi.so" "$(MIDI_BUNDLE_DIR)/"

$(PICO_FREQUENCY_DIR) $(PICO_GATE_TRIGGER_DIR) $(PICO_MIDI_DIR):
	mkdir -p "$@"

$(PICO_FREQUENCY_OBJECT): plugins/guitar2frequency/src/lib.rs dsp/frequency.rs | $(PICO_FREQUENCY_DIR)
	rustc --edition=2021 --target thumbv8m.main-none-eabihf --crate-type lib --emit=obj \
		-C panic=abort -C opt-level=2 -C overflow-checks=no -C relocation-model=pic \
		-o "$@" plugins/guitar2frequency/src/lib.rs

$(PICO_GATE_TRIGGER_OBJECT): plugins/guitar2gatetrigger/src/lib.rs dsp/gate_trigger.rs | $(PICO_GATE_TRIGGER_DIR)
	rustc --edition=2021 --target thumbv8m.main-none-eabihf --crate-type lib --emit=obj \
		-C panic=abort -C opt-level=2 -C overflow-checks=no -C relocation-model=pic \
		-o "$@" plugins/guitar2gatetrigger/src/lib.rs

$(PICO_MIDI_OBJECT): plugins/guitar2midi/src/lib.rs plugins/guitar2midi/src/midi.rs dsp/frequency.rs dsp/gate_trigger.rs | $(PICO_MIDI_DIR)
	rustc --edition=2021 --target thumbv8m.main-none-eabihf --crate-type lib --emit=obj \
		-C panic=abort -C opt-level=2 -C overflow-checks=no -C relocation-model=pic \
		-o "$@" plugins/guitar2midi/src/lib.rs

$(PICO_FREQUENCY_LIBRARY): $(PICO_FREQUENCY_OBJECT) $(PICOLV2_SDK_DIR)/src/runtime.c
	arm-none-eabi-gcc -mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16 \
		-shared -nostdlib -Wl,-Bsymbolic -Wl,-z,undefs \
		-Wl,-z,max-page-size=0x1000 -Wl,--no-warnings $(PICO_STRIP_FLAG) \
		-o "$@" $^ -Wl,--start-group -lc -lm -lnosys -lgcc -Wl,--end-group

$(PICO_GATE_TRIGGER_LIBRARY): $(PICO_GATE_TRIGGER_OBJECT) $(PICOLV2_SDK_DIR)/src/runtime.c
	arm-none-eabi-gcc -mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16 \
		-shared -nostdlib -Wl,-Bsymbolic -Wl,-z,undefs \
		-Wl,-z,max-page-size=0x1000 -Wl,--no-warnings $(PICO_STRIP_FLAG) \
		-o "$@" $^ -Wl,--start-group -lc -lm -lnosys -lgcc -Wl,--end-group

$(PICO_MIDI_LIBRARY): $(PICO_MIDI_OBJECT) $(PICOLV2_SDK_DIR)/src/runtime.c
	arm-none-eabi-gcc -mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16 \
		-shared -nostdlib -Wl,-Bsymbolic -Wl,-z,undefs \
		-Wl,-z,max-page-size=0x1000 -Wl,--no-warnings $(PICO_STRIP_FLAG) \
		-o "$@" $^ -Wl,--start-group -lc -lm -lnosys -lgcc -Wl,--end-group

bundle-pico: bundle-pico-frequency bundle-pico-gatetrigger bundle-pico-midi

bundle-pico-frequency: $(PICO_FREQUENCY_LIBRARY)
	mkdir -p "$(PICO_FREQUENCY_BUNDLE)"
	sed 's/libguitar2frequency.so/plugin.so/' plugins/guitar2frequency/guitar2frequency.lv2/manifest.ttl > "$(PICO_FREQUENCY_BUNDLE)/manifest.ttl"
	cp plugins/guitar2frequency/guitar2frequency.lv2/guitar2frequency.ttl "$(PICO_FREQUENCY_LIBRARY)" "$(PICO_FREQUENCY_BUNDLE)/"

bundle-pico-gatetrigger: $(PICO_GATE_TRIGGER_LIBRARY)
	mkdir -p "$(PICO_GATE_TRIGGER_BUNDLE)"
	sed 's/libguitar2gatetrigger.so/plugin.so/' plugins/guitar2gatetrigger/guitar2gatetrigger.lv2/manifest.ttl > "$(PICO_GATE_TRIGGER_BUNDLE)/manifest.ttl"
	cp plugins/guitar2gatetrigger/guitar2gatetrigger.lv2/guitar2gatetrigger.ttl "$(PICO_GATE_TRIGGER_LIBRARY)" "$(PICO_GATE_TRIGGER_BUNDLE)/"

bundle-pico-midi: $(PICO_MIDI_LIBRARY)
	mkdir -p "$(PICO_MIDI_BUNDLE)"
	sed 's/libguitar2midi.so/plugin.so/' plugins/guitar2midi/guitar2midi.lv2/manifest.ttl > "$(PICO_MIDI_BUNDLE)/manifest.ttl"
	cp plugins/guitar2midi/guitar2midi.lv2/guitar2midi.ttl "$(PICO_MIDI_LIBRARY)" "$(PICO_MIDI_BUNDLE)/"

clean:
	rm -rf build
