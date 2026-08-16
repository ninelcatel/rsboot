// RamDisk boot method: register the iso pages as a virtual CD, find the FAT filesystem
// and load \EFI\BOOT\BOOTX64.EFI.

extern crate alloc;

use uefi::{Identify, boot::ScopedProtocol};

use super::load_from_buffer;
use crate::{
    downloader::IsoBuffer,
    handlers::{EFI_DEVICE_PATH, EFI_RAM_DISK_PROTOCOL, RAM_DISK_GUID, VIRTUAL_CD_GUID},
};

#[cfg(target_arch = "x86_64")]
const BOOT_FILE: &uefi::CStr16 = uefi::cstr16!("\\EFI\\BOOT\\BOOTX64.EFI");
#[cfg(target_arch = "aarch64")]
const BOOT_FILE: &uefi::CStr16 = uefi::cstr16!("\\EFI\\BOOT\\BOOTAA64.EFI");

const RAM_DISK_DXE: &[u8] = include_bytes!("../../assets/RamDiskDxe.efi");

pub(super) fn locate_ramdisk(handler: uefi::Handle) -> uefi::Result<uefi::Handle> {
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
pub(super) struct VirtualCd {
    block_handle: Option<uefi::Handle>,
    unregister: unsafe extern "efiapi" fn(*const EFI_DEVICE_PATH) -> uefi::Status,
    dp: *const EFI_DEVICE_PATH,
    iso: Option<IsoBuffer>,
}

impl VirtualCd {
    pub(super) fn device_path(&self) -> &uefi::proto::device_path::DevicePath {
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

pub(super) fn register_virtual_cd(
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

pub(super) fn find_fat_fs(
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
pub(super) fn load_boot_file(
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
        .map_err(|_| uefi::Status::DEVICE_ERROR)?;

    let instance = uefi::boot::load_image(
        parent_image,
        uefi::boot::LoadImageSource::FromDevicePath {
            device_path: full_path,
            boot_policy: uefi::proto::BootPolicy::ExactMatch,
        },
    )?;
    Ok(instance)
}
