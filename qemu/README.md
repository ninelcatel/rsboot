## dependencies

### **to do**:  add commands for other distros 

```sh
sudo pacman -S qemu-full edk2-ovmf
```

### copy OVMF in this directory

double check path after installing edk2-ovmf package (see [Makefile](./Makefile))

```sh
make setup     # copies OVMF_CODE.fd and OVMF_VARS.fd here
```

## usage

```sh
make run          # boot firmware only 
make run MEM=4G 
make run NOGRAPHIC=1 # terminal/serial approach
make run-app      # boots the app
make clean        # remove generated esp
```

```sh
mkdir -p esp/efi/boot
cp <efi_path> esp/efi/boot/bootx64.efi
make run-app
```

## reference

Rust UEFI Book  <https://rust-osdev.github.io/uefi-rs/tutorial/vm.html>
