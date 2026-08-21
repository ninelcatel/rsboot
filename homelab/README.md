## Basic Homelab setup for `rsboot`

Basically all you need is one or two local servers:
  * PXE (DHCP and TFTP) server
  * Mirror server (i will use `nginx`)

### Instructions: 
#### PXE server (for serving the `rsboot.efi`) 
  1. Install `dnsmasq` package on your PXE server.
  2. Paste the [config](./pxe/dnsmasq.conf) contents to `/etc/dnsmasq.conf` 
  3. Paste the `rsboot.efi` to `/srv/tftp` (or any other directory, but make sure to update `dnsmasq` config too)
  4. Start the `dnsmasq` service via your init system or run `dnsmasq -k -C /etc/dnsmasq.conf`
  5. PXE server setup is ready!
  
#### Mirror server (for serving the OS images)  
__you can do this on the same server, but i suggest doing it separately__ 
  1. Install `nginx`
  2. Add the OS images from [this list](../efi/assets/os_list) to `/usr/share/nginx/html`
  3. Ensure `nginx` serves on the same IP as the URLs in the list above (you can change them to match the chosen subnet for the DHCP server)
  4. Start `nginx` service via your init system
  5. Mirror server is ready!

--- 
  Now you can use **PXE/Network Boot** to boot up `rsboot`!

#### If using Docker:
  0. Install `Docker` and `Docker Compose`
  1. Add the `rsboot.efi` into [./efi/](./efi/)
  2. Add the OS images into [./os/](./os/)
  3. Change the subnets in the [Docker Compose](./docker-compose.yml) and in [dnsmasq config](./pxe/dnsmasq.conf) if you wish.
  4. `docker compose up -d --build`
  
##### See [QEMU directory](../qemu/) to test it out.
