// load a .efi into memory and then run it, keeps write/text format as before
// qemu was set up with the root file system at /esp , a valid path for example would be
// \\efi\\boot\\hello.efi
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
    // allocates memory and loads the buffer in RAM . file path is optional

    uefi::boot::start_image(instance)?;
    // hangs until the .efi exits
    Ok(())
}
