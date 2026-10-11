// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026 Alexandru-Nicolas Negrișan
// Definitions from this file (EFI_RAM_DISK_PROTOCOL, EFI_DEVICE_PATH and the EFI_LOAD_FILE2_PROTOCOL)
// are adapted from EDK II, licensed underBSD-2-Clause-Patent.
// Copyright (c) Intel Corporation and contributors.
// LINUX_EFI_INITRD_MEDIA_GUID is the initrd LoadFile2 GUID defined by the Linux kernel EFI stub
// See NOTICE for details.

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

// helper struct used for clean up after loop/memmap start_image fails
// if it fails, when booting another instance, the efi stub might pick up stale or broken
// protocols from before and would most likely lead to UB
pub struct InstalledInitrd {
    handle: uefi::Handle,
    protocol: *mut LoadFile2Protocol,
    dp: *mut InitrdDevicePath,
    #[allow(dead_code)] // its as an attribute only to consume it
    initrd: alloc::vec::Vec<u8>,
}
impl InstalledInitrd {
    pub fn leak(self) {
        core::mem::forget(self);
    }
    pub fn uninstall(self) {
        unsafe {
            let _ = uefi::boot::uninstall_protocol_interface(
                self.handle,
                &LOAD_FILE2_GUID,
                self.protocol.cast(),
            );
            let _ = uefi::boot::uninstall_protocol_interface(
                self.handle,
                &DEVICE_PATH_GUID,
                self.dp.cast(),
            );
            drop(alloc::boxed::Box::from_raw(self.protocol));
            drop(alloc::boxed::Box::from_raw(self.dp));
        }
    }
}

// install the LoadFile2 and InitrdDevicePath so kernel's efi stub  can pull the initrd from RAM, this is MANDATORY for
// distributions that have their kernel/initrd on ISO9660 file system, if they are on EFI you can
// just append initrd=<INITRD_PATH> to the cmdline
pub fn install_initrd(initrd: alloc::vec::Vec<u8>) -> uefi::Result<InstalledInitrd> {
    // both structs must outlive this call: the stub reads them during start_image,
    // so leak them on purpose
    let proto: *mut LoadFile2Protocol =
        alloc::boxed::Box::into_raw(alloc::boxed::Box::new(LoadFile2Protocol {
            load_file: initrd_load_file,
            data: initrd.as_ptr(),
            len: initrd.len(),
        }));
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
    let handle;
    unsafe {
        // create the handle and attach the device path protocol to it
        let handle_tobefound_stub =
            uefi::boot::install_protocol_interface(None, &DEVICE_PATH_GUID, dp.cast())?;
        // install the loadfile2 protocol too
        handle = uefi::boot::install_protocol_interface(
            Some(handle_tobefound_stub),
            &LOAD_FILE2_GUID,
            proto.cast(),
        )?;
    }
    Ok(InstalledInitrd {
        handle,
        protocol: proto,
        dp,
        initrd,
    })
}
use uefi::Identify;
/// install into the acpi table the SSDT containing ACPI0012 and the NFIT virtual volatile disk
pub fn install_nfit(base: u64, len: u64) -> uefi::Result {
    if len == 0 {
        return Err(uefi::Status::INVALID_PARAMETER.into());
    }

    base.checked_add(len)
        .ok_or(uefi::Status::INVALID_PARAMETER)?;

    let handle = *uefi::boot::locate_handle_buffer(uefi::boot::SearchType::ByProtocol(
        &uefi::proto::acpi::AcpiTable::GUID,
    ))?
    .first()
    .ok_or(uefi::Status::NOT_FOUND)?;

    let proto = unsafe {
        uefi::boot::open_protocol::<uefi::proto::acpi::AcpiTable>(
            uefi::boot::OpenProtocolParams {
                handle,
                agent: uefi::boot::image_handle(),
                controller: None,
            },
            uefi::boot::OpenProtocolAttributes::GetProtocol,
        )?
    };

    let ssdt = build_ssdt()?;
    let nfit = build_nfit(base, len)?;

    // the ssdt must exist before the nfit driver binds to the SPA range
    unsafe {
        proto.install_acpi_table(ssdt.as_ptr().cast(), ssdt.len())?;
        proto.install_acpi_table(nfit.as_ptr().cast(), nfit.len())?;
    };

    Ok(())
}

