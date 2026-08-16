NOGRAPHIC ?= 0

ifeq ($(NOGRAPHIC),1)
    DISPLAY := -nographic
else
    DISPLAY :=
endif

MEM ?= 3G
OVMF_SRC := /usr/share/edk2/x64
# double check path after installing edk2-ovmf package

OVMF_CODE := OVMF_CODE.fd
OVMF_VARS := OVMF_VARS.fd

QEMU := qemu-system-x86_64 -enable-kvm -cpu host -machine q35 -m $(MEM) $(DISPLAY) \
	-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
	-drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS) \
	-device virtio-vga,xres=1280,yres=800 

AARCH_SRC := /usr/share/edk2/aarch64
AAVMF_CODE := $(AARCH_SRC)/QEMU_EFI.fd
AAVMF_VARS := AAVMF_VARS.fd

QEMU_ARM := qemu-system-aarch64 -machine virt -cpu cortex-a57 -m $(MEM) $(DISPLAY) \
  -drive if=pflash,format=raw,readonly=on,file=$(AAVMF_CODE) \
  -drive if=pflash,format=raw,file=$(AAVMF_VARS) \
  -device virtio-gpu-pci -device qemu-xhci -device usb-kbd \
  -device virtio-net-pci,netdev=n0 -netdev user,id=n0 \
  -device virtio-rng-pci

.PHONY: setup run run-app run-arm clean

setup: $(OVMF_CODE) $(OVMF_VARS)

$(OVMF_CODE):
	cp $(OVMF_SRC)/OVMF_CODE.4m.fd $(OVMF_CODE)

$(OVMF_VARS):
	cp $(OVMF_SRC)/OVMF_VARS.4m.fd $(OVMF_VARS)

$(AAVMF_VARS):
	cp $(AARCH_SRC)/QEMU_VARS.fd $(AAVMF_VARS)

run: setup
	$(QEMU)

run-app: setup
	@test -f esp/efi/boot/bootx64.efi || { echo "missing esp/efi/boot/bootx64.efi ! build the .efi and copy it there"; exit 1; }
	$(QEMU) -drive format=raw,file=fat:rw:esp

run-arm: $(AAVMF_VARS)
	@test -f esp/efi/boot/bootaa64.efi || { echo "missing esp/efi/boot/bootaa64.efi ! build it with 'cd ../efi && make install ARCH=aarch64'"; exit 1; }
	$(QEMU_ARM) -drive format=raw,file=fat:rw:esp

clean:
	rm -rf esp $(AAVMF_VARS)
