extern crate alloc;
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
    pub fn get(&mut self, url: &str) -> uefi::Result<alloc::vec::Vec<u8>> {
        self.http.request_get(url)?;
        let first = self.http.response_first(true)?;

        // need exact length, in order to know when the download is complete

        // TODO: implement logic for http sites that DONT have Content-Length
        let mut len = None;
        for (key, val) in first.headers {
            if key.eq_ignore_ascii_case("content-length") {
                len = val.parse::<usize>().ok();
                break;
            }
        }
        let mut content = first.body;
        if let Some(item) = len {
            content.reserve(item.saturating_sub(content.len()));
            loop {
                if item <= content.len() {
                    break;
                }
                self.http.response_more(&mut content)?;
            }
        } else {
            loop {
                match self.http.response_more(&mut content) {
                    Ok(chunk) if chunk.is_empty() => break,
                    Ok(_) => {}
                    Err(e) => {
                        log::info!("Error in downloading the file! {e}");
                        break;
                    }
                }
            }
        }
        Ok(content)
    }
}
