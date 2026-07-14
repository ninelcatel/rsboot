## usage:

### via cargo

```sh
cargo build --target x86_64-unknown-uefi
mkdir -p ../qemu/esp/efi/boot/
cp target/x86_64-unknown-uefi/debug/rsboot.efi ../qemu/esp/efi/boot/bootx64.efi
```

### via Make

```sh
make build
make install #builds and copies it to ../qemu 
```
