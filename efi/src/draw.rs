use core::fmt::Write;
use uefi::{
    proto::console::text::{Color, Key, Output},
    system::with_stdin,
};

use crate::environment;

const RECT: Color = Color::LightGray;
const TITLE: &str = "Welcome to";
const SUBTITLE: &str = "Please select the OS you wish to install.";
const PROJECT_TITLE: &str = "rsboot";

pub struct Tui<'a> {
    out: &'a mut Output,
    cols: usize,
    rows: usize,
    layout_drawn: bool, // flag to check if dimensions were changed and need a FULL redraw
}

impl<'a> Tui<'a> {
    pub fn new(out: &'a mut Output) -> Self {
        let (cols, rows) = Tui::get_dimensions(out);
        Self {
            out,
            cols,
            rows,
            layout_drawn: false,
        }
    }

    pub fn clear_screen(&mut self) {
        self.out.set_color(Color::LightGray, Color::Black).ok(); // set colors to default
        self.out.clear().ok();
        self.layout_drawn = false;
    }

    // draw the rect adn title once; repaint only if the resolution changed
    fn check_layout(&mut self) {
        let (cols, rows) = Tui::get_dimensions(self.out);
        if (cols, rows) != (self.cols, self.rows) {
            self.cols = cols;
            self.rows = rows;
            self.layout_drawn = false;
        }
        if !self.layout_drawn {
            self.clear_screen();
            self.draw_rect();
            self.draw_title();
            self.layout_drawn = true;
        }
    }

    // clear only the inside of the rect
    fn clear_body(&mut self) {
        self.out.set_color(Color::Black, RECT).ok();
        let col_start = self.cols / 5;
        let col_end = self.cols.saturating_sub(col_start);
        let width = col_end.saturating_sub(col_start);
        let row_start = self.rows / 5;
        let row_end = self.rows.saturating_sub(self.rows / 8);
        for r in row_start..row_end {
            self.out.set_cursor_position(col_start, r).ok();
            self.out.write_fmt(format_args!("{:width$}", "")).ok();
        }
    }

    // items can be &[&str] (family names) or &[String]
    pub fn draw_menu<S: AsRef<str>>(
        &mut self,
        selected: usize,
        items: &[S],
        _env: &environment::Env,
    ) {
        self.check_layout();
        self.clear_body();

        for i in 0..items.len() {
            self.draw_item(items, i, selected);
        }
    }

    // new render, rectangle for downloading screen
    pub fn begin_download(&mut self, name: &str) {
        self.check_layout();
        self.clear_body();

        self.out.set_color(Color::Magenta, RECT).ok();
        let line_len = "Downloading ".len() + name.len() + "...".len();
        self.center(line_len, self.rows / 3);
        self.out
            .write_fmt(format_args!("Downloading {name}..."))
            .ok();
    }

    // repaints the progress bar
    pub fn draw_progress(&mut self, written: usize, total: usize) {
        const BAR: usize = 40;
        let pct = (written * 100).checked_div(total).unwrap_or(0);
        let filled = BAR * pct / 100;

        let width = BAR + 2 + 5;
        let row = self.rows / 2;

        self.out.set_color(Color::Green, RECT).ok();
        self.center(width, row);
        self.out.write_fmt(format_args!("[")).ok();
        for i in 0..BAR {
            let c = if i < filled { '#' } else { '-' };
            self.out.write_fmt(format_args!("{c}")).ok();
        }
        self.out.write_fmt(format_args!("] {pct:>3}%")).ok();

        const MIB: usize = 1024 * 1024;
        const CNT_LEN: usize = 19; // "NNNNNN / NNNNNN MiB"
        self.out.set_color(Color::Black, RECT).ok();
        self.center(CNT_LEN, row + 1);
        self.out
            .write_fmt(format_args!(
                "{:>6} / {:>6} MiB",
                written / MIB,
                total / MIB
            ))
            .ok();
    }

