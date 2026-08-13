extern crate alloc;
pub enum Env {
    Menu,
    Os, // SOMETHING_ELSE,
}

#[derive(Clone, Copy)]
pub enum BootMethod {
    RamDisk,
    Memmap,
    Netboot,
    LoopInjection,
}

pub struct OS<'a> {
    pub name: &'a str,
    pub url: &'a str,
    pub boot_method: BootMethod,
}

pub struct BootConfig {
    pub kernel_path: alloc::string::String,
    pub initrd_path: alloc::string::String,
    pub cmdline: alloc::string::String,
}