// build the NFIT entry with the iso buffer poitner and len
// this basically describes the memory address the nfit driver will act on
fn build_nfit(base: u64, len: u64) -> uefi::Result<alloc::vec::Vec<u8>> {
    const ACPI_HEADER_SIZE: usize = 36;
    const NFIT_RESERVED_SIZE: usize = 4;
    const SPA_RANGE_SIZE: usize = 56;
    const NFIT_SIZE: usize = ACPI_HEADER_SIZE + NFIT_RESERVED_SIZE + SPA_RANGE_SIZE;

    let mut table = alloc::vec::Vec::with_capacity(NFIT_SIZE);

    append_acpi_header(
        &mut table,
        *b"NFIT",
        NFIT_SIZE as u32,
        1,
        *b"RSBOOT",
        *b"RAMDISK ",
    );

    // nfit reserved field  after the acpi header.
    table.extend_from_slice(&0u32.to_le_bytes());
    // SPA structure
    table.extend_from_slice(&0u16.to_le_bytes()); // 0 SPA range.
    table.extend_from_slice(&(SPA_RANGE_SIZE as u16).to_le_bytes());
    // spa flags and attributes
    table.extend_from_slice(&0u16.to_le_bytes());
    table.extend_from_slice(&0u16.to_le_bytes());
    table.extend_from_slice(&0u32.to_le_bytes());
    table.extend_from_slice(&0u32.to_le_bytes());
    // VOLATILE VIRTUAL DISK GUID
    table.extend_from_slice(&[
        0x5a, 0x53, 0xab, 0x77, 0xfc, 0x45, 0x4b, 0x62, 0x55, 0x60, 0xf7, 0xb2, 0x81, 0xd1, 0xf9,
        0x6e,
    ]);
    table.extend_from_slice(&base.to_le_bytes());
    table.extend_from_slice(&len.to_le_bytes());
    //ramdisk edk2 conventions
    table.extend_from_slice(&0u64.to_le_bytes());

    edit_acpi_chksum(&mut table)?;

    Ok(table)
}
// this is just black magic for ssdt acpi entry
// similar to nfit build but requires this append_pkg_length stuff in order
// for the OS' acpi interpreter (ACPI machine language) to understand it
// this is basically for the OS to be able to bind the NFIT driver
fn build_ssdt() -> uefi::Result<alloc::vec::Vec<u8>> {
    let mut device_body = alloc::vec::Vec::new();

    device_body.extend_from_slice(b"NVDR");

    device_body.push(0x08);
    device_body.extend_from_slice(b"_HID");
    device_body.push(0x0d);
    device_body.extend_from_slice(b"ACPI0012");
    device_body.push(0x00);
    let mut method_body = alloc::vec::Vec::new();

    method_body.extend_from_slice(b"_STA");
    method_body.push(0x00);
    method_body.push(0xa4);
    method_body.push(0x0a);
    method_body.push(0x0f);
    device_body.push(0x14);
    append_pkg_length(&mut device_body, method_body.len())?;
    device_body.extend_from_slice(&method_body);

    let mut device = alloc::vec::Vec::new();

    device.push(0x5b);
    device.push(0x82);
    append_pkg_length(&mut device, device_body.len())?;
    device.extend_from_slice(&device_body);

    let mut scope_body = alloc::vec::Vec::new();

    scope_body.push(0x5c);
    scope_body.extend_from_slice(b"_SB_");
    scope_body.extend_from_slice(&device);

    let mut aml = alloc::vec::Vec::new();

    aml.push(0x10);
    append_pkg_length(&mut aml, scope_body.len())?;
    aml.extend_from_slice(&scope_body);

    let total_size = 36usize + aml.len();

    let mut table = alloc::vec::Vec::with_capacity(total_size);

    append_acpi_header(
        &mut table,
        *b"SSDT",
        total_size as u32,
        2,
        *b"RSBOOT",
        *b"RAMDISK ",
    );

    table.extend_from_slice(&aml);

    edit_acpi_chksum(&mut table)?;

    Ok(table)
}

fn append_acpi_header(
    table: &mut alloc::vec::Vec<u8>,
    signature: [u8; 4],
    length: u32,
    revision: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
) {
    table.extend_from_slice(&signature);
    table.extend_from_slice(&length.to_le_bytes());
    table.push(revision);
    table.push(0);
    table.extend_from_slice(&oem_id);
    table.extend_from_slice(&oem_table_id);
    table.extend_from_slice(&1u32.to_le_bytes());
    table.extend_from_slice(b"RSBT");
    table.extend_from_slice(&1u32.to_le_bytes());
}

// needed for building anything written in AML
// this lets OS know where each section ends
// i.e method-pkg_length-devbody
fn append_pkg_length(output: &mut alloc::vec::Vec<u8>, body_length: usize) -> uefi::Result {
    for encoded_length in 1usize..=4 {
        let package_length = body_length
            .checked_add(encoded_length)
            .ok_or(uefi::Status::BAD_BUFFER_SIZE)?;

        let maximum = match encoded_length {
            1 => 0x3f,
            2 => 0x0fff,
            3 => 0x0f_ffff,
            4 => 0x0fff_ffff,
            _ => unreachable!(),
        };

        if package_length <= maximum {
            encode_pkg_length(output, package_length, encoded_length);
            return Ok(());
        }
    }
    Err(uefi::Status::BAD_BUFFER_SIZE.into())
}
//helper for pkg length
fn encode_pkg_length(
    output: &mut alloc::vec::Vec<u8>,
    package_length: usize,
    encoded_length: usize,
) {
    if encoded_length == 1 {
        output.push(package_length as u8);
        return;
    }

    let following_bytes = encoded_length - 1;

    let lead = ((following_bytes as u8) << 6) | ((package_length & 0x0f) as u8);

    output.push(lead);

    let mut remaining = package_length >> 4;

    for _ in 0..following_bytes {
        output.push((remaining & 0xff) as u8);
        remaining >>= 8;
    }
}
fn edit_acpi_chksum(table: &mut [u8]) -> uefi::Result<()> {
    if table.len() < 36 {
        return Err(uefi::Status::BAD_BUFFER_SIZE.into());
    }
    table[9] = 0; // checksum byte of the ACPI header 

    let sum = table.iter().copied().fold(0u8, u8::wrapping_add);

    table[9] = 0u8.wrapping_sub(sum); // two complement of whole table sum
    Ok(())
}
