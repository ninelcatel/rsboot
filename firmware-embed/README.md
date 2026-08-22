# Embedding rsboot into UEFI firmware 

How to embed `rsboot.efi` **into** an UEFI firmware image as an UEFI application with its own boot entry, so it launches straight from the ROM. No ESP, no USB, no PXE server. 

## SAFETY CAUTION!
This guide targets **OVMF under QEMU**. Flashing a **real** motherboard ROM is vendor-specific, can **BREAK** it. Use at your own RISK. 
---
## 1. The core idea 

1. **Embed the application** into the firmware.
2. **Register a boot entry** for the app.

## 2. Dependencies

### Building OVMF
- **edk2 source tree** (I used [edk2](github.com/tianocore/edk2) source code).
- Build once with `make -C BaseTools` if `Build/.../bin` is empty.
- **Toolchain:** `gcc`, `make`, `python3`, `nasm` , `iasl` 
  On Arch/Artix: `pacman -S base-devel python nasm acpica`. 
- **The bootloader:** `rsboot.efi` (`cd efi && make build`, .efi at `target/x86_64-unknown-uefi/debug/rsboot.efi`).

### Firmware Dependencies (drivers that **must** be present in the built firmware for rsboot to work)
- **HTTP network stack** — `HttpDxe`/`HttpUtilitiesDxe`/`DnsDxe`, enabled by **`-D NETWORK_HTTP_BOOT_ENABLE=TRUE`** flag at build time, **`NETWORK_ALLOW_HTTP_CONNECTIONS=TRUE`** (this is already defaulted to true)
- **`EFI_RAM_DISK_PROTOCOL`** Optional, `rsboot` bundles a fallback `RamDiskDxe.efi`


## 3. The edits needed to bake `rsboot` in the firmware: 

Paths are relative to the edk2 root 

### 3.0 Use the python script, resolve to manual steps only if failing.

```sh
git clone https://github.com/tianocore/edk2.git && cd edk2
cp /path/to/embed-rsboot.py embed-rsboot.py
git submodule update --init
make -C BaseTools
./embed-rsboot.py /path/to/rsboot.efi #this script MIGHT not work. only tested on #d98a39d
```

### 3.1 Drop in the binary 
```
mkdir OvmfPkg/Rsboot
cp /path/to/rsboot.efi OvmfPkg/Rsboot/Rsboot.efi
```

### 3.2 Create the `.inf` binary file

`OvmfPkg/Rsboot/Rsboot.inf`: 

```
[Defines]
  INF_VERSION = 0x00010005
  BASE_NAME = Rsboot
  FILE_GUID = B1ADEED9-ECC0-3330-4444-777777751033
  MODULE_TYPE = UEFI_APPLICATION
  VERSION_STRING = 1.0

[Binaries.X64]
  PE32|Rsboot.efi|*
```

The `FILE_GUID` is the identity the boot entry points at.

### 3.3 Edit `OvmfPkg/OvmfPkgX64.dsc`:
Append `OvmfPkg/Rsboot/Rsboot.inf` under `[Components]`

### 3.4 Edit `OvmfPkg/OvmfPkgX64.fdf`: 
Append `INF OvmfPkg/Rsboot/Rsboot.inf` under [FV.DXEFV], where all the INF statements are.

### 3.5 Register the boot entry: `OvmfPkg/Library/PlatformBootManagerLib/BdsPlatform.c`

In `PlatformBootManagerAfterConsole()`, declare the GUID (must match the `Rsboot.inf`'s `FILE_GUID`):

```c
  VOID EFIAPI PlatformBootManagerAfterConsole(VOID) {
  EFI_BOOT_MODE BootMode;
  EFI_GUID RsbootFileGuid = {0xB1ADEED9, 0xECC0, 0x3330, {0x44, 0x44, 0x77, 0x77, 0x77, 0x75, 0x10, 0x33}};
  ...
  }
```

and register it right before the `RemoveStaleFvFileOptions ();`  
```c
  // Register rsboot
  PlatformRegisterFvBootOption (
    &RsbootFileGuid,
    L"rsboot",
    LOAD_OPTION_ACTIVE,
    TRUE
    );
```
---

## 4. Build

```sh
cd /path/to/edk2
source ./edksetup.sh
build -a X64 -t GCC -b RELEASE -p OvmfPkg/OvmfPkgX64.dsc -D NETWORK_HTTP_BOOT_ENABLE=TRUE
```

Output: `Build/OvmfX64/RELEASE_GCC/FV/OVMF_CODE.fd` and `OVMF_VARS.fd`.

Verify rsboot and the HTTP dependencies 

```sh
grep -iE 'Rsboot|HttpDxe|HttpUtilitiesDxe|DnsDxe' Build/OvmfX64/RELEASE_GCC/FV/Guid.xref
# expect: Rsboot, HttpDxe, HttpUtilitiesDxe, DnsDxe
```

---

## 5. Run it

```sh
cd /path/to/edk2/Build/OvmfX64/RELEASE_GCC/FV

qemu-system-x86_64 -enable-kvm -cpu host -machine q35 -m 4G \
  -drive if=pflash,format=raw,readonly=on,file=OVMF_CODE.fd \
  -drive if=pflash,format=raw,file=OVMF_VARS.fd \
  -device virtio-vga,xres=1280,yres=800 \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0

```

**Network:** the NIC must be able to reach whatever the `os_list` URLs point at. With `-netdev user` the host is `10.0.2.2`, to hit the homelab `mirror at `192.168.100.3`, put the NIC on the `bridge-pxe` tap instead:

```sh
sudo ip tuntap add dev tap0 mode tap user $(id -un) 
sudo ip link set tap0 master bridge-pxe
sudo ip link set tap0 up
# then swap the -netdev line for:
  -netdev tap,id=n0,ifname=tap0,script=no,downscript=no -device virtio-net-pci,netdev=n0
```


