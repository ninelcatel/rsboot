#/bin/sh
modprobe loop
losetup -f /rsboot.iso
udevadm trigger --action=add
udevadm settle
