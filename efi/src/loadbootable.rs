// load a .efi into memory and then run it, keeps write/text format as before
// qemu was set up with the root file system at /esp , a valid path for example would be
// \\efi\\boot\\hello.efi

use uefi::{
    Identify,
    boot::{load_image, start_image},
};

use crate::{
    downloader::IsoBuffer,
    environment::{self, BootMethod},
};

extern crate alloc;

#[repr(C)]
struct EFI_DEVICE_PATH {
    r#type: u8, // media device path : example 0x01 = Hardware, 0x02 = ACPI
    sub_type: u8,
    length: [u8; 2], // total bytes, header + payload, LITTLE ENDIAN
}

#[repr(C)]
struct EFI_RAM_DISK_PROTOCOL {
    register: unsafe extern "efiapi" fn(
        ram_disk_base: u64,
        ram_disk_size: u64,
        ram_disk_type: *const uefi::Guid,
        parent_device_path: *const EFI_DEVICE_PATH,
        device_path: *mut *const EFI_DEVICE_PATH,
    ) -> uefi::Status,
    unregister: unsafe extern "efiapi" fn(device_path: *const EFI_DEVICE_PATH) -> uefi::Status,
}

const RAM_DISK_GUID: uefi::Guid = uefi::guid!("ab38a0df-6873-44a9-87e6-d4eb56148449");
const VIRTUAL_CD_GUID: uefi::Guid = uefi::guid!("3d5abd30-4175-87ce-6d64-d2ade523c4bb");

const BOOT_FILE: &uefi::CStr16 = uefi::cstr16!("\\EFI\\BOOT\\BOOTX64.EFI");
const RAM_DISK_DXE: &[u8] = include_bytes!("../assets/RamDiskDxe.efi");

// EFI_LOAD_FILE2_PROTOCOL guid + the vendor guid the kernel's EFI stub looks up for its initrd
const LOAD_FILE2_GUID: uefi::Guid = uefi::guid!("4006c0c1-fcb3-403e-996d-4a6c8724e06d");
const DEVICE_PATH_GUID: uefi::Guid = uefi::guid!("09576e91-6d3f-11d2-8e39-00a0c969723b");
const LINUX_EFI_INITRD_MEDIA_GUID: uefi::Guid = uefi::guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");

const CONFIGS: [&str; 3] = [
    "boot/syslinux/archiso_sys-linux.cfg", // arch / cachy / blackarch (syslinux)
    "boot/grub/grub.cfg",                  // gentoo etc (grub)
    "EFI/BOOT/grub.cfg",                   // opensuse (grub, efi-only)
];

// make the appended hook run first
const GENTOO_HOOK: &str = include_str!("../assets/gentoo.sh");
const HOOK_PATH: &str = "usr/lib/dracut/hooks/pre-trigger/00-rsboot.sh";

// implementing this for the protocol to succesfully Identify it and not have to add a separate
// attribute that may break the structs internal structure when calling the methods
unsafe impl uefi::Identify for EFI_RAM_DISK_PROTOCOL {
    const GUID: uefi::Guid = RAM_DISK_GUID;
}
impl uefi::proto::Protocol for EFI_RAM_DISK_PROTOCOL {}

#[allow(dead_code)]
pub fn load_bootable(path: &uefi::CStr16) -> uefi::Result {
    let handler = uefi::boot::image_handle();

    let mut fs = uefi::fs::FileSystem::new(uefi::boot::get_image_file_system(handler)?);
    // needs mut because when reading the file system, the read pointer changes its state

    let buffer = fs.read(path).map_err(|e| {
        log::error!("failed to read {path}, {e}");
        uefi::Status::LOAD_ERROR
    })?;
    boot_from_iso(
        crate::downloader::IsoBuffer::from_bytes(&buffer)?,
        BootMethod::Netboot,
    )
}

