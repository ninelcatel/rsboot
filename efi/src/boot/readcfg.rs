// reads the kernel/initrd/cmdline out of the ISO: locate a known bootloader config, parse it,
// and read files from the ISO9660filesystem.

extern crate alloc;

use crate::{downloader::IsoBuffer, environment::BootConfig};

pub(super) const CONFIGS: [&str; 3] = [
    "boot/syslinux/archiso_sys-linux.cfg", // arch / cachy / blackarch (syslinux)
    "boot/grub/grub.cfg",                  // gentoo etc (grub)
    "EFI/BOOT/grub.cfg",                   // opensuse (grub, efi-only)
];

// returns paths as Strings, stopped returning CStr16 due to the new ISO9660 parser using String
pub(super) fn parse_config(config: alloc::vec::Vec<u8>) -> Option<BootConfig> {
    use alloc::string::ToString;

    let text = core::str::from_utf8(&config).unwrap_or("");
    // first entry only: is_none() condition
    // configs list several menu entries,
    let (mut kernel, mut initrd, mut options, mut inline_opts) = (None, None, None, None);

    for line in text.lines() {
        let l = line.trim();
        if kernel.is_none()
            && let Some(s) = l
                .strip_prefix("linux ") // systemd-boot / grub
                .or_else(|| l.strip_prefix("LINUX ")) // syslinux
                .or_else(|| l.strip_prefix("KERNEL "))
        // isolinux
        {
            // grub combines "linux <path> <options>"; syslinux/systemd-boot give just the path
            match s.trim().split_once(char::is_whitespace) {
                Some((path, rest)) => {
                    kernel = Some(path.to_string());
                    inline_opts = Some(rest.trim().to_string());
                }
                None => kernel = Some(s.trim().to_string()),
            }
        }
        if initrd.is_none()
            && let Some(s) = l
                .strip_prefix("initrd ") // systemd-boot / grub
                .or_else(|| l.strip_prefix("INITRD "))
        // syslinux/isolinux
        {
            initrd = Some(s.trim().to_string());
        }
        if options.is_none()
            && let Some(s) = l
                .strip_prefix("options ") // systemd-boot
                .or_else(|| l.strip_prefix("APPEND "))
        // syslinux/isolinux
        {
            options = Some(s.trim().to_string());
        }
    }
    Some(BootConfig {
        kernel_path: kernel?,
        initrd_path: initrd?,
        cmdline: options.or(inline_opts)?,
    })
}

pub(super) fn read_iso(iso: &IsoBuffer, path: &str) -> Option<alloc::vec::Vec<u8>> {
    let bytes = iso.as_slice();
    let cursor = hadris_io::Cursor::new(bytes);

    let img = hadris_iso::sync::IsoImage::open(cursor).ok()?;
    let mut dir_ref = img.root_dir().dir_ref();
    let mut parts = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty()) // checks for paths//like//this
        .peekable(); // secventially
    // go through the directories, thats how the parser works
    while let Some(part) = parts.next() {
        let dir = img.open_dir(dir_ref);
        let entry = dir
            .entries()
            .filter_map(|e| e.ok())
            .find(|e| e.display_name().eq_ignore_ascii_case(part))?;
        if parts.peek().is_none() {
            // if there arent directories left to navigate, it means we are
            // at the destination file
            let ext = entry.extents().next()?;
            let start = ext.sector.0.checked_mul(2048)?;
            let len = ext.length as usize;
            return Some(bytes.get(start..start.checked_add(len)?)?.to_vec());
        }
        dir_ref = entry.as_dir_ref(&img).ok()?;
    }
    None
}

// black magic to read the pvd label for artix or similar arch+grub distros
pub(super) fn read_label(iso: &IsoBuffer) -> Option<alloc::string::String> {
    let bytes = iso.as_slice();

    if bytes.get(0x8000)? != &1 || bytes.get(0x8001..0x8006)? != b"CD001" {
        return None;
    }
    let label = core::str::from_utf8(bytes.get(0x8028..0x8028 + 32)?)
        .ok()?
        .trim();
    if !label.is_empty() {
        return Some(label.into());
    }
    None
}
pub(super) fn grub_fallback(iso: &IsoBuffer) -> Option<BootConfig> {
    let label = read_label(iso)?;
    Some(BootConfig {
        kernel_path: alloc::string::String::from("boot/vmlinuz-x86_64"),
        initrd_path: alloc::string::String::from("boot/initramfs-x86_64.img"),
        cmdline: alloc::format!("label={label}"),
    })
}
