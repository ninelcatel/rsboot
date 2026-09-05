extern crate alloc;

const PAGE_ALLIGNER: usize = 2 << 10 << 10; // 2 MB
// the whole iso, living in one contiuous allocation (RESERVED so the booted OS
// won't reuse it). write the download straight into it, so there's
// never a second copy at boot time, was leading to OOM panics.
// previously was building the vector, then allocated the pages with the FULL vector
// now we do it directly into the allocated pages
pub struct IsoBuffer {
    base: core::ptr::NonNull<u8>,
    len: usize,
    mapped: usize, // need this for the memmap bootmethod but also is a cleaner allocation in memory
    cap: usize,
    raw: core::ptr::NonNull<u8>,
    pages: usize,
}

impl IsoBuffer {
    pub fn new(len: usize) -> uefi::Result<Self> {
        let mapped = len
            .checked_next_multiple_of(PAGE_ALLIGNER)
            .ok_or(uefi::Status::OUT_OF_RESOURCES)?;
        let pages = (mapped
            .checked_add(PAGE_ALLIGNER)
            .ok_or(uefi::Status::OUT_OF_RESOURCES)?)
        .div_ceil(uefi::boot::PAGE_SIZE);
        let raw = uefi::boot::allocate_pages(
            uefi::boot::AllocateType::AnyPages,
            uefi::boot::MemoryType::RESERVED,
            pages,
        )?;

        let alligned = (raw.as_ptr() as usize).next_multiple_of(PAGE_ALLIGNER);
        let base =
            core::ptr::NonNull::new(alligned as *mut u8).ok_or(uefi::Status::OUT_OF_RESOURCES)?;

        let cap = pages * uefi::boot::PAGE_SIZE - (alligned - raw.as_ptr() as usize);
        Ok(Self {
            base,
            len,
            mapped,
            cap,
            raw,
            pages,
        })
    }

    // copy  bytes into the buffer at offset
    fn write(&mut self, offset: usize, bytes: &[u8]) -> usize {
        let n = bytes.len().min(self.cap.saturating_sub(offset));
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.base.as_ptr().add(offset), n);
        }
        n
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.base.as_ptr()
    }

    //helper func for everytime i need the iso bytes
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn mapped_len(&self) -> usize {
        self.mapped
    }
}
impl Drop for IsoBuffer {
    fn drop(&mut self) {
        unsafe {
            let _ = uefi::boot::free_pages(self.raw, self.pages);
        }
    }
}
pub struct Downloader {
    nic: uefi::Handle,
}

impl Downloader {
    const LIST_URL: &str = "http://192.168.44.3/os_list";
    pub fn connect() -> uefi::Result<Self> {
        let nic = uefi::boot::get_handle_for_protocol::<uefi::proto::network::http::HttpBinding>()?;
        let mut ip = uefi::proto::network::ip4config2::Ip4Config2::new(nic)?;
        ip.ifup()?;
        Ok(Self { nic })
    }

    pub fn get_os(
        &self,
        url: &str,
        max_bytes: Option<usize>,
        mut in_progress: impl FnMut(usize, usize) -> bool,
    ) -> uefi::Result<IsoBuffer> {
        let (mut http, first) = self.fetch(url)?;

        // we need Content-Length to size the buffer and know when its done
        let len = Self::content_length(&first).ok_or(uefi::Status::UNSUPPORTED)?;

        // this is for the Loop injection method, cpio has a 4GB limit
        // therefore an iso with loop inject > 4GB will crash
        if max_bytes.is_some_and(|m| len > m) {
            return Err(uefi::Status::BUFFER_TOO_SMALL.into());
        }
        let mut iso = IsoBuffer::new(len)?;

        // the first response might have more than the headers, so append to the buffer
        let mut written = iso.write(0, &first.body);
        if written != first.body.len() {
            return Err(uefi::Status::BUFFER_TOO_SMALL.into());
        }
        if !in_progress(written, len) {
            return Err(uefi::Status::ABORTED.into());
        }

        // mutable vector that holds the http payload and is cleared every loop (max 16KB per loop)
        // in order to prevent OOM panics
        let mut data = alloc::vec::Vec::new();
        while written < len {
            data.clear();
            let chunk = http.response_more(&mut data)?;
            if chunk.is_empty() {
                break;
            }
            let n = iso.write(written, chunk);
            written += n;
            if n != chunk.len() {
                // should prevent overflowing
                return Err(uefi::Status::BUFFER_TOO_SMALL.into());
            }
            if !in_progress(written, len) {
                return Err(uefi::Status::ABORTED.into());
            }
        }
        if written < len {
            return Err(uefi::Status::NOT_READY.into());
        }
        Ok(iso)
    }

    // http get to the mirror, if theres anyythign wrong,
    // return None and use built in OS_LIST
    pub fn get_list(&self) -> Option<alloc::string::String> {
        let (mut http, first) = self.fetch(Self::LIST_URL).ok()?;
        let len = Self::content_length(&first)?;

        let mut list = alloc::vec::Vec::with_capacity(len);
        list.extend_from_slice(&first.body);
        let mut data = alloc::vec::Vec::new();
        while list.len() < len {
            data.clear();
            let chunk = http.response_more(&mut data).ok()?;
            if chunk.is_empty() {
                break;
            }
            list.extend_from_slice(chunk);
        }
        if list.len() < len {
            return None; // short read -> fall back to built-in
        }
        alloc::string::String::from_utf8(list).ok()
    }

    // parse Content-Length from response headers
    fn content_length(resp: &uefi::proto::network::http::HttpHelperResponse) -> Option<usize> {
        for (key, val) in &resp.headers {
            if key.eq_ignore_ascii_case("content-length") {
                return val.trim().parse().ok();
            }
        }
        None
    }
    fn fetch(
        &self,
        url: &str,
    ) -> uefi::Result<(
        uefi::proto::network::http::HttpHelper,
        uefi::proto::network::http::HttpHelperResponse,
    )> {
        // one httphelper instance per download, dropped when this fn returs in order to not get
        // Status::TIMEOUT err when restablishing TCP coneciton
        let mut http = uefi::proto::network::http::HttpHelper::new(self.nic)?;
        http.configure()?;

        http.request_get(url)?;
        let first = http.response_first(true)?;
        //translates to if http resp status != http 200, couldve used HttpStatus but i need to
        //import another crate and i cba
        if first.status.0 != 3 {
            return Err(uefi::Status::PROTOCOL_ERROR.into());
        };
        Ok((http, first))
    }
}
