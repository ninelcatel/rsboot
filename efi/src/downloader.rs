extern crate alloc;
pub struct Downloader {
    nic: uefi::Handle,
    http: uefi::proto::network::http::HttpHelper,
}

impl Downloader {
    pub fn connect() -> uefi::Result<Self> {
        let nic = uefi::boot::get_handle_for_protocol::<uefi::proto::network::http::HttpBinding>()?;
        let mut ip = uefi::proto::network::ip4config2::Ip4Config2::new(nic)?;
        ip.ifup()?;
        let mut http = uefi::proto::network::http::HttpHelper::new(nic)?;
        http.configure()?;
        Ok(Self { nic, http })
    }
    pub fn get(&mut self, url: &str) -> uefi::Result<alloc::vec::Vec<u8>> {
        self.http.request_get(url)?;
        let mut content = self.http.response_first(true).unwrap().body;
        loop {
            let payload = self.http.response_more(&mut content)?;
            if payload.is_empty() {
                break;
            }
        }
        Ok(content)
    }
}
