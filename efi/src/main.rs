#![no_main]
#![no_std]

use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod draw;
mod environment;

use environment::Env;

const OS: [&str; 4] = ["Debian", "Arch Linux", "Ubuntu", "Fedora"];

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let mut running: bool = true;

    let mut env = Env::Menu;
    let mut current_pick: usize = 0;

    system::with_stdout(|out| {
        let mut tui = draw::Tui::new(out);
        tui.clear_screen();
        tui.draw_menu(current_pick, &OS, &env);

        while running {
            let Ok(Some(key)) = system::with_stdin(|stdin| stdin.read_key()) else {
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
                (Env::Menu, Key::Printable(c)) if c == Char16::try_from('\r').unwrap() => {
                    env = Env::Os;
                }
                (Env::Os, Key::Special(ScanCode::ESCAPE)) => {
                    env = Env::Menu;
                }
                _ => continue,
            }

            tui.draw_menu(current_pick, &OS, &env);
        }
    });

    Status::SUCCESS
}
