use core::fmt::Write;
use uefi::proto::console::text::Color;
use uefi::system;

use crate::environment;

const BACKGROUND: Color = Color::Black;

pub fn draw_menu(selected: usize, items: &[&str], env: &environment::ENV) {
    system::with_stdout(|out| {
        out.clear().unwrap();
        draw_rect(out);
        match env {
            environment::ENV::MENU => {
                for (i, item) in items.iter().enumerate() {
                    if i == selected {
                        out.set_color(Color::Magenta, BACKGROUND).unwrap();
                    } else {
                        out.set_color(Color::LightGray, BACKGROUND).unwrap();
                    };

                    center_line(out, item, i);

                    out.write_fmt(format_args!("{item}\n")).unwrap();
                }
            }
            environment::ENV::OS => {
                uefi::println!("  {}", items[selected]);
            }
        }
    });
}

pub fn get_dimensions(out: &uefi::proto::console::text::Output) -> (usize, usize) {
    let (cols, rows) = out
        .current_mode()
        .ok()
        .flatten()
        .map(|m| (m.columns(), m.rows()))
        .unwrap();
    (cols, rows)
}
pub fn center_line(out: &mut uefi::proto::console::text::Output, text: &str, row_index: usize) {
    let (cols, rows) = get_dimensions(out);
    let col = cols.saturating_sub(text.chars().count()) / 2;
    let row = rows / 3;
    out.set_cursor_position(col, row + row_index).unwrap();
}

pub fn draw_rect(out: &mut uefi::proto::console::text::Output) {
    let (cols, rows) = get_dimensions(out);
    out.set_color(Color::Black, Color::LightGray).unwrap();
    let col_start = cols / 5;
    let col_end = cols - col_start;
    let width = col_end - col_start;

    let row_start = rows / 5;
    let row_end = rows - row_start;

    for r in row_start..row_end {
        out.set_cursor_position(col_start, r).unwrap();
        out.write_fmt(format_args!("{:width$}", "")).unwrap();
    }
}
