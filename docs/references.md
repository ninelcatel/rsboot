# References

Resources used while building rsboot.

## Core

- **Rust UEFI Book**: the basis for the project: `x86_64-unknown-uefi`,
  and the QEMU/OVMF setup. <https://rust-osdev.github.io/uefi-rs/>
- **uefi-rs crate**: UEFI protocols [docs](https://docs.rs/uefi/latest/uefi/) - [repo](https://github.com/rust-osdev/uefi-rs)
- **EDK2 / TianoCore firmware**: the firmware `rsboot` is tested against. The `RamDiskDxe`
  driver and its `VirtualCd` GUID is reused for the RamDisk boot method
  (`boot/ramdisk.rs`), bundled as a fallback for non-EDK2 firmware
	(`assets/RamDiskDxe.efi`).
    <https://github.com/tianocore/edk2> - [GUIDs](https://github.com/tianocore/edk2/blob/master/MdePkg/MdePkg.dec)
## ISO parsing

Two different parsers/readers for the ISO, depending on the boot method:

- **UEFI [SimpleFileSystem](https://docs.rs/uefi/latest/uefi/proto/media/fs/struct.SimpleFileSystem.html)**: for the RamDisk method, the firmware's 
  driver mounts the registered virtual CD and `rsboot` reads `\EFI\BOOT\BOOTX64.EFI`
  through this protocol (`boot/ramdisk.rs`). 
- **hadris-iso**: for the Memmap/Loop methods there's no mounted filesystem, so hadris
  parses the ISO9660 filesystem straight out of the raw RAM buffer to locate the
  kernel / initrd / bootloader config.
  [docs](https://docs.rs/hadris-iso)-[crate](https://crates.io/crates/hadris-iso)
- **hadris-io**: read wrapper over the ISO bytes that hadris-iso parses 
  [docs](https://docs.rs/hadris-io)-[crate](https://crates.io/crates/hadris-io)
## Checksums

- **sha2 crate** SHA256 for the integrity check
  [docs](https://docs.rs/sha2)-[crate](https://crates.io/crates/sha2)

## Boot methods

- **Linux EFI stub, initrd via LoadFile2**: how the kernel's EFI stub pulls the initrd
  from a `LINUX_EFI_INITRD_MEDIA_GUID` device path (`handlers.rs`).
  <https://docs.kernel.org/admin-guide/efi-stub.html>
- **Kernel memmap / persistent memory**: exposing the ISO as `pmem` for the
  Memmap boot method. <https://docs.kernel.org/admin-guide/kernel-parameters.html>
- **initramfs / cpio format**: the archive layout appended to the initrd for the
  Gentoo LoopInjection method.
  <https://www.kernel.org/doc/html/latest/driver-api/early-userspace/buffer-format.html>
