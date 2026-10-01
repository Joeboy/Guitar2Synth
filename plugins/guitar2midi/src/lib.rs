//! Guitar2MIDI LV2 plugin.
#![cfg_attr(target_os = "none", no_std)]

#[cfg(target_os = "none")]
use core::panic::PanicInfo;
#[cfg(not(target_os = "none"))]
use std::boxed::Box;

use core::ffi::{c_char, c_void};
use core::ptr;

#[path = "../../../dsp/frequency.rs"]
mod frequency;
#[path = "../../../dsp/gate_trigger.rs"]
mod gate_trigger;
mod midi;
use midi::MidiTracker;

const URI: &[u8] = b"urn:guitarsynthplugins:Guitar2MIDI\0";
const URID_MAP_URI: &[u8] = b"http://lv2plug.in/ns/ext/urid#map\0";
const ATOM_SEQUENCE_URI: &[u8] = b"http://lv2plug.in/ns/ext/atom#Sequence\0";
const MIDI_EVENT_URI: &[u8] = b"http://lv2plug.in/ns/ext/midi#MidiEvent\0";

#[cfg(target_os = "none")]
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[repr(C)]
pub struct Lv2Feature {
    uri: *const c_char,
    data: *mut c_void,
}

#[repr(C)]
struct Lv2UridMap {
    handle: *mut c_void,
    map: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
}

#[repr(C)]
pub struct Lv2Descriptor {
    uri: *const c_char,
    instantiate: Option<
        unsafe extern "C" fn(
            *const Lv2Descriptor,
            f64,
            *const c_char,
            *const *const Lv2Feature,
        ) -> *mut c_void,
    >,
    connect_port: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void)>,
    activate: Option<unsafe extern "C" fn(*mut c_void)>,
    run: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    deactivate: Option<unsafe extern "C" fn(*mut c_void)>,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

unsafe impl Sync for Lv2Descriptor {}

#[repr(C)]
struct Atom {
    size: u32,
    atom_type: u32,
}

#[repr(C)]
struct AtomSequence {
    atom: Atom,
    unit: u32,
    pad: u32,
}

struct SequenceWriter {
    sequence: *mut AtomSequence,
    capacity: usize,
    used: usize,
    midi_urid: u32,
}

impl SequenceWriter {
    unsafe fn begin(
        sequence: *mut AtomSequence,
        sequence_urid: u32,
        midi_urid: u32,
    ) -> Option<Self> {
        if sequence.is_null() {
            return None;
        }
        let capacity = (*sequence).atom.size as usize;
        if capacity < 8 {
            return None;
        }
        (*sequence).atom.atom_type = sequence_urid;
        (*sequence).atom.size = 8;
        (*sequence).unit = 0;
        (*sequence).pad = 0;
        Some(Self {
            sequence,
            capacity,
            used: 0,
            midi_urid,
        })
    }

    unsafe fn write(&mut self, frame: usize, message: [u8; 3]) -> bool {
        const EVENT_BYTES: usize = 24;
        if 8 + self.used + EVENT_BYTES > self.capacity {
            return false;
        }
        let destination = (self.sequence as *mut u8).add(16 + self.used);
        ptr::write_bytes(destination, 0, EVENT_BYTES);
        ptr::write_unaligned(destination.cast::<i64>(), frame as i64);
        ptr::write_unaligned(destination.add(8).cast::<u32>(), 3);
        ptr::write_unaligned(destination.add(12).cast::<u32>(), self.midi_urid);
        ptr::copy_nonoverlapping(message.as_ptr(), destination.add(16), 3);
        self.used += EVENT_BYTES;
        (*self.sequence).atom.size = (8 + self.used) as u32;
        true
    }
}

struct Plugin {
    input: *const f32,
    threshold: *const f32,
    output: *mut AtomSequence,
    sequence_urid: u32,
    midi_urid: u32,
    tracker: MidiTracker,
}

unsafe fn uri_matches(actual: *const c_char, expected: &[u8]) -> bool {
    if actual.is_null() {
        return false;
    }
    for (index, byte) in expected.iter().enumerate() {
        if *actual.add(index) as u8 != *byte {
            return false;
        }
        if *byte == 0 {
            return true;
        }
    }
    false
}

