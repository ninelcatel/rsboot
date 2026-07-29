# References

Resources used while building rsboot.

**Rust UEFI Book**: the basis for the project, `x86_64-unknown-uefi`, and the QEMU setup

uefi-rs crate (https://docs.rs/uefi/latest/uefi/index.html)[documentation]

**EDK2 TianoCore firmware** was used for testing this project. I'm also using the RamDiskDxe and VirtulCD drivers/GUIDs from their firmware for the loadbootable.rs, the driver being a fallback for non edk2 firmware.


