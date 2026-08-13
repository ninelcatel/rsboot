// load a .efi into memory and then run it, keeps write/text format as before
// qemu was set up with the root file system at /esp , a valid path for example would be
// \\efi\\boot\\hello.efi

use uefi::{
    Identify,
    boot::{ScopedProtocol, load_image, start_image},
};

use crate::{
    downloader::IsoBuffer,
    environment::{self, BootConfig, BootMethod},
    handlers::*,
};

extern crate alloc;

const BOOT_FILE: &uefi::CStr16 = uefi::cstr16!("\\EFI\\BOOT\\BOOTX64.EFI");
const RAM_DISK_DXE: &[u8] = include_bytes!("../assets/RamDiskDxe.efi");

// make the appended hook run first
const GENTOO_HOOK: &str = include_str!("../assets/gentoo.sh");
const HOOK_PATH: &str = "usr/lib/dracut/hooks/pre-trigger/00-rsboot.sh";

const CONFIGS: [&str; 3] = [
    "boot/syslinux/archiso_sys-linux.cfg", // arch / cachy / blackarch (syslinux)
    "boot/grub/grub.cfg",                  // gentoo etc (grub)
    "EFI/BOOT/grub.cfg",                   // opensuse (grub, efi-only)
];
pub fn boot_from_iso(
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

// load a PE/EFI image already sitting in RAM and return its handle, ready to start
fn load_from_buffer(parent: uefi::Handle, buffer: &[u8]) -> uefi::Result<uefi::Handle> {
    load_image(
        parent,
        uefi::boot::LoadImageSource::FromBuffer {
            buffer,
            file_path: None,
        },
    )
}


// returns paths as Strings, stopped returning CStr16 due to the new ISO9660 parser using String
fn parse_config(config: alloc::vec::Vec<u8>) -> Option<BootConfig> {
    use alloc::string::ToString;

    let text = core::str::from_utf8(&config).unwrap_or("");
    // first entry only: is_none() condition
    // configs list several menu entries,
    let (mut kernel, mut initrd, mut options, mut inline_opts) = (None, None, None, None);

    for line in text.lines() {
        let l = line.trim();
        if kernel.is_none()
            && let Some(s) = l
                .strip_prefix("linux ") // systemd-boot / grub
                .or_else(|| l.strip_prefix("LINUX ")) // syslinux
                .or_else(|| l.strip_prefix("KERNEL "))
        // isolinux
        {
            // grub combines "linux <path> <options>"; syslinux/systemd-boot give just the path
            match s.trim().split_once(char::is_whitespace) {
                Some((path, rest)) => {
                    kernel = Some(path.to_string());
                    inline_opts = Some(rest.trim().to_string());
                }
                None => kernel = Some(s.trim().to_string()),
            }
        }
        if initrd.is_none()
            && let Some(s) = l
                .strip_prefix("initrd ") // systemd-boot / grub
                .or_else(|| l.strip_prefix("INITRD "))
        // syslinux/isolinux
        {
            initrd = Some(s.trim().to_string());
        }
        if options.is_none()
            && let Some(s) = l
                .strip_prefix("options ") // systemd-boot
                .or_else(|| l.strip_prefix("APPEND "))
        // syslinux/isolinux
        {
            options = Some(s.trim().to_string());
        }
    }
    Some(BootConfig {
        kernel_path: kernel?,
        initrd_path: initrd?,
        cmdline: options.or(inline_opts)?,
    })
}

fn read_iso(iso: &IsoBuffer, path: &str) -> Option<alloc::vec::Vec<u8>> {
    let bytes = iso.as_slice();
    let cursor = hadris_io::Cursor::new(bytes);

    let img = hadris_iso::sync::IsoImage::open(cursor).ok()?;
    let mut dir_ref = img.root_dir().dir_ref();
    let mut parts = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty()) // checks for paths//like//this
        .peekable(); // secventially
    // go through the directories, thats how the parser works
    while let Some(part) = parts.next() {
        let dir = img.open_dir(dir_ref);
        let entry = dir
            .entries()
            .filter_map(|e| e.ok())
            .find(|e| e.display_name().eq_ignore_ascii_case(part))?;
        if parts.peek().is_none() {
            // if there arent directories left to navigate, it means we are
            // at the destination file
            let ext = entry.extents().next()?;
            let start = ext.sector.0 << 11;
            let len = ext.length as usize;
            return Some(bytes.get(start..start + len)?.to_vec());
        }
        dir_ref = entry.as_dir_ref(&img).ok()?;
    }
    None
}