    fn draw_item<S: AsRef<str>>(&mut self, items: &[S], i: usize, selected: usize) {
        let Some(item) = items.get(i) else { return };
        let item = item.as_ref();
        if i == selected {
            self.out.set_color(Color::Magenta, RECT).ok();
        } else {
            self.out.set_color(Color::Black, RECT).ok();
        }
        self.center(item.len(), self.rows / 4 + i);
        self.out.write_fmt(format_args!("{item}")).ok();
    }

    // helper function to overwrite only the 2 affected selections
    pub fn update_selection<S: AsRef<str>>(&mut self, items: &[S], old: usize, new: usize) {
        self.draw_item(items, old, new);
        self.draw_item(items, new, new);
    }

    fn get_dimensions(out: &Output) -> (usize, usize) {
        out.current_mode()
            .ok()
            .flatten()
            .map(|m| (m.columns(), m.rows()))
            .unwrap_or((80, 25))
    }

    fn draw_title(&mut self) {
        let col = self
            .cols
            .saturating_sub(TITLE.len() + PROJECT_TITLE.len() + 2)
            / 2; // + 2 pentru spatii!"
        self.out.set_color(Color::Red, RECT).ok();

        self.out.set_cursor_position(col, self.rows / 8).ok();
        self.out.write_fmt(format_args!("{TITLE}")).ok();

        self.out
            .set_cursor_position(col + TITLE.len() + 1, self.rows / 8)
            .ok();
        self.out.set_color(Color::Yellow, RECT).ok();
        self.out.write_fmt(format_args!("{PROJECT_TITLE}")).ok();

        self.out
            .set_cursor_position(col + TITLE.len() + PROJECT_TITLE.len() + 1, self.rows / 8)
            .ok();
        self.out.set_color(Color::Red, RECT).ok();
        self.out.write_fmt(format_args!("!")).ok();

        let col2 = self.cols.saturating_sub(SUBTITLE.len()) / 2;

        self.out.set_cursor_position(col2, self.rows / 8 + 1).ok();
        self.out.write_fmt(format_args!("{SUBTITLE}")).ok();
    }

    fn center(&mut self, len: usize, row: usize) {
        let col = self.cols.saturating_sub(len) / 2;
        self.out.set_cursor_position(col, row).ok();
    }

    fn draw_rect(&mut self) {
        self.out.set_color(Color::Black, RECT).ok();
        let col_start = self.cols / 5;
        let col_end = self.cols.saturating_sub(col_start);
        let width = col_end.saturating_sub(col_start);

        let row_start = self.rows / 8;
        let row_end = self.rows.saturating_sub(row_start);

        for r in row_start..row_end {
            self.out.set_cursor_position(col_start, r).ok();
            self.out.write_fmt(format_args!("{:width$}", "")).ok();
        }
    }
    // startup screen shown while dhcp comes up
    pub fn begin_dhcp(&mut self) {
        self.check_layout();
        self.clear_body();
        self.out.set_color(Color::Magenta, RECT).ok();
        let msg = "Establishing DHCP...";
        self.center(msg.len(), self.rows / 3);
        self.out.write_fmt(format_args!("{msg}")).ok();
    }

    // draws the error, blocks for a key, and returns which key was pressed (so callers
    // like the DHCP screen can treat ESC specially)
    pub fn show_error(&mut self, status: uefi::Status) -> Option<Key> {
        self.check_layout();
        self.clear_body();
        self.out.set_color(Color::Red, RECT).ok();
        let msg = "action failed, press a key to continue (ESC quits)";
        self.center(msg.len(), self.rows / 3);
        self.out.write_fmt(format_args!("{msg}: {status:?}")).ok();
        with_stdin(|input| {
            if let Ok(event) = input.wait_for_key_event() {
                let _ = uefi::boot::wait_for_event(&mut [event]);
            }
            input.read_key().ok().flatten()
        })
    }
}
