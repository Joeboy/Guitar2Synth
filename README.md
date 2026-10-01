# Guitar Synth Plugins

LV2 plugins to help convert a monophonic guitar signal into things a synth can
use. The current plugins output frequency (Hz), Gate, and Trigger. MIDI notes
may follow.

Designed around the assumption it'll need to run on very low-end hardware. If
you're using a decent computer there might be better things available.

Designed to work (reasonably) well with guitars, obviously you can plug other
things into it but YMMV.

## Guitar2Frequency

Converts a mono guitar input into a Frequency (Hz) output.

Outputs `0` when the signal is quiet or has no credible periodicity. The output
is updated once per LV2 `run` call; the detector calculates a new estimate every
64 downsampled samples.

The detector uses fixed arrays, `f32` arithmetic, a low-cost one-pole filter and
YIN difference search at approximately 8 kHz. The search spans 70–1300 Hz. It
uses no heap allocation during audio processing and keeps independent state for
each instance. Input sample rates from 8–384 kHz are accepted. Detection needs a
full 256-sample downsampled history (about 32 ms at 8 kHz), then updates every 8
ms. A float control output cannot represent within-block timing; that will
matter when adding MIDI events.

## Guitar2GateTrigger

Converts a mono guitar input into two sample-rate CV outputs: `gate` is 1 while
the input level is above the threshold, and `trigger` is a 5 ms pulse on the
first attack or a stronger new pluck while the gate remains open. Both are 0
otherwise. A 40 ms retrigger interval prevents the waveform from repeatedly
firing the trigger. The input level is tracked with a fast and a slow envelope;
processing uses fixed state and no heap allocation in the audio callback.

The `threshold` control defaults to 0.01 (relative to full-scale input) and can
be adjusted from 0.0005 to 0.2. Gate closes when the fast envelope falls below
half that threshold. Lower the threshold for a quiet input; raise it if noise
holds the gate open. In a modular patch, connect the same guitar audio input to
both plugins, then connect `gate` and `trigger` to the envelope's CV inputs.
The gate may open before Guitar2Frequency has produced its first estimate.

## Building

Build both desktop LV2 bundles in `build/`:

```sh
make
```

Set `BUNDLE_ROOT` to change the output location, for example
`make BUNDLE_ROOT="$HOME/.lv2"` to install both for the current user.

Build both PicoLV2 bundles at `build/picolv2/` with `make bundle-pico`. Set
`PICOLV2_SDK_DIR` to the PicoLV2 SDK directory if it is not at the Makefile's
default `../../sdk`, for example:

```sh
make bundle-pico PICOLV2_SDK_DIR=/path/to/picolv2/plugin-src/sdk
```

This renames the former Guitar2Pitch plugin, its LV2 URI, and its output symbol.
If you installed the old bundle, remove that installation and reconnect saved
graphs to Guitar2Frequency's `frequency_hz` output.

## Tests

```sh
cargo test
```

## AI declaration

Kinda vibecoded. I mean I am reading the code, but more in a casual
sanity-checking way than a careful line-by-line review way.
