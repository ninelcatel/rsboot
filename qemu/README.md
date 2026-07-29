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
make run MEM=4G   # needed for iso files >500mb
make run NOGRAPHIC=1 # terminal/serial approach
make run-app      # boots the app
make clean        # remove generated esp
```

```sh
mkdir -p esp/efi/boot
cp <efi_path> esp/efi/boot/bootx64.efi
make run-app
```

### for testing locally, create a <<dir>> that holds the .iso files and start http serever there, QEMU maps 10.0.2.2 to localhost by default so its faster to test like this rather than actual mirrors

```
python3 -m http.server 8000 --directory <<dir>>
```
## reference

Rust UEFI Book  <https://rust-osdev.github.io/uefi-rs/tutorial/vm.html>
