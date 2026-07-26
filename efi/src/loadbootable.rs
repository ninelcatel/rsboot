// load a .efi into memory and then run it, keeps write/text format as before
// qemu was set up with the root file system at /esp , a valid path for example would be
// \\efi\\boot\\hello.efi
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
        device_path: *mut *const EFI_DEVICE_PATH,
    ) -> uefi::Status,
    unregister: unsafe extern "efiapi" fn(DevicePath: *const EFI_DEVICE_PATH) -> uefi::Status,
}

const RAM_DISK_GUID: uefi::Guid = uefi::guid!("ab38a0df-6873-44a9-87e6-d4eb56148449");
const VIRTUAL_CD_GUID: uefi::Guid = uefi::guid!("3d5abd30-4175-87ce-6d64-d2ade523c4bb");
pub fn load_bootable(path: &uefi::CStr16) -> uefi::Result {
    let handler = uefi::boot::image_handle();

    let mut fs = uefi::fs::FileSystem::new(uefi::boot::get_image_file_system(handler)?);
    // needs mut because when reading the file system, the read pointer changes its state

    let buffer = fs.read(path).map_err(|e| {
        log::error!("failed to read path, {e}");
        uefi::Status::LOAD_ERROR
    })?;
    // loads the the bootable file's bytes into  a buffer

    let instance = uefi::boot::load_image(
        handler,
        uefi::boot::LoadImageSource::FromBuffer {
            buffer: &buffer,
            file_path: None,
        },
    )?;
    let protocol =
        uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(&RAM_DISK_GUID))?;

    // allocates memory and loads the buffer in RAM . file path is optional

    uefi::boot::start_image(instance)?;
    // hangs until the .efi exits
    Ok(())
}
