# ARCH=aarch64 builds for the ARM architecture
ARCH ?= x86_64

ifeq ($(ARCH),aarch64)
    TARGET   := aarch64-unknown-uefi
    BOOT_EFI := bootaa64.efi
else
    TARGET   := x86_64-unknown-uefi
    BOOT_EFI := bootx64.efi
endif

# RELEASE=1 builds the optimized profile 

RELEASE ?= 0

ifeq ($(RELEASE),1)
    PROFILE     := release
    CARGO_FLAGS := --release
else
    PROFILE     := debug
    CARGO_FLAGS :=
endif

EFI      := target/$(TARGET)/$(PROFILE)/rsboot.efi
ESP_DIR  := ../qemu/esp/efi/boot
ESP_EFI  := $(ESP_DIR)/$(BOOT_EFI)

.PHONY: build install clean

build:
	cargo build --target $(TARGET) $(CARGO_FLAGS)

install: build
	mkdir -p $(ESP_DIR)
	cp $(EFI) $(ESP_EFI)

clean:
	cargo clean