unsafe extern "C" fn instantiate(
    _descriptor: *const Lv2Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    features: *const *const Lv2Feature,
) -> *mut c_void {
    let Some(tracker) = MidiTracker::new(sample_rate as f32) else {
        return ptr::null_mut();
    };
    if features.is_null() {
        return ptr::null_mut();
    }
    let mut urid_map = ptr::null::<Lv2UridMap>();
    let mut index = 0;
    while !(*features.add(index)).is_null() {
        let feature = &**features.add(index);
        if uri_matches(feature.uri, URID_MAP_URI) {
            urid_map = feature.data.cast();
            break;
        }
        index += 1;
    }
    if urid_map.is_null() {
        return ptr::null_mut();
    }
    let sequence_urid = ((*urid_map).map)((*urid_map).handle, ATOM_SEQUENCE_URI.as_ptr().cast());
    let midi_urid = ((*urid_map).map)((*urid_map).handle, MIDI_EVENT_URI.as_ptr().cast());
    if sequence_urid == 0 || midi_urid == 0 {
        return ptr::null_mut();
    }
    let plugin = Plugin {
        input: ptr::null(),
        threshold: ptr::null(),
        output: ptr::null_mut(),
        sequence_urid,
        midi_urid,
        tracker,
    };
    #[cfg(target_os = "none")]
    {
        let memory = malloc(core::mem::size_of::<Plugin>()).cast::<Plugin>();
        if memory.is_null() {
            return ptr::null_mut();
        }
        ptr::write(memory, plugin);
        memory.cast()
    }
    #[cfg(not(target_os = "none"))]
    {
        Box::into_raw(Box::new(plugin)).cast()
    }
}

unsafe extern "C" fn connect_port(instance: *mut c_void, port: u32, data: *mut c_void) {
    let plugin = &mut *(instance as *mut Plugin);
    match port {
        0 => plugin.input = data.cast(),
        1 => plugin.threshold = data.cast(),
        2 => plugin.output = data.cast(),
        _ => {}
    }
}

unsafe extern "C" fn activate(instance: *mut c_void) {
    (&mut *(instance as *mut Plugin)).tracker.reset();
}

unsafe extern "C" fn run(instance: *mut c_void, sample_count: u32) {
    let plugin = &mut *(instance as *mut Plugin);
    let threshold = if plugin.threshold.is_null() {
        0.01
    } else {
        *plugin.threshold
    };
    plugin.tracker.set_threshold(threshold);
    let mut writer = SequenceWriter::begin(plugin.output, plugin.sequence_urid, plugin.midi_urid);
    for frame in 0..sample_count as usize {
        let sample = if plugin.input.is_null() {
            0.0
        } else {
            *plugin.input.add(frame)
        };
        plugin.tracker.push(sample, |message| {
            writer
                .as_mut()
                .is_some_and(|output| unsafe { output.write(frame, message) })
        });
    }
}

unsafe extern "C" fn cleanup(instance: *mut c_void) {
    #[cfg(target_os = "none")]
    {
        ptr::drop_in_place(instance.cast::<Plugin>());
        free(instance);
    }
    #[cfg(not(target_os = "none"))]
    {
        drop(Box::from_raw(instance.cast::<Plugin>()));
    }
}

unsafe extern "C" fn extension_data(_uri: *const c_char) -> *const c_void {
    ptr::null()
}

static DESCRIPTOR: Lv2Descriptor = Lv2Descriptor {
    uri: URI.as_ptr().cast(),
    instantiate: Some(instantiate),
    connect_port: Some(connect_port),
    activate: Some(activate),
    run: Some(run),
    deactivate: None,
    cleanup: Some(cleanup),
    extension_data: Some(extension_data),
};

