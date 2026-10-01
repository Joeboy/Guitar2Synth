//! Guitar2Frequency LV2 plugin.
#![cfg_attr(target_os = "none", no_std)]

#[cfg(target_os = "none")]
use core::panic::PanicInfo;
#[cfg(not(target_os = "none"))]
use std::boxed::Box;

#[path = "../../../dsp/frequency.rs"]
mod frequency;
use frequency::FrequencyDetector;

use core::ffi::{c_char, c_void};
use core::ptr;

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

const URI: &[u8] = b"urn:guitarsynthplugins:Guitar2Frequency\0";

#[repr(C)]
pub struct Lv2Feature {
    uri: *const c_char,
    data: *mut c_void,
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

// A descriptor is immutable, and the URI points to immutable static storage.
unsafe impl Sync for Lv2Descriptor {}

struct Plugin {
    input: *const f32,
    output: *mut f32,
    detector: FrequencyDetector,
}

unsafe extern "C" fn instantiate(
    _descriptor: *const Lv2Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    _features: *const *const Lv2Feature,
) -> *mut c_void {
    let Some(detector) = FrequencyDetector::new(sample_rate as f32) else {
        return ptr::null_mut();
    };
    let plugin = Plugin {
        input: ptr::null(),
        output: ptr::null_mut(),
        detector,
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
        0 => plugin.input = data as *const f32,
        1 => plugin.output = data as *mut f32,
        _ => {}
    }
}

unsafe extern "C" fn activate(instance: *mut c_void) {
    (&mut *(instance as *mut Plugin)).detector.reset();
}

unsafe extern "C" fn run(instance: *mut c_void, sample_count: u32) {
    let plugin = &mut *(instance as *mut Plugin);
    if plugin.output.is_null() {
        return;
    }
    if !plugin.input.is_null() {
        for i in 0..sample_count as usize {
            plugin.detector.push(*plugin.input.add(i));
        }
    }
    *plugin.output = plugin.detector.frequency_hz();
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
    uri: URI.as_ptr() as *const c_char,
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

    #[test]
    fn lv2_entry_points_handle_variable_blocks() {
        unsafe {
            let descriptor = lv2_descriptor(0);
            assert!(!descriptor.is_null());
            assert!(lv2_descriptor(1).is_null());
            let features = [ptr::null()];
            let instance = ((*descriptor).instantiate.unwrap())(
                descriptor,
                48_000.0,
                ptr::null(),
                features.as_ptr(),
            );
            assert!(!instance.is_null());
            let mut output = -1.0f32;
            ((*descriptor).connect_port.unwrap())(instance, 1, (&mut output as *mut f32).cast());
            ((*descriptor).activate.unwrap())(instance);
            ((*descriptor).run.unwrap())(instance, 0);
            assert_eq!(output, 0.0);

            let mut phase = 0.0f32;
            for size in [37usize, 64, 101, 256] {
                let mut input = std::vec![0.0f32; size];
                ((*descriptor).connect_port.unwrap())(instance, 0, input.as_mut_ptr().cast());
                for _ in 0..16 {
                    for sample in &mut input {
                        *sample = 0.5 * phase.sin();
                        phase += core::f32::consts::TAU * 220.0 / 48_000.0;
                    }
                    ((*descriptor).run.unwrap())(instance, size as u32);
                }
            }
            assert!((output - 220.0).abs() < 2.0, "output={output}");
            ((*descriptor).cleanup.unwrap())(instance);
        }
    }
}
