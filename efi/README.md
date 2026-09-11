## usage:

#### via cargo, replace paths accordingly:
```sh
# aarch64-unknown-uefi 
cargo build --target x86_64-unknown-uefi
mkdir -p ../qemu/esp/efi/boot/
# bootaa64.efi
cp target/x86_64-unknown-uefi/debug/rsboot.efi ../qemu/esp/efi/boot/bootx64.efi
```

### via Make

```sh
make build             # x86_64, debug target (default)
make install           # builds and copies bootx64.efi to ../qemu/esp/efi/boot/bootx64.efi

make build ARCH=aarch64    # builds arm64 instead 
make install ARCH=aarch64  # copies to bootaa64.efi instead

make install RELEASE=1     # optimized build 
```

## dependencies

* **rustup**: installer for rust compiler, cargo and targets 
* **uefi-rs**: UEFI crate for protocols, drivers, TUI, etc, base for the app 
* **hadris-iso**: parse the ISO9660 from RAM (kernel/initrd/cmdline)
* **hadris-io**: read wrapper over the ISO bytes for hadris-iso
* **sha2**: integrity check for the iso 

### RamDiskDxe fallback: From Tianocore official EDK2 OVMF repository, built from commit `fc939c7b37d72e5245a7a5abeda576704b5bd31b` 


### GUIDs  that may or may not be used from [edk2 repo](https://github.com/tianocore/edk2/blob/master/MdePkg/MdePkg.dec):


``` 
#define EFI_RAM_DISK_PROTOCOL_GUID \
  { 0xab38a0df, 0x6873, 0x44a9, { 0x87, 0xe6, 0xd4, 0xeb, 0x56, 0x14, 0x84, 0x49 }};
  gEfiVirtualDiskGuid            = { 0x77AB535A, 0x45FC, 0x624B, {0x55, 0x60, 0xF7, 0xB2, 0x81, 0xD1, 0xF9, 0x6E }}
  gEfiVirtualCdGuid              = { 0x3D5ABD30, 0x4175, 0x87CE, {0x6D, 0x64, 0xD2, 0xAD, 0xE5, 0x23, 0xC4, 0xBB }}
  gEfiPersistentVirtualDiskGuid  = { 0x5CEA02C9, 0x4D07, 0x69D3, {0x26, 0x9F ,0x44, 0x96, 0xFB, 0xE0, 0x96, 0xF9 }}
  gEfiPersistentVirtualCdGuid    = { 0x08018188, 0x42CD, 0xBB48, {0x10, 0x0F, 0x53, 0x87, 0xD5, 0x3D, 0xED, 0x3D }}


```