pub fn boot_from_iso(
    iso: crate::downloader::IsoBuffer,
    boot_method: environment::BootMethod,
) -> uefi::Result {
    let handler = uefi::boot::image_handle();

    /*
    TODO: refactor this method, netboot methods panic on finding the filesystem
    also it hurts the eyes its horrid

    let image = unsafe { core::slice::from_raw_parts(iso.as_ptr(), iso.len()) };
    let instance = load_image(
        handler,
        uefi::boot::LoadImageSource::FromBuffer {
            buffer: image,
            file_path: None,
        },
    )?;
    if let Ok(sth) = start_image(instance) {
        return Ok(());
    } */

    // the iso is already allocated  so we only register those pages as virtual cd
    let base = iso.as_ptr() as u64;
    let size = iso.len() as u64;

    match boot_method {
        BootMethod::RamDisk => {
            // locate the ram disk protocol by its guid, then open it to call register
            let mut ramdisk_handlers = uefi::boot::locate_handle_buffer(
                uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID),
            );
            if ramdisk_handlers.is_err() {
                let ram_disk_dxe_instance = uefi::boot::load_image(
                    handler,
                    uefi::boot::LoadImageSource::FromBuffer {
                        buffer: RAM_DISK_DXE,
                        file_path: None,
                    },
                )?;
                uefi::boot::start_image(ram_disk_dxe_instance)?;
                ramdisk_handlers = uefi::boot::locate_handle_buffer(
                    uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID),
                );
            }

            let ramdisk_handle = *ramdisk_handlers?.first().ok_or(uefi::Status::NOT_FOUND)?; // .first() to get the
            // ramdisk protocol handle is installed only once in the uefi system therefore .first() is
            // enough and we dont need to iterate through it
            let ram_disk =
                uefi::boot::open_protocol_exclusive::<EFI_RAM_DISK_PROTOCOL>(ramdisk_handle)?;

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
            let ram_devicepath = unsafe {
                uefi::proto::device_path::DevicePath::from_ffi_ptr(ram_devicepath_ptr.cast())
            };

            // use the firmware to enumerate the FAT ESP inside the iso
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

            let handles_found = uefi::boot::locate_handle_buffer(
                uefi::boot::SearchType::ByProtocol(&uefi::proto::media::fs::SimpleFileSystem::GUID),
            )?;
            let mut fs_handle = None;
            for &h in handles_found.iter() {
                if let Ok(dp) =
                    uefi::boot::open_protocol_exclusive::<uefi::proto::device_path::DevicePath>(h)
                    && dp.as_bytes().starts_with(prefix)
                // most likely we have  multiple file systems, use
                // the handler found that coincides with the iso device path
                {
                    fs_handle = Some(h);
                    break;
                }
            }
            let fs_handle = fs_handle.ok_or_else(|| {
                log::error!("no EFI filesystem found on the iso");
                uefi::Status::NOT_FOUND
            })?; // might fail from older iso images that don't have UEFI adaptation

            // build fs path + \EFI\BOOT\BOOTX64.EFI and load it
            let fs_devicepath = uefi::boot::open_protocol_exclusive::<
                uefi::proto::device_path::DevicePath,
            >(fs_handle)?;
            let mut buf = alloc::vec::Vec::new();
            let mut builder =
                uefi::proto::device_path::build::DevicePathBuilder::with_vec(&mut buf);

            for node in fs_devicepath.node_iter() {
                builder = builder.push(&node).unwrap();
            }

            let full_path = builder
                .push(&uefi::proto::device_path::build::media::FilePath {
                    path_name: BOOT_FILE,
                })
                .unwrap()
                .finalize()
                .unwrap(); // too lazy to treat these results

            let instance = uefi::boot::load_image(
                handler,
                uefi::boot::LoadImageSource::FromDevicePath {
                    device_path: full_path,
                    boot_policy: uefi::proto::BootPolicy::ExactMatch,
                },
            )?;

            start_image(instance)
        }
        //same kernel + initrd from the ISO logic.
        // only initrd and cmdline differs
        BootMethod::Memmap | BootMethod::LoopInjection => {
            let cfg = CONFIGS
                .iter()
                .find_map(|p| read_iso(&iso, p))
                .ok_or(uefi::Status::NOT_FOUND)?;
            let (kernel_path, initrd_path, options) =
                parse_config(cfg).ok_or(uefi::Status::NOT_FOUND)?;
            let kernel_bytes = read_iso(&iso, &kernel_path).ok_or(uefi::Status::NOT_FOUND)?;
            let mut initrd = read_iso(&iso, &initrd_path).ok_or(uefi::Status::NOT_FOUND)?;

            let cmdline = match boot_method {
                BootMethod::LoopInjection => {
                    // add cpio as new initramfs segment
                    let iso_bytes = unsafe { core::slice::from_raw_parts(iso.as_ptr(), iso.len()) };
                    while !initrd.len().is_multiple_of(4) {
                        initrd.push(0);
                    }
                    initrd.extend_from_slice(&build_cpio(&[
                        ("rsboot.iso", iso_bytes, 0o100644), // 100 - regular file 644 - chmod permissions
                        (HOOK_PATH, GENTOO_HOOK.as_bytes(), 0o100755), // 100 755 - chmod permisions
                    ]));
                    options
                }
                BootMethod::Memmap => alloc::format!(
                    "{options} memmap={:#x}!{:#x}",
                    iso.mapped_len(),
                    iso.as_ptr() as usize,
                ),
                _ => unreachable!("this block reaches only  memmap or loop injection"),
            };

            let instance = load_image(
                handler,
                uefi::boot::LoadImageSource::FromBuffer {
                    buffer: &kernel_bytes,
                    file_path: None,
                },
            )?;
            install_initrd(initrd)?;

            let cmdline = uefi::CString16::try_from(cmdline.as_str()).unwrap();
            unsafe {
                let mut loaded_image = uefi::boot::open_protocol_exclusive::<
                    uefi::proto::loaded_image::LoadedImage,
                >(instance)?;
                loaded_image.set_load_options(cmdline.as_ptr().cast(), cmdline.num_bytes() as u32);
            }
            start_image(instance)
        }
        BootMethod::Netboot => {
            let image = unsafe { core::slice::from_raw_parts(iso.as_ptr(), iso.len()) };
            let instance = load_image(
                handler,
                uefi::boot::LoadImageSource::FromBuffer {
                    buffer: image,
                    file_path: None,
                },
            )?;
            start_image(instance)
        } /* _ => {
              log::error!("boot method not implemented yet");
              Err(uefi::Status::UNSUPPORTED.into())
          }*/
    }
}

