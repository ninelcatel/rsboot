// load a .efi into memory and then run it, keeps write/text format as before
// qemu was set up with the root file system at /esp , a valid path for example would be
// \\efi\\boot\\hello.efi

use uefi::Identify;

extern crate alloc;

#[repr(C)]
struct EFI_DEVICE_PATH {
    r#type: u8,
    SubType: u8,
    Length: [u8; 2],
}

#[repr(C)]
struct EFI_RAM_DISK_PROTOCOL {
    register: unsafe extern "efiapi" fn(
        RamDiskBase: u64,
        RamDiskSize: u64,
        RamDiskType: *const uefi::Guid,
        ParentDevicePath: *const EFI_DEVICE_PATH,
        DevicePath: *mut *const EFI_DEVICE_PATH,
    ) -> uefi::Status,
    unregister: unsafe extern "efiapi" fn(DevicePath: *const EFI_DEVICE_PATH) -> uefi::Status,
}

const RAM_DISK_GUID: uefi::Guid = uefi::guid!("ab38a0df-6873-44a9-87e6-d4eb56148449");
const VIRTUAL_CD_GUID: uefi::Guid = uefi::guid!("3d5abd30-4175-87ce-6d64-d2ade523c4bb");
const BOOT_FILE: &uefi::CStr16 = uefi::cstr16!("\\EFI\\BOOT\\BOOTX64.EFI");

// implementing this for the protocol to succesfully Identify it and not have to add a separate
// attribute that may break the structs internal structure when calling the methods
unsafe impl uefi::Identify for EFI_RAM_DISK_PROTOCOL {
    const GUID: uefi::Guid = RAM_DISK_GUID;
}
impl uefi::proto::Protocol for EFI_RAM_DISK_PROTOCOL {}

pub fn load_bootable(path: &uefi::CStr16) -> uefi::Result {
    let handler = uefi::boot::image_handle();

    let mut fs = uefi::fs::FileSystem::new(uefi::boot::get_image_file_system(handler)?);
    // needs mut because when reading the file system, the read pointer changes its state

    let buffer = fs.read(path).map_err(|e| {
        log::error!("failed to read {path}, {e}");
        uefi::Status::LOAD_ERROR
    })?;
    // loads the the file in RAM

    // the ram disk points at this memory in place, when loading the bootable, Rust might free the
    // buffer and then lead to undefined behaviour
    let image: &mut [u8] = buffer.leak();
    let base = image.as_ptr() as u64;
    let size = image.len() as u64;

    // locate the ram disk protocol by its guid, then open it to call register
    let mut handles_found =
        uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID))?;
    let ramdisk_handle = *handles_found.first().ok_or(uefi::Status::NOT_FOUND)?; // .first() to get the
    // ramdisk protocol handle is installed only once in the uefi system therefore .first() is
    // enough and we dont need to iterate through it
    let ram_disk = uefi::boot::open_protocol_exclusive::<EFI_RAM_DISK_PROTOCOL>(ramdisk_handle)?;

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
    let ram_devicepath =
        unsafe { uefi::proto::device_path::DevicePath::from_ffi_ptr(ram_devicepath_ptr.cast()) };

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

    handles_found = uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(
        &uefi::proto::media::fs::SimpleFileSystem::GUID,
    ))?;
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
    let fs_devicepath =
        uefi::boot::open_protocol_exclusive::<uefi::proto::device_path::DevicePath>(fs_handle)?;
    let mut buf = alloc::vec::Vec::new();
    let mut builder = uefi::proto::device_path::build::DevicePathBuilder::with_vec(&mut buf);

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

    uefi::boot::start_image(instance)?;
    // hangs until the .efi exits
    Ok(())
}
