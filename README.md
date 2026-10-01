# Guitar Synth Plugins

LV2 plugins to help convert a monophonic guitar signal into things a synth can
use. For now, there's just frequency (Hz). Coming soon (I hope) are Gate,
Trigger and MIDI notes.

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

## Building

Build a desktop LV2 bundle at `build/guitar2frequency.lv2`:

```sh
make
```

Set `BUNDLE_DIR` to change the output location, for example
`make BUNDLE_DIR="$HOME/.lv2/guitar2frequency.lv2"` to install it for the current
user.

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
