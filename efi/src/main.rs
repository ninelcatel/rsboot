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
extern crate alloc;
use crate::boot::boot;

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
    let mut family_picked: usize = 0;
    let os = environment::get_list();
    let names: alloc::vec::Vec<&str> = os.iter().map(|o| o.name).collect();
    let mut children_names: alloc::vec::Vec<alloc::string::String> = alloc::vec![];

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
                (Env::Os, Key::Special(ScanCode::UP)) => {
                    let old = current_pick;
                    current_pick = current_pick.saturating_sub(1);
                    if current_pick != old {
                        tui.update_selection(&children_names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::DOWN)) => {
                    let old = current_pick;
                    if current_pick + 1 < os.len() {
                        current_pick += 1;
                        tui.update_selection(&names, old, current_pick);
                    }
                    continue;
                }
                (Env::Os, Key::Special(ScanCode::DOWN)) => {
                    let old = current_pick;
                    if current_pick + 1 < os[family_picked].children.len() {
                        current_pick += 1;
                        tui.update_selection(&children_names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::ESCAPE)) => {
                    running = false;
                    tui.clear_screen();
                }
                (Env::Menu, Key::Printable(c)) if c == enter => {
                    // enter the version picker menu for cufrent os

                    family_picked = current_pick;
                    current_pick = 0;
                    env = Env::Os;
                    children_names = os[family_picked]
                        .children
                        .iter()
                        .map(|f| {
                            alloc::format!(
                                "{} {} {} {}",
                                os[family_picked].name,
                                f.edition.unwrap_or(""),
                                f.version,
                                f.os_type
                            )
                        })
                        .collect();
                }

                (Env::Os, Key::Printable(c)) if c == enter => {
                    let os = &os[family_picked].children[current_pick];
                    tui.begin_download(children_names[current_pick].as_str());
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
                                    children_names.clear();
                                    current_pick = family_picked;
                                    family_picked = 0;
                                }
                            }
                        }
                        // esc pressed mid-download: iso already freed, back to the menu
                        Err(e) if e.status() == Status::ABORTED => {
                            env = Env::Menu;
                            children_names.clear();
                            current_pick = family_picked;
                            family_picked = 0;
                        }
                        Err(e) => {
                            tui.show_error(e.status());
                            env = Env::Menu;
                            children_names.clear();
                            current_pick = family_picked;
                            family_picked = 0;
                        }
                    }
                }
                (Env::Os, Key::Special(ScanCode::ESCAPE)) => {
                    env = Env::Menu;
                    children_names.clear();
                    current_pick = family_picked;
                    family_picked = 0;
                }
                _ => continue,
            }

            match env {
                Env::Menu => tui.draw_menu(current_pick, &names, &env),
                Env::Os => tui.draw_menu(current_pick, &children_names, &env),
            }
        }
    });

    Ok(())
}
