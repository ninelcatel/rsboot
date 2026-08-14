#![no_main]
#![no_std]

use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod boot;
mod checksum;
mod downloader;
mod draw;
mod environment;
mod handlers;

use environment::Env;

use crate::boot::boot;

const OS: [environment::OS; 11] = [
    environment::OS {
        name: "Debian",
        url: "http://ftp2.de.debian.org/debian/dists/trixie/main/installer-amd64/20250803+deb13u6/images/netboot/mini.iso",
        sha256: Some("3cedd6f417cab308e2a9fbf4273435849ceeffef4568dc764c24c394fb815483"),
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "Arch Linux",
        url: "http://10.0.2.2:8000/arch.iso",
        sha256: None,
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "Ubuntu",
        url: "http://10.0.2.2:8000/ubuntu.iso",
        sha256: None,
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "Fedora",
        url: "http://10.0.2.2:8000/fedora.iso",
        sha256: None,
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "CachyOS",
        url: "http://10.0.2.2:8000/cachy.iso",
        sha256: None,
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "Gentoo",
        url: "http://10.0.2.2:8000/gentoo_gui.iso",
        sha256: None,
        boot_method: environment::BootMethod::LoopInjection,
    },
    environment::OS {
        name: "NixOS",
        url: "http://10.0.2.2:8000/nixos.efi",
        sha256: None,
        boot_method: environment::BootMethod::Netboot,
    },
    environment::OS {
        name: "Artix",
        url: "http://10.0.2.2:8000/artix.iso",
        sha256: None,
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "openSUSE",
        url: "http://10.0.2.2:8000/opensuse.iso",
        sha256: None,
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "OpenBSD",
        url: "http://10.0.2.2:8000/openbsd.iso",
        sha256: Some("7a4a92e953618035097c796a90b54424a0f3ae775552e1e7d102cf8a5130449f"),
        boot_method: environment::BootMethod::RamDisk,
    },
    // this is actually mfsBSD, but has FreeBSD kernel and userland
    // read more at https://mfsbsd.vx.sk/
    // login: root:mfsroot
    environment::OS {
        name: "FreeBSD",
        url: "http://10.0.2.2:8000/freebsd_mfs.iso",
        sha256: Some("2803be01ef284cb4d58c9177475c7a20ac72292e4943bc91eb159c592bfc3b5c"),
        boot_method: environment::BootMethod::RamDisk,
    },
];

#[entry]
fn main() -> Status {
    match run() {
        Ok(()) => Status::SUCCESS,
        Err(e) => e.status(),
    }
}
fn run() -> uefi::Result {
    uefi::helpers::init()?;
    log::set_max_level(log::LevelFilter::Info); // without this, it adds unnecessarry buffering and
    // downloads REALLY slow
    let mut running: bool = true;

    let enter: Char16 = Char16::try_from('\r').expect("'\\r' should always be a valid char");

    let mut env = Env::Menu;
    let mut current_pick: usize = 0;

    system::with_stdout(|out| {
        let mut tui = draw::Tui::new(out);

        //  bring the network up before the menu
        //  goes into main menu only if dhcp is up
        let dl = loop {
            tui.begin_dhcp();
            match downloader::Downloader::connect() {
                Ok(dl) => break dl,
                // ESC on the DHCP error screen quits
                Err(e) => {
                    if matches!(
                        tui.show_error(e.status()),
                        Some(Key::Special(ScanCode::ESCAPE))
                    ) {
                        tui.clear_screen();
                        return;
                    }
                }
            }
        };

        let names: [&str; OS.len()] = core::array::from_fn(|i| OS[i].name);
        tui.draw_menu(current_pick, &names, &env);

        while running {
            let key = system::with_stdin(|input| {
                if let Ok(event) = input.wait_for_key_event() {
                    let _ = uefi::boot::wait_for_event(&mut [event]);
                }
                input.read_key()
            });
            let Ok(Some(key)) = key else {
                continue;
            };

            match (&env, key) {
                (Env::Menu, Key::Special(ScanCode::UP)) => {
                    let old = current_pick;
                    current_pick = current_pick.saturating_sub(1);
                    if current_pick != old {
                        tui.update_selection(&names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::DOWN)) => {
                    let old = current_pick;
                    if current_pick + 1 < OS.len() {
                        current_pick += 1;
                        tui.update_selection(&names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::ESCAPE)) => {
                    running = false;
                    tui.clear_screen();
                }
                (Env::Menu, Key::Printable(c)) if c == enter => {
                    // enter the version picker menu for cufrent os
                    env = Env::Os;
                }
                (Env::Os, Key::Printable(c)) if c == enter => {
                    let os = &OS[current_pick];
                    tui.begin_download(os.name);
                    let mut last = usize::MAX;
                    let result = dl.get(os.url, os.boot_method.max_bytes(), |written, total| {
                        let pct = (written * 100).checked_div(total).unwrap_or(0);
                        if pct != last {
                            last = pct;
                            tui.draw_progress(written, total);
                        }
                        // aborts download
                        let key = system::with_stdin(|input| input.read_key());
                        !matches!(key, Ok(Some(Key::Special(ScanCode::ESCAPE))))
                    });
                    match result {
                        Ok(buffer) => {
                            // integrity check
                            let verified = match os.sha256 {
                                Some(hex) => checksum::verify_sha256(buffer.as_slice(), hex),
                                None => Ok(()),
                            };
                            match verified.and_then(|_| boot(buffer, os.boot_method)) {
                                Ok(()) => {}
                                Err(e) => {
                                    tui.show_error(e.status());
                                    env = Env::Menu;
                                }
                            }
                        }
                        // esc pressed mid-download: iso already freed, back to the menu
                        Err(e) if e.status() == Status::ABORTED => {
                            env = Env::Menu;
                        }
                        Err(e) => {
                            tui.show_error(e.status());
                            env = Env::Menu;
                        }
                    }
                }
                (Env::Os, Key::Special(ScanCode::ESCAPE)) => {
                    env = Env::Menu;
                }
                _ => continue,
            }

            tui.draw_menu(current_pick, &names, &env);
        }
    });

    Ok(())
}
