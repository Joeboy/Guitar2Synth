//! Guitar2GateTrigger LV2 plugin.
#![cfg_attr(target_os = "none", no_std)]

#[cfg(target_os = "none")]
use core::panic::PanicInfo;
#[cfg(not(target_os = "none"))]
use std::boxed::Box;

#[path = "../../../dsp/gate_trigger.rs"]
mod detector;

use core::ffi::{c_char, c_void};
use core::ptr;
use detector::GateTriggerDetector;

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

const URI: &[u8] = b"urn:guitarsynthplugins:Guitar2GateTrigger\0";

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

unsafe impl Sync for Lv2Descriptor {}

struct Plugin {
    input: *const f32,
    threshold: *const f32,
    gate: *mut f32,
    trigger: *mut f32,
    detector: GateTriggerDetector,
}

unsafe extern "C" fn instantiate(
    _descriptor: *const Lv2Descriptor,
    sample_rate: f64,
    _bundle_path: *const c_char,
    _features: *const *const Lv2Feature,
) -> *mut c_void {
    let Some(detector) = GateTriggerDetector::new(sample_rate as f32) else {
        return ptr::null_mut();
    };
    let plugin = Plugin {
        input: ptr::null(),
        threshold: ptr::null(),
        gate: ptr::null_mut(),
        trigger: ptr::null_mut(),
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
        1 => plugin.threshold = data as *const f32,
        2 => plugin.gate = data as *mut f32,
        3 => plugin.trigger = data as *mut f32,
        _ => {}
    }
}

unsafe extern "C" fn activate(instance: *mut c_void) {
    (&mut *(instance as *mut Plugin)).detector.reset();
}

unsafe extern "C" fn run(instance: *mut c_void, sample_count: u32) {
    let plugin = &mut *(instance as *mut Plugin);
    let threshold = if plugin.threshold.is_null() {
        0.01
    } else {
        *plugin.threshold
    };
    plugin.detector.set_threshold(threshold);
    for i in 0..sample_count as usize {
        let input = if plugin.input.is_null() {
            0.0
        } else {
            *plugin.input.add(i)
        };
        let (gate, trigger) = plugin.detector.push(input);
        if !plugin.gate.is_null() {
            *plugin.gate.add(i) = gate;
        }
        if !plugin.trigger.is_null() {
            *plugin.trigger.add(i) = trigger;
        }
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
    fn lv2_outputs_cv_across_variable_blocks() {
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
            ((*descriptor).activate.unwrap())(instance);
            let threshold = 0.01f32;
            ((*descriptor).connect_port.unwrap())(
                instance,
                1,
                (&threshold as *const f32).cast_mut().cast(),
            );
            let mut phase = 0.0f32;
            let mut saw_gate = false;
            let mut saw_trigger = false;
            for size in [37usize, 101, 64, 256] {
                let mut input = std::vec![0.0f32; size];
                let mut gate = std::vec![-1.0f32; size];
                let mut trigger = std::vec![-1.0f32; size];
                ((*descriptor).connect_port.unwrap())(instance, 0, input.as_mut_ptr().cast());
                ((*descriptor).connect_port.unwrap())(instance, 2, gate.as_mut_ptr().cast());
                ((*descriptor).connect_port.unwrap())(instance, 3, trigger.as_mut_ptr().cast());
                for _ in 0..16 {
                    for sample in &mut input {
                        *sample = 0.3 * phase.sin();
                        phase += core::f32::consts::TAU * 110.0 / 48_000.0;
                    }
                    ((*descriptor).run.unwrap())(instance, size as u32);
                    assert!(gate.iter().all(|v| *v == 0.0 || *v == 1.0));
                    assert!(trigger.iter().all(|v| *v == 0.0 || *v == 1.0));
                    saw_gate |= gate.contains(&1.0);
                    saw_trigger |= trigger.contains(&1.0);
                }
            }
            assert!(saw_gate && saw_trigger);
            ((*descriptor).cleanup.unwrap())(instance);
        }
    }
}
