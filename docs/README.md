# Bachelor Thesis - An UEFI OS installer written in Rust

* An UEFI TUI written in Rust.
* Downloads a selected OS installer ISO into RAM over the network, and boots it directly.
* Most UEFI apps are developed with C, I believe Rust would make a great modern alternative to a more secure and safe environment.


## Main app flow:

1. Shows a TUI listing available distros.
2. Acquire an IP via DHCP.
3. Downloads the chosen installer ISO over HTTP from a proxy.
4. Loads the ISO into RAM, and boot into it to start the install process.

Will start with Linux only, Windows/BSD might or might not be implemented

## Phase 1: .efi is directly flashed into QEMU 
1. Building the TUI (in progress)
2. Loading another local .efi and booting into it
3. Booting into OS .iso loaded in RAM
4. DHCP + HTTP get
5. Verify the .iso (checksums)
6. Boot the verified .iso

## Phase 2: PXE/Network Boot via Docker
**To Do**

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
