## usage:

```sh
cargo build --target x86_64-unknown-uefi
mkdir -p ../qemu/esp/efi/boot/
cp target/x86_64-unknown-uefi/debug/rsboot.efi ../qemu/esp/efi/boot/bootx64.efi
```





