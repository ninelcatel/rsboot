extern crate alloc;
pub enum Env {
    Menu,
    Os,
}

#[derive(Clone, Copy)]
pub enum BootMethod {
    RamDisk,
    Memmap,
    Netboot,
    LoopInjection,
}

impl BootMethod {
    pub fn max_bytes(&self) -> Option<usize> {
        match self {
            BootMethod::LoopInjection => Some(0xFFFF_FFFF),
            _ => None,
        }
    }
}

pub struct OsChild<'a> {
    pub edition: Option<&'a str>, // KDE, XFCE, OpenRC/Dinit
    pub version: &'a str,
    pub url: &'a str,
    pub os_type: &'a str,
    pub sha256: Option<&'a str>,
    pub boot_method: BootMethod,
}
pub struct OS<'a> {
    pub name: &'a str,
    pub children: alloc::vec::Vec<OsChild<'a>>,
}

pub fn get_list<'a>(public_list: Option<&'a str>) -> alloc::vec::Vec<OS<'a>> {
    let mut families: alloc::vec::Vec<OS<'a>> = alloc::vec::Vec::new();
    let list = public_list.unwrap_or(include_str!("../assets/os_list"));

    for line in list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("#") {
            continue;
        }
        // an entry looks like this:
        // family | edition | version | image type | boot method | sha256 | url
        let mut p = line.split('|').map(str::trim);
        let (
            Some(family),
            Some(edition),
            Some(version),
            Some(os_type),
            Some(boot),
            Some(sha256),
            Some(url),
            None,
        ) = (
            p.next(),
            p.next(),
            p.next(),
            p.next(),
            p.next(),
            p.next(),
            p.next(),
            p.next(),
        )
        else {
            continue; // not exactly 7 fields => skip 
        };

        let boot_method = match boot {
            "ramdisk" => BootMethod::RamDisk,
            "memmap" => BootMethod::Memmap,
            "loop" => BootMethod::LoopInjection,
            "netboot" => BootMethod::Netboot,
            _ => continue,
        };
        // ARM images contain "ARM" in their edition.
        // don't expose aarch64 distros for x86_64
        // architecture cpus and vice-versa
        let is_arm = edition.contains("ARM");
        #[cfg(target_arch = "aarch64")]
        if !is_arm {
            continue;
        }
        #[cfg(not(target_arch = "aarch64"))]
        if is_arm {
            continue;
        }
        let edition = match edition {
            "-" => None,
            "" => continue,
            _ => Some(edition),
        };
        let sha256 = match sha256 {
            "-" => None,
            "" => continue,
            _ => Some(sha256),
        };
        let child = OsChild {
            edition,
            version,
            url,
            os_type,
            sha256,
            boot_method,
        };
        match families.iter_mut().find(|f| f.name == family) {
            Some(f) => f.children.push(child),
            None => families.push(OS {
                name: family,
                children: alloc::vec![child],
            }),
        }
    }
    families
}

pub struct BootConfig {
    pub kernel_path: alloc::string::String,
    pub initrd_path: alloc::string::String,
    pub cmdline: alloc::string::String,
}
