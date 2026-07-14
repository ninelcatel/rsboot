use core::fmt::Write;
use uefi::proto::console::text::Color;
use uefi::system;

use crate::environment;

const RECT: Color = Color::LightGray;
const TITLE: &str = "Welcome to";
const SUBTITLE: &str = "Please select the OS you wish to install.";

pub fn clear_screen() {
    system::with_stdout(|out| {
        out.set_color(Color::LightGray, Color::Black).unwrap(); // set colors to default
        out.clear().unwrap();
    });
}

pub fn draw_menu(selected: usize, items: &[&str], env: &environment::ENV) {
    system::with_stdout(|out| {
        draw_rect(out);

        draw_title(out);

        match env {
            environment::ENV::MENU => {
                for i in 0..items.len() {
                    draw_item(out, items, i, selected);
                }
            }
            environment::ENV::OS => {
                out.set_color(Color::Magenta, RECT).unwrap();

                center_line(out, items[selected], 0);

                out.write_fmt(format_args!("{}", items[selected])).unwrap();
            }
        }
    });
}

fn draw_item(
    out: &mut uefi::proto::console::text::Output,
    items: &[&str],
    i: usize,
    selected: usize,
) {
    if i == selected {
        out.set_color(Color::Magenta, RECT).unwrap();
    } else {
        out.set_color(Color::Black, RECT).unwrap();
    }
    center_line(out, items[i], i);
    out.write_fmt(format_args!("{}", items[i])).unwrap();
}

// helper function to overwrite only the 2 affected selections
pub fn update_selection(items: &[&str], old: usize, new: usize) {
    system::with_stdout(|out| {
        draw_item(out, items, old, new);
        draw_item(out, items, new, new);
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

pub fn draw_title(out: &mut uefi::proto::console::text::Output) {
    let (cols, rows) = get_dimensions(out);
    let col = cols.saturating_sub(TITLE.chars().count() + 8) / 2; // + 8 de la " rsboot!"
    out.set_color(Color::Red, RECT).unwrap();

    out.set_cursor_position(col, rows / 5).unwrap();
    out.write_fmt(format_args!("{TITLE}")).unwrap();

    out.set_cursor_position(col + TITLE.len() + 1, rows / 5)
        .unwrap();
    out.set_color(Color::Yellow, RECT).unwrap();
    out.write_fmt(format_args!("rsboot")).unwrap();

    out.set_cursor_position(col + TITLE.len() + 7, rows / 5)
        .unwrap();
    out.set_color(Color::Red, RECT).unwrap();
    out.write_fmt(format_args!("!")).unwrap();

    let col2 = cols.saturating_sub(SUBTITLE.chars().count()) / 2;

    out.set_cursor_position(col2, rows / 5 + 1).unwrap();
    out.write_fmt(format_args!("{SUBTITLE}")).unwrap();
}

pub fn center_line(out: &mut uefi::proto::console::text::Output, text: &str, row_index: usize) {
    let (cols, rows) = get_dimensions(out);
    let col = cols.saturating_sub(text.chars().count()) / 2;
    let row = rows / 3;
    out.set_cursor_position(col, row + row_index).unwrap();
}

pub fn draw_rect(out: &mut uefi::proto::console::text::Output) {
    let (cols, rows) = get_dimensions(out);
    out.set_color(Color::Black, RECT).unwrap();
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