fn build_cpio(files: &[(&str, &[u8], u32)]) -> alloc::vec::Vec<u8> {
    fn record(out: &mut alloc::vec::Vec<u8>, ino: u32, mode: u32, name: &str, data: &[u8]) {
        let header = alloc::format!(
            "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            ino,
            mode,
            0,
            0,
            1,
            0,
            data.len(),
            0,
            0,
            0,
            0,
            name.len() + 1,
            0
        );
        // header format containing metadata
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(0); // \0 end of line
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out.extend_from_slice(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }

    let mut out = alloc::vec::Vec::new();
    let mut ino = 1u32;
    let mut added: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for &(path, data, mode) in files {
        let mut prefix = alloc::string::String::new();
        let comps: alloc::vec::Vec<&str> = path.split('/').collect();
        for comp in &comps[..comps.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(comp);
            if !added.contains(&prefix) {
                record(&mut out, ino, 0o040755, &prefix, &[]); // its a directory, no data 
                ino += 1;
                added.push(prefix.clone())
            }
        }
        record(&mut out, ino, mode, path, data);
        ino += 1;
    }
    record(&mut out, ino, 0, "TRAILER!!!", &[]); //EOF marker
    out
}

fn locate_ramdisk(handler: uefi::Handle) -> uefi::Result<uefi::Handle> {
    // locate the ram disk protocol by its guid, then open it to call register
    let mut ramdisk_handlers =
        uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID));
    if ramdisk_handlers.is_err() {
        let ram_disk_dxe_instance = load_from_buffer(handler, RAM_DISK_DXE)?;
        uefi::boot::start_image(ram_disk_dxe_instance)?;
        ramdisk_handlers =
            uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID));
    }

    // the ramdisk protocol handle is installed only once in the uefi system, so
    // .first() is enough and we dont need to iterate through the buffer
    let ramdisk_handle = *ramdisk_handlers?.first().ok_or(uefi::Status::NOT_FOUND)?;
    Ok(ramdisk_handle)
}

// a virtual CD that OWNS the iso pages. iso pages lifetime are linked to
// the registration. on a failed boot Drop disconnects the FAT driver, unregisters, then frees or
// leaks the pages (leaking is a fallback to not lead to UB)
struct VirtualCd {
    block_handle: Option<uefi::Handle>,
    unregister: unsafe extern "efiapi" fn(*const EFI_DEVICE_PATH) -> uefi::Status,
    dp: *const EFI_DEVICE_PATH,
    iso: Option<IsoBuffer>,
}

impl VirtualCd {
    fn device_path(&self) -> &uefi::proto::device_path::DevicePath {
        unsafe { uefi::proto::device_path::DevicePath::from_ffi_ptr(self.dp.cast()) }
    }
}

impl Drop for VirtualCd {
    fn drop(&mut self) {
        // order matters disconnect the FAT driver we bound so the firmware can
        // release the pages, THEN unregister, THEN free.
        if let Some(handle) = self.block_handle {
            let _ = uefi::boot::disconnect_controller(handle, None, None);
        }
        let status = unsafe { (self.unregister)(self.dp) };
        if !status.is_success() {
            core::mem::forget(self.iso.take()); // FALLBACK TO NOT LEAD TO UB 
        } else {
            drop(self.iso.take());
        }
    }
}

