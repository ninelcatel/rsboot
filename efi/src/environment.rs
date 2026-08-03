#[allow(dead_code)]
pub enum Env {
    Menu,
    Os, // SOMETHING_ELSE,
}

#[allow(dead_code)]
pub enum BootMethod {
    RamDisk,
    Memmap,
    Netboot,
    LoopInjection,
}
