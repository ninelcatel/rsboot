// SPDX-License-Identifier: BSD-3-Clause
// Copyright (c) 2026 Alexandru-Nicolas Negrișan

extern crate alloc;

// make the appended hook run first
pub(super) const GENTOO_HOOK: &str = include_str!("../../assets/gentoo.sh");
pub(super) const HOOK_PATH: &str = "usr/lib/dracut/hooks/pre-trigger/00-rsboot.sh";

pub(super) fn build_cpio(files: &[(&str, &[u8], u32)]) -> alloc::vec::Vec<u8> {
    fn record(out: &mut alloc::vec::Vec<u8>, ino: u32, mode: u32, name: &str, data: &[u8]) {
        let header = alloc::format!(
            "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            ino,
            mode,
            0,
            0,
            1,
            0,
            data.len(),
            0,
            0,
            0,
            0,
            name.len() + 1,
            0
        );
        // header format containing metadata
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(0); // \0 end of line
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        out.extend_from_slice(data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }

    let mut out = alloc::vec::Vec::new();
    let mut ino = 1u32;
    let mut added: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for &(path, data, mode) in files {
        let mut prefix = alloc::string::String::new();
        let comps: alloc::vec::Vec<&str> = path.split('/').collect();
        for comp in &comps[..comps.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(comp);
            if !added.contains(&prefix) {
                record(&mut out, ino, 0o040755, &prefix, &[]); // its a directory, no data
                ino += 1;
                added.push(prefix.clone())
            }
        }
        record(&mut out, ino, mode, path, data);
        ino += 1;
    }
    record(&mut out, ino, 0, "TRAILER!!!", &[]); //EOF marker
    out
}
