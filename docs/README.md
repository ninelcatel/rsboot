# Bachelor Thesis - An UEFI OS installer written in Rust

* An UEFI TUI written in Rust.
* Downloads a selected OS installer ISO into RAM over the network, and boots it directly.
* Most UEFI apps are developed with C, I believe Rust would make a great modern alternative to a more secure and safe environment.


## Main app flow:

1. Establish a DHCP connection.
2. Shows a TUI listing available distros and their versions.
3. Downloads the chosen installer ISO over HTTP from a mirror list, own server, or host.
4. Loads the ISO into RAM, and boot into it to start the install process.
5. Install OS 


## Phase 1: .efi is directly flashed into QEMU
1. Building the **TUI** (done)
2. Loading another local .efi and booting into it (done)
3. Booting into OS .iso loaded in RAM (done)
4. **DHCP** + **HTTP** get (done)
5. Boot methods for distributions:
  * Distros that ship RAM Disk support (mostly Debian/RHEL based and some BSD) : **RamDisk Boot** 
  * Distros that don't have initramfs segment that scans the RAM for block devices (Arch based): **Memmap Boot** 
	  [Artix needs PVD label, some other Arch based distros might need it too, and may or may not have a different cmdline parameter for the label (**misolabel or archisolabel**)] 
  * Distros that need the full ISO injected in the initrd (Gentoo): **Loop Injection** Boot 
  * Distros that ship an .efi and rely on network to work (NixOS): **Netboot** 
(done)
6. Verify the .iso checksums (done)
7. Boot the verified .iso (done)
8. Add OS catalog (mostly done, will probably add a Latest edition for **_easy_** maintaining)
9. Add hardcoded paths for ARM architecture, the bootloader's file name is different, maybe change catalog too 

## Phase 2: PXE/Network Boot via Docker
1. PXE/Network boot container holding the .efi
2. HTTP server on the container
3. Configure network and the server to act as an PXE server 
4. Python/Bash script job to routinely check for latest version 
5. Get a VPS holding the OS iso files and make the OS catalog in regards to the public IP/domain
6. Security measures for the VPS (with or without a proxy)
7. Test each distribution.

## Work environment (Artix with dinit)

```sh
sudo pacman -S rustup
rustup default stable
rustup target add x86_64-unknown-uefi

sudo pacman -S qemu-full edk2-ovmf

sudo pacman -S docker docker-compose docker-dinit

sudo dinitctl enable docker
sudo dinitctl start docker

```

## Rust crates used: 

* [uefi](https://crates.io/crates/uefi) ([docs](https://docs.rs/uefi)): UEFI protocols + boot services
* [hadris-iso](https://crates.io/crates/hadris-iso) ([docs](https://docs.rs/hadris-iso)): parse the ISO9660 from RAM (kernel/initrd/cmdline)
* [hadris-io](https://crates.io/crates/hadris-io) ([docs](https://docs.rs/hadris-io)): read wrapper over the ISO bytes for hadris-iso
* [sha2](https://crates.io/crates/sha2) ([docs](https://docs.rs/sha2)): sha256 integrity check for the iso