// returns paths as Strings, stopped returning CStr16 due to the new ISO9660 parser using String
fn parse_config(
    config: alloc::vec::Vec<u8>,
) -> Option<(
    alloc::string::String,
    alloc::string::String,
    alloc::string::String,
)> {
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
    Some((kernel?, initrd?, options.or(inline_opts)?))
}

fn read_iso(iso: &IsoBuffer, path: &str) -> Option<alloc::vec::Vec<u8>> {
    // wrap the in-RAM iso bytes as a Read+Seek source for hadris-iso
    let bytes = unsafe { core::slice::from_raw_parts(iso.as_ptr(), iso.len()) };
    let cursor = hadris_io::Cursor::new(bytes);

    let img = hadris_iso::sync::IsoImage::open(cursor).ok()?;
    let mut dir_ref = img.root_dir().dir_ref();
    let mut parts = path.trim_matches('/').split('/').peekable(); // secventially
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

// EFI_LOAD_FILE2_PROTOCOL: C struct model took from edk2 firmware
#[repr(C)]
struct LoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: *mut LoadFile2Protocol,
        file_path: *const EFI_DEVICE_PATH,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut u8,
    ) -> uefi::Status,
    data: *const u8,
    len: usize,
}

// the EFI stub calls this to fetch the initrd: once with a null buffer to learn the size,
// then again with a buffer big enough to receive it
unsafe extern "efiapi" fn initrd_load_file(
    this: *mut LoadFile2Protocol,
    _file_path: *const EFI_DEVICE_PATH,
    boot_policy: bool,
    buffer_size: *mut usize,
    buffer: *mut u8,
) -> uefi::Status {
    // the initrd media protocol must be invoked with BootPolicy = false
    if boot_policy {
        return uefi::Status::UNSUPPORTED;
    }
    if this.is_null() || buffer_size.is_null() {
        return uefi::Status::INVALID_PARAMETER;
    }
    let this = unsafe { &*this }; // oh how much i love C 
    // first call (with null buffer) or too-small buffer: update the size  so the stub knows to allocate
    if buffer.is_null() || unsafe { *buffer_size } < this.len {
        unsafe { *buffer_size = this.len };
        return uefi::Status::BUFFER_TOO_SMALL;
    }
    unsafe {
        let dst: &mut [u8] = core::slice::from_raw_parts_mut(buffer, this.len);
        let src = core::slice::from_raw_parts(this.data, this.len);
        dst.copy_from_slice(src);
        *buffer_size = this.len;
    }
    uefi::Status::SUCCESS
}

