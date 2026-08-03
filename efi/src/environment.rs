#[allow(dead_code)]
pub enum Env {
    Menu,
    Os, // SOMETHING_ELSE,
}

#[allow(dead_code)]
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