fn register_virtual_cd(
    ram_disk: &ScopedProtocol<EFI_RAM_DISK_PROTOCOL>,
    iso: IsoBuffer,
) -> uefi::Result<VirtualCd> {
    let base = iso.as_ptr() as u64;
    let size = iso.len() as u64;
    let mut ram_devicepath_ptr: *const EFI_DEVICE_PATH = core::ptr::null();
    let status = unsafe {
        (ram_disk.register)(
            base,
            size,
            &VIRTUAL_CD_GUID,  // we register the type as a virtual CD
            core::ptr::null(), // optional in the edk2 definition
            &mut ram_devicepath_ptr, // output address to UEFI device path definition, needed for
                               // locating the filesystem via other protocols
        )
    };

    if !status.is_success() {
        log::error!("ram disk protocol register failed: {status}");
        return Err(status.into());
    }

    let device_path =
        unsafe { uefi::proto::device_path::DevicePath::from_ffi_ptr(ram_devicepath_ptr.cast()) };
    let mut remaining: &uefi::proto::device_path::DevicePath = device_path;
    let block_handle =
        uefi::boot::locate_device_path::<uefi::proto::media::block::BlockIO>(&mut remaining).ok();

    Ok(VirtualCd {
        block_handle,
        unregister: ram_disk.unregister,
        dp: ram_devicepath_ptr,
        iso: Some(iso),
    })
}

fn find_fat_fs(
    ram_devicepath: &uefi::proto::device_path::DevicePath,
) -> uefi::Result<uefi::Handle> {
    let mut remaining = ram_devicepath;
    if let Ok(disk_handle) =
        uefi::boot::locate_device_path::<uefi::proto::media::block::BlockIO>(&mut remaining)
    {
        uefi::boot::connect_controller(disk_handle, &[], None, true)?;
    }

    // find the filesystem that lives on the ram disk
    let ram_bytes = ram_devicepath.as_bytes();
    let prefix = &ram_bytes[..ram_bytes.len().saturating_sub(4)];
    //slice containing all the bytes except last 4 , last 4 bytes are just marking the end making it
    // impossible to find the actual filesystem handle (it queries STARTS_WITH)

    let handles_found = uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(
        &uefi::proto::media::fs::SimpleFileSystem::GUID,
    ))?;
    let mut fs_handle = None;
    for &h in handles_found.iter() {
        // most likely we have  multiple file systems, use
        // the handler found that coincides with the iso device path
        if let Ok(dp) =
            uefi::boot::open_protocol_exclusive::<uefi::proto::device_path::DevicePath>(h)
            && dp.as_bytes().starts_with(prefix)
        {
            fs_handle = Some(h);
            break;
        }
    }
    let fs_handle = fs_handle.ok_or_else(|| {
        log::error!("no EFI filesystem found on the iso");
        uefi::Status::NOT_FOUND
    })?; // might fail from older iso images that don't have UEFI adaptation
    Ok(fs_handle)
}

// build <fs>\EFI\BOOT\BOOTX64.EFI
fn load_boot_file(
    fs_handle: uefi::Handle,
    parent_image: uefi::Handle,
) -> uefi::Result<uefi::Handle> {
    let fs_devicepath =
        uefi::boot::open_protocol_exclusive::<uefi::proto::device_path::DevicePath>(fs_handle)?;
    let mut buf = alloc::vec::Vec::new();
    let mut builder = uefi::proto::device_path::build::DevicePathBuilder::with_vec(&mut buf);

    for node in fs_devicepath.node_iter() {
        builder = builder
            .push(&node)
            .map_err(|_| uefi::Status::DEVICE_ERROR)?;
    }

    let full_path = builder
        .push(&uefi::proto::device_path::build::media::FilePath {
            path_name: BOOT_FILE,
        })
        .map_err(|_| uefi::Status::DEVICE_ERROR)?
        .finalize()
        .map_err(|_| uefi::Status::DEVICE_ERROR)?; // too lazy to treat these results

    let instance = uefi::boot::load_image(
        parent_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: full_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )?;
    Ok(instance)
}
