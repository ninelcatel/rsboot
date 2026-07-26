#![no_main]
#![no_std]

use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod draw;
mod environment;
mod loadbootable;

use environment::Env;

use crate::loadbootable::load_bootable;

const OS: [&str; 4] = ["Debian", "Arch Linux", "Ubuntu", "Fedora"];

#[entry]
fn main() -> Status {
    match run() {
        Ok(()) => Status::SUCCESS,
        Err(e) => e.status(),
    }
}

fn run() -> uefi::Result {
    uefi::helpers::init()?;

    let mut running: bool = true;

    let enter: Char16 = Char16::try_from('\r').expect("'\\r' should always be a valid char");

    let mut env = Env::Menu;
    let mut current_pick: usize = 0;

    system::with_stdout(|out| {
        let mut tui = draw::Tui::new(out);
        tui.clear_screen();
        tui.draw_menu(current_pick, &OS, &env);

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
                        tui.update_selection(&OS, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::DOWN)) => {
                    let old = current_pick;
                    if current_pick + 1 < OS.len() {
                        current_pick += 1;
                        tui.update_selection(&OS, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::ESCAPE)) => {
                    running = false;
                    tui.clear_screen();
                }
                (Env::Menu, Key::Printable(c)) if c == enter => {
                    // env = Env::Os;
                    load_bootable(uefi::cstr16!("\\debian.iso")).ok();
                }
                (Env::Os, Key::Special(ScanCode::ESCAPE)) => {
                    env = Env::Menu;
                }
                _ => continue,
            }

            tui.draw_menu(current_pick, &OS, &env);
        }
    });

    Ok(())
}
