#!/usr/bin/env python3
# run from the edk2 root:  ./embed-rsboot.py /path/to/rsboot.efi

import os
import sys

GUID = "B1ADEED9-ECC0-3330-4444-777777751033"
GUID_C = "{0xB1ADEED9, 0xECC0, 0x3330, {0x44, 0x44, 0x77, 0x77, 0x77, 0x75, 0x10, 0x33}}"

DSC = "OvmfPkg/OvmfPkgX64.dsc"
FDF = "OvmfPkg/OvmfPkgX64.fdf"
BDS = "OvmfPkg/Library/PlatformBootManagerLib/BdsPlatform.c"
RSDIR = "OvmfPkg/Rsboot"
INF = RSDIR + "/Rsboot.inf"

def backup(path):
    if not os.path.exists(path + ".rsboot.bak"):
        os.system(f"cp '{path}' '{path}.rsboot.bak'")


def insert_after(path, ref, new_line):
    with open(path) as f:
        lines = f.read().splitlines(keepends=True)
    if any(new_line.strip() in ln for ln in lines):
        print(f"{path} already edited, skipping")
        return
    for i, line in enumerate(lines):
        if ref in line:
            if not line.endswith("\n"):
                lines[i] += "\n"
            lines.insert(i + 1, new_line + "\n")
            backup(path)
            with open(path, "w") as f:
                f.write("".join(lines))
            print(f"edited {path}")
            return
    sys.exit(f"error: ref '{ref}' not found in {path}, please do it manually")


def patch_bds():
    with open(BDS) as f:
        text = f.read()
    if "RsbootFileGuid" in text:
        print(f"{BDS} already has rsboot, skipping")
        return
    out, d1, d2 = [], False, False
    for line in text.splitlines(keepends=True):
        if not d1 and "EFI_BOOT_MODE" in line and "BootMode" in line and ";" in line:
            out.append(line if line.endswith("\n") else line + "\n")
            out.append(f"EFI_GUID RsbootFileGuid = {GUID_C};\n")
            d1 = True
            continue
        if not d2 and all(s in line for s in ("RemoveStaleFvFileOptions", "(", ")", ";")):
            out.append("// Register rsboot\n")
            out.append('PlatformRegisterFvBootOption (&RsbootFileGuid, L"rsboot", LOAD_OPTION_ACTIVE, TRUE);\n\n')
            out.append(line)
            d2 = True
            continue
        out.append(line)
    if d1 and d2:
        backup(BDS)
        with open(BDS, "w") as f:
            f.write("".join(out))
        print(f"edited {BDS}")
    else:
        sys.exit(f"error: refs not found in {BDS}, please do it manually")


if len(sys.argv) != 2:
    sys.exit("usage: embed-rsboot.py /path/to/rsboot.efi")
rsboot_efi = sys.argv[1]
for p in (DSC, FDF, BDS):
    if not os.path.isfile(p):
        sys.exit(f"error: {p} not found: run from the edk2 root")
if not os.path.isfile(rsboot_efi):
    sys.exit(f"error: rsboot binary not found at {rsboot_efi}")

os.makedirs(RSDIR, exist_ok=True)
os.system(f"cp '{rsboot_efi}' '{RSDIR}/Rsboot.efi'")

with open(INF,"w") as f:
    f.write(
    "[Defines]\n"
    "INF_VERSION = 0x00010005\n"
    "BASE_NAME = Rsboot\n"
    f"FILE_GUID = {GUID}\n"
    "MODULE_TYPE = UEFI_APPLICATION\n"
    "VERSION_STRING = 1.0\n\n"
    "[Binaries.X64]\n"
    "PE32|Rsboot.efi|*\n")
print(f"wrote {INF} and copied Rsboot.efi")

insert_after(DSC, "MdeModulePkg/Logo/LogoDxe.inf", "OvmfPkg/Rsboot/Rsboot.inf")
insert_after(FDF, "INF  MdeModulePkg/Application/UiApp/UiApp.inf", "INF  OvmfPkg/Rsboot/Rsboot.inf")
patch_bds()

print("\nAll edits applied. Build with:")
print("source ./edksetup.sh")
print(" build -a X64 -t GCC -b RELEASE -p OvmfPkg/OvmfPkgX64.dsc -D NETWORK_HTTP_BOOT_ENABLE=TRUE")
