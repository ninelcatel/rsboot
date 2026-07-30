extern crate alloc;

// the whole iso, living in one contiuous allocation (RESERVED so the booted OS
// won't reuse it). write the download straight into it, so there's
// never a second copy at boot time, was leading to OOM panics.
// previously was building the vector, then allocated the pages with the FULL vector
// now we do it directly into the allocated pages
pub struct IsoBuffer {
    base: core::ptr::NonNull<u8>,
    len: usize,
}

impl IsoBuffer {
    pub fn new(len: usize) -> uefi::Result<Self> {
        let pages = len.div_ceil(uefi::boot::PAGE_SIZE).max(1);
        let base = uefi::boot::allocate_pages(
            uefi::boot::AllocateType::AnyPages,
            uefi::boot::MemoryType::RESERVED,
            pages,
        )?;
        Ok(Self { base, len })
    }
    // this is used for the load_bootable method, will be needded for OSs that ship via .efi even
    // though its not iso, example: arch, its bootloader doesnt have a certain kernel module which
    // makes it not possible to boot unless we change the initramfs
    pub fn from_bytes(bytes: &[u8]) -> uefi::Result<Self> {
        let iso = Self::new(bytes.len())?;
        iso.write(0, bytes);
        Ok(iso)
    }

    // copy  bytes into the buffer at offset
    fn write(&self, offset: usize, bytes: &[u8]) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.base.as_ptr().add(offset),
                bytes.len(),
            );
        }
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.base.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

pub struct Downloader {
    http: uefi::proto::network::http::HttpHelper,
}

impl Downloader {
    pub fn connect() -> uefi::Result<Self> {
        let nic = uefi::boot::get_handle_for_protocol::<uefi::proto::network::http::HttpBinding>()?;
        let mut ip = uefi::proto::network::ip4config2::Ip4Config2::new(nic)?;
        ip.ifup()?;
        let mut http = uefi::proto::network::http::HttpHelper::new(nic)?;
        http.configure()?;
        Ok(Self { http })
    }

    pub fn get(&mut self, url: &str) -> uefi::Result<IsoBuffer> {
        self.http.request_get(url)?;
        let first = self.http.response_first(true)?;

        // we need Content-Length to size the buffer and know when its done
        let mut len = 0;
        for (key, val) in &first.headers {
            if key.eq_ignore_ascii_case("content-length") {
                len = val.parse::<usize>().unwrap_or(0);
                break;
            }
        }

        let iso = IsoBuffer::new(len)?;

        // the first response might have more than the headers, so append to the buffer
        let mut written = first.body.len();
        iso.write(0, &first.body);

        // mutable vector that holds the http payload and is cleared every loop (max 16KB per loop)
        // in order to prevent OOM panics
        let mut data = alloc::vec::Vec::new();
        while written < len {
            data.clear();
            let chunk = self.http.response_more(&mut data)?;
            if chunk.is_empty() {
                break;
            }
            iso.write(written, chunk);
            written += chunk.len();
        }

        Ok(iso)
    }
}
