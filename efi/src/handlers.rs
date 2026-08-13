extern crate alloc;

pub const RAM_DISK_GUID: uefi::Guid = uefi::guid!("ab38a0df-6873-44a9-87e6-d4eb56148449");
pub const VIRTUAL_CD_GUID: uefi::Guid = uefi::guid!("3d5abd30-4175-87ce-6d64-d2ade523c4bb");

// EFI_LOAD_FILE2_PROTOCOL guid + the vendor guid the kernel's EFI stub looks up for its initrd
const LOAD_FILE2_GUID: uefi::Guid = uefi::guid!("4006c0c1-fcb3-403e-996d-4a6c8724e06d");
const DEVICE_PATH_GUID: uefi::Guid = uefi::guid!("09576e91-6d3f-11d2-8e39-00a0c969723b");
const LINUX_EFI_INITRD_MEDIA_GUID: uefi::Guid = uefi::guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");

#[repr(C)]
pub struct EFI_DEVICE_PATH {
    r#type: u8, // media device path : example 0x01 = Hardware, 0x02 = ACPI
    sub_type: u8,
    length: [u8; 2], // total bytes, header + payload, LITTLE ENDIAN
}

#[repr(C)]
pub struct EFI_RAM_DISK_PROTOCOL {
    pub register: unsafe extern "efiapi" fn(
        ram_disk_base: u64,
        ram_disk_size: u64,
        ram_disk_type: *const uefi::Guid,
        parent_device_path: *const EFI_DEVICE_PATH,
        device_path: *mut *const EFI_DEVICE_PATH,
    ) -> uefi::Status,
    pub unregister: unsafe extern "efiapi" fn(device_path: *const EFI_DEVICE_PATH) -> uefi::Status,
}

// implementing this for the protocol to succesfully Identify it and not have to add a separate
// attribute that may break the structs internal structure when calling the methods
unsafe impl uefi::Identify for EFI_RAM_DISK_PROTOCOL {
    const GUID: uefi::Guid = RAM_DISK_GUID;
}
impl uefi::proto::Protocol for EFI_RAM_DISK_PROTOCOL {}

// EFI_LOAD_FILE2_PROTOCOL: C struct model took from edk2 firmware
#[repr(C)]
pub struct LoadFile2Protocol {
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
pub unsafe extern "efiapi" fn initrd_load_file(
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
pub struct InitrdDevicePath {
    vendor: EFI_DEVICE_PATH, //4 bytes
    vendor_guid: uefi::Guid, //16 bytes
    end: EFI_DEVICE_PATH,    // 4 bytes
}

// install the LoadFile2 and InitrdDevicePath so kernel's efi stub  can pull the initrd from RAM, this is MANDATORY for
// distributions that have their kernel/initrd on ISO9660 file system, if they are on EFI you can
// just append initrd=<INITRD_PATH> to the cmdline
pub fn install_initrd(initrd: alloc::vec::Vec<u8>) -> uefi::Result {
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