#[no_mangle]
pub extern "C" fn lv2_descriptor(index: u32) -> *const Lv2Descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        ptr::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn map_uri(_handle: *mut c_void, uri: *const c_char) -> u32 {
        if uri_matches(uri, ATOM_SEQUENCE_URI) {
            11
        } else if uri_matches(uri, MIDI_EVENT_URI) {
            22
        } else {
            0
        }
    }

    #[test]
    fn atom_writer_respects_capacity_and_layout() {
        let mut storage = [0u64; 8];
        let sequence = storage.as_mut_ptr().cast::<AtomSequence>();
        unsafe {
            (*sequence).atom.size = 56;
            let mut writer = SequenceWriter::begin(sequence, 11, 22).unwrap();
            assert!(writer.write(17, [0x90, 69, 100]));
            assert!(writer.write(23, [0x80, 69, 0]));
            assert!(!writer.write(24, [0x90, 70, 100]));
            assert_eq!((*sequence).atom.size, 56);
            assert_eq!((*sequence).atom.atom_type, 11);
            let bytes = storage.as_ptr().cast::<u8>();
            assert_eq!(ptr::read_unaligned(bytes.add(16).cast::<i64>()), 17);
            assert_eq!(ptr::read_unaligned(bytes.add(28).cast::<u32>()), 22);
            assert_eq!(*bytes.add(32), 0x90);
            assert_eq!(*bytes.add(33), 69);
            assert_eq!(*bytes.add(34), 100);
            assert_eq!(ptr::read_unaligned(bytes.add(40).cast::<i64>()), 23);
        }
    }

    #[test]
    fn lv2_emits_timed_note_on_and_off() {
        unsafe {
            let descriptor = lv2_descriptor(0);
            assert!(!descriptor.is_null());
            assert!(lv2_descriptor(1).is_null());
            let mut map = Lv2UridMap {
                handle: ptr::null_mut(),
                map: map_uri,
            };
            let feature = Lv2Feature {
                uri: URID_MAP_URI.as_ptr().cast(),
                data: (&mut map as *mut Lv2UridMap).cast(),
            };
            let features = [&feature as *const Lv2Feature, ptr::null()];
            let instance = ((*descriptor).instantiate.unwrap())(
                descriptor,
                48_000.0,
                ptr::null(),
                features.as_ptr(),
            );
            assert!(!instance.is_null());
            ((*descriptor).activate.unwrap())(instance);

            let mut input = [0.0f32; 256];
            let block_len = input.len();
            let threshold = 0.01f32;
            let mut storage = [0u64; 32];
            let sequence = storage.as_mut_ptr().cast::<AtomSequence>();
            ((*descriptor).connect_port.unwrap())(instance, 0, input.as_mut_ptr().cast());
            ((*descriptor).connect_port.unwrap())(
                instance,
                1,
                (&threshold as *const f32).cast_mut().cast(),
            );
            ((*descriptor).connect_port.unwrap())(instance, 2, sequence.cast());

            let mut events = std::vec::Vec::new();
            for block in 0..56usize {
                for (index, sample) in input.iter_mut().enumerate() {
                    let absolute_frame = block * block_len + index;
                    *sample = if absolute_frame < 48_000 / 5 {
                        0.4 * (core::f32::consts::TAU * 220.0 * absolute_frame as f32 / 48_000.0)
                            .sin()
                    } else {
                        0.0
                    };
                }
                (*sequence).atom.size = (core::mem::size_of_val(&storage) - 8) as u32;
                ((*descriptor).run.unwrap())(instance, block_len as u32);
                assert_eq!((*sequence).atom.atom_type, 11);
                let mut offset = 16usize;
                while offset < 8 + (*sequence).atom.size as usize {
                    let bytes = storage.as_ptr().cast::<u8>().add(offset);
                    let frame = ptr::read_unaligned(bytes.cast::<i64>());
                    let size = ptr::read_unaligned(bytes.add(8).cast::<u32>());
                    let atom_type = ptr::read_unaligned(bytes.add(12).cast::<u32>());
                    assert!((0..256).contains(&frame));
                    assert_eq!(size, 3);
                    assert_eq!(atom_type, 22);
                    events.push([*bytes.add(16), *bytes.add(17), *bytes.add(18)]);
                    offset += 24;
                }
            }
            assert_eq!(events, [[0x90, 57, 100], [0x80, 57, 0]]);
            ((*descriptor).cleanup.unwrap())(instance);
        }
    }
}