// the device path the stub locates LoadFile2 on: vendor: custom 3rd party node
// carrying LINUX_EFI_INITRD_MEDIA_GUID, terminated by an end node
#[repr(C, packed)] // packed so that rust doesnt add padding, otherwise efi stub would read garbage  
struct InitrdDevicePath {
    vendor: EFI_DEVICE_PATH, //4 bytes
    vendor_guid: uefi::Guid, //16 bytes
    end: EFI_DEVICE_PATH,    // 4 bytes
}

// install the LoadFile2 and InitrdDevicePath so kernel's efi stub  can pull the initrd from RAM, this is MANDATORY for
// distributions that have their kernel/initrd on ISO9660 file system, if they are on EFI you can
// just append initrd=<INITRD_PATH> to the cmdline
fn install_initrd(initrd: alloc::vec::Vec<u8>) -> uefi::Result {
    // both structs must outlive this call: the stub reads them during start_image,
    // so leak them on purpose
    let proto: *mut LoadFile2Protocol =
        alloc::boxed::Box::into_raw(alloc::boxed::Box::new(LoadFile2Protocol {
            load_file: initrd_load_file,
            data: initrd.as_ptr(),
            len: initrd.len(),
        }));
    core::mem::forget(initrd);
    let dp: *mut InitrdDevicePath =
        alloc::boxed::Box::into_raw(alloc::boxed::Box::new(InitrdDevicePath {
            // MEDIA_DEVICE_PATH (0x04) / MEDIA_VENDOR_DP (0x03), length = 4 header + 16 guid
            vendor: EFI_DEVICE_PATH {
                r#type: 0x04,
                sub_type: 0x03,
                length: [20, 0],
            },
            vendor_guid: LINUX_EFI_INITRD_MEDIA_GUID,
            // END_DEVICE_PATH (0x7f) / END_ENTIRE (0xff), length = 4
            end: EFI_DEVICE_PATH {
                r#type: 0x7f,
                sub_type: 0xff,
                length: [4, 0],
            },
        }));

    // EFI STUB finds the handle via this func:
    // LocateDevicePath(&LOAD_FILE2_GUID,  //
    // &initrd_device_path, // the dp
    // &out_handle) // the found handle
    unsafe {
        // create the handle and attach the device path protocol to it
        let handle_tobefound_stub =
            uefi::boot::install_protocol_interface(None, &DEVICE_PATH_GUID, dp.cast())?;
        // install the loadfile2 protocol too
        uefi::boot::install_protocol_interface(
            Some(handle_tobefound_stub),
            &LOAD_FILE2_GUID,
            proto.cast(),
        )?;
    }
    Ok(())
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
