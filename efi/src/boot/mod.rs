extern crate alloc;

mod cpio;
mod ramdisk;
mod readcfg;

use uefi::boot::{load_image, start_image};

use crate::{
    environment::{self, BootMethod},
    handlers::{EFI_RAM_DISK_PROTOCOL, install_initrd},
};

use cpio::{GENTOO_HOOK, HOOK_PATH, build_cpio};
use ramdisk::{find_fat_fs, load_boot_file, locate_ramdisk, register_virtual_cd};
use readcfg::{CONFIGS, parse_config, read_iso};

pub fn boot(
    iso: crate::downloader::IsoBuffer,
    boot_method: environment::BootMethod,
) -> uefi::Result {
    let handler = uefi::boot::image_handle();

    match boot_method {
        BootMethod::RamDisk => {
            // the iso is already allocated, so we only register those pages as a virtual cd,
            // then find FAT fs and load its bootloader
            let ramdisk_handle = locate_ramdisk(handler)?;
            let ram_disk =
                uefi::boot::open_protocol_exclusive::<EFI_RAM_DISK_PROTOCOL>(ramdisk_handle)?;

            let vcd = register_virtual_cd(&ram_disk, iso)?;
            let fs_handle = find_fat_fs(vcd.device_path())?;
            let instance = load_boot_file(fs_handle, handler)?;

            start_image(instance)
        }
        //same kernel + initrd from the ISO logic.
        // only initrd and cmdline differs
        BootMethod::Memmap | BootMethod::LoopInjection => {
            let cfg = CONFIGS
                .iter()
                .find_map(|p| read_iso(&iso, p))
                .ok_or(uefi::Status::NOT_FOUND)?;
            let boot_cfg = parse_config(cfg).ok_or(uefi::Status::NOT_FOUND)?;
            let kernel_bytes =
                read_iso(&iso, &boot_cfg.kernel_path).ok_or(uefi::Status::NOT_FOUND)?;
            let mut initrd =
                read_iso(&iso, &boot_cfg.initrd_path).ok_or(uefi::Status::NOT_FOUND)?;

            let cmdline = match boot_method {
                BootMethod::LoopInjection => {
                    // add cpio as new initramfs segment
                    let iso_bytes = iso.as_slice();
                    while !initrd.len().is_multiple_of(4) {
                        initrd.push(0);
                    }
                    initrd.extend_from_slice(&build_cpio(&[
                        ("rsboot.iso", iso_bytes, 0o100644), // 100 - regular file 644 - chmod permissions
                        (HOOK_PATH, GENTOO_HOOK.as_bytes(), 0o100755), // 100 755 - chmod permisions
                    ]));
                    boot_cfg.cmdline
                }
                BootMethod::Memmap => {
                    let options = boot_cfg.cmdline;
                    alloc::format!(
                        "{options} memmap={:#x}!{:#x}",
                        iso.mapped_len(),
                        iso.as_ptr() as usize,
                    )
                }
                _ => unreachable!("this block reaches only  memmap or loop injection"),
            };

            let instance = load_from_buffer(handler, &kernel_bytes)?;
            install_initrd(initrd)?;

            let cmdline = uefi::CString16::try_from(cmdline.as_str())
                .map_err(|_| uefi::Status::INVALID_PARAMETER)?;
            unsafe {
                let mut loaded_image = uefi::boot::open_protocol_exclusive::<
                    uefi::proto::loaded_image::LoadedImage,
                >(instance)?;
                loaded_image.set_load_options(cmdline.as_ptr().cast(), cmdline.num_bytes() as u32);
            }
            start_image(instance)
        }
        BootMethod::Netboot => {
            let image = iso.as_slice();
            let instance = load_from_buffer(handler, image)?;
            start_image(instance)
        }
    }
}

// load a EFI image already sitting in RAM and return its handle
fn load_from_buffer(parent: uefi::Handle, buffer: &[u8]) -> uefi::Result<uefi::Handle> {
    load_image(
        parent,
        uefi::boot::LoadImageSource::FromBuffer {
            buffer,
            file_path: None,
        },
    )
}
