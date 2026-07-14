use uefi::system;

pub fn draw_menu(selected: usize, items: &[&str]) {
    system::with_stdout(|out| {
        out.clear().unwrap();

        for (i, item) in items.iter().enumerate() {
            let marker = if i == selected { "* " } else { " " };

            uefi::println!("{marker}{item}");
        }
    });
}
