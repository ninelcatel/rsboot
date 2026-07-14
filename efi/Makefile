EFI      := target/$(TARGET)/debug/rsboot.efi
ESP_DIR  := ../qemu/esp/efi/boot
ESP_EFI  := $(ESP_DIR)/bootx64.efi

.PHONY: build install clean

build:
	cargo build x86_64-unknown-uefi

install: build
	mkdir -p $(ESP_DIR)
	cp $(EFI) $(ESP_EFI)

clean:
	cargo clean
