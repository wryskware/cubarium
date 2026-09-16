//! Q1a: does the Vulkan loader find an ICD on this board, and what does the
//! physical device advertise that a headless + dma-buf scanout path needs?
use std::ffi::CStr;

use anyhow::{Context, Result};
use ash::vk;

fn cstr(bytes: &[std::ffi::c_char]) -> String {
    unsafe { CStr::from_ptr(bytes.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn main() -> Result<()> {
    let entry = unsafe { ash::Entry::load() }.context("load libvulkan.so.1")?;

    match unsafe { entry.try_enumerate_instance_version() }? {
        Some(v) => println!(
            "loader instance version: {}.{}.{}",
            vk::api_version_major(v),
            vk::api_version_minor(v),
            vk::api_version_patch(v)
        ),
        None => println!("loader instance version: 1.0"),
    }

    let iexts = unsafe { entry.enumerate_instance_extension_properties(None) }?;
    println!("instance extensions ({}):", iexts.len());
    for e in &iexts {
        println!("  {}", cstr(&e.extension_name));
    }

    // Everything the dma-buf export path wants, at instance scope.
    let want_inst = [
        c"VK_KHR_get_physical_device_properties2",
        c"VK_KHR_external_memory_capabilities",
    ];
    let have: Vec<_> = iexts.iter().map(|e| cstr(&e.extension_name)).collect();
    let enable: Vec<*const std::ffi::c_char> = want_inst
        .iter()
        .filter(|n| have.iter().any(|h| h.as_str() == n.to_str().unwrap()))
        .map(|n| n.as_ptr())
        .collect();

    let app = vk::ApplicationInfo::default()
        .application_name(c"gpu-scanout-probe")
        .api_version(vk::make_api_version(0, 1, 1, 0));
    let ci = vk::InstanceCreateInfo::default()
        .application_info(&app)
        .enabled_extension_names(&enable);
    let instance = unsafe { entry.create_instance(&ci, None) }.context("vkCreateInstance")?;

    let pds = unsafe { instance.enumerate_physical_devices() }?;
    println!("\nphysical devices: {}", pds.len());
    for pd in pds {
        let p = unsafe { instance.get_physical_device_properties(pd) };
        println!(
            "  name={:?} type={:?} api={}.{}.{} driver=0x{:x} vendor=0x{:x} device=0x{:x}",
            cstr(&p.device_name),
            p.device_type,
            vk::api_version_major(p.api_version),
            vk::api_version_minor(p.api_version),
            vk::api_version_patch(p.api_version),
            p.driver_version,
            p.vendor_id,
            p.device_id
        );
        println!(
            "  maxImageDimension2D={} maxComputeWorkGroupInvocations={}",
            p.limits.max_image_dimension2_d, p.limits.max_compute_work_group_invocations
        );

        let qs = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        for (i, q) in qs.iter().enumerate() {
            println!("  queue[{i}] count={} flags={:?}", q.queue_count, q.queue_flags);
        }

        let mem = unsafe { instance.get_physical_device_memory_properties(pd) };
        for i in 0..mem.memory_type_count as usize {
            let t = mem.memory_types[i];
            println!(
                "  memtype[{i}] heap={} flags={:?}",
                t.heap_index, t.property_flags
            );
        }

        let dexts = unsafe { instance.enumerate_device_extension_properties(pd) }?;
        let names: Vec<String> = dexts.iter().map(|e| cstr(&e.extension_name)).collect();
        println!("  device extensions ({}):", names.len());
        for n in &names {
            println!("    {n}");
        }
        for need in [
            "VK_KHR_external_memory",
            "VK_KHR_external_memory_fd",
            "VK_EXT_external_memory_dma_buf",
            "VK_EXT_image_drm_format_modifier",
            "VK_EXT_queue_family_foreign",
            "VK_KHR_bind_memory2",
            "VK_KHR_image_format_list",
            "VK_KHR_sampler_ycbcr_conversion",
            "VK_KHR_maintenance1",
            "VK_KHR_timeline_semaphore",
            "VK_KHR_external_fence_fd",
            "VK_KHR_external_semaphore_fd",
        ] {
            println!(
                "  [{}] {need}",
                if names.iter().any(|n| n == need) { "YES" } else { "no " }
            );
        }
    }

    unsafe { instance.destroy_instance(None) };
    Ok(())
}
