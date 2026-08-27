import sys
import os
import re
import json
import hashlib
import requests
from urllib.parse import urljoin

"""
 distros.json is maintained by a separate script (at least thats what i think i will do)
 {
   "mirror": "http://192.168.100.3",
   "distros": [
     {
       "family": "Debian", "edition": "-", "image": "netinstall", "boot": "ramdisk",
       "latest": {
         "checksum-url": "https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/SHA256SUMS",
         "file": "debian_latest.iso",            # served name under mirror
         "path": "/srv/http/debian_latest.iso"   # where to write it locally
       },
       "history": [ {"version": "12", "file": "debian_12.iso", "sha256": "..."} ]
     }
   ]
 }


OS catalog
family | edition | version | image type | boot | sha256 | url

this scrit will be run as a cron job to verify the latest distro family (via sha256
and 512 checksums, some distros dont use 256) and if needed, update it, also creates the OS_LIST 
for the bootloader based on the distros.json
"""

SHA256 = re.compile(r"\b[0-9a-fA-F]{64}\b")
SHA512 = re.compile(r"\b[0-9a-fA-F]{128}\b")

def fetch(url):
    r = requests.get(url, timeout=60)
    r.raise_for_status()
    return r.text

def upstreamed_hashes(text):
    return (
        {h.lower() for h in SHA256.findall(text)},
        {h.lower() for h in SHA512.findall(text)},
    )

def find_iso_url(text, checksum_url):
    names = []
    
    for line in text.splitlines():
        for tok in line.split():
            name = tok.strip("*()")
            if name.lower().endswith(".iso"):
                names.append(name)
    
    if not names:
        return None
    
    return urljoin(checksum_url, min(names, key=len))


def hash_file(path):
    h256 = hashlib.sha256()
    h512 = hashlib.sha512()

    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(2**20), b""):
            h256.update(chunk)
            h512.update(chunk)
    
    return h256.hexdigest(), h512.hexdigest()


def download(url, path, pub256, pub512):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    temp = path + ".temp"
    
    h256 = hashlib.sha256()
    h512 = hashlib.sha512()

    r = requests.get(url, stream=True, timeout=60)
    r.raise_for_status()
    
    with open(temp, "wb") as f:
        for chunk in r.iter_content(chunk_size=2**20):
            if chunk:
                h256.update(chunk)
                h512.update(chunk)
                f.write(chunk)
    
    s256 = h256.hexdigest()
    s512 = h512.hexdigest()
    
    if pub256:
        ok = s256 in pub256
    elif pub512:
        ok = s512 in pub512
    else:
        ok = False
    if not ok:
        os.remove(temp)
        raise ValueError(f"integrity check failed for {path}")
    
    os.replace(temp, path)
    return s256, s512


def is_current(entry, pub256, pub512):
    # download only if file is missing or checksums dont match
    if not entry:
        return False
    if pub256 and entry.get("sha256") in pub256:
        return True
    if pub512 and entry.get("sha512") in pub512:
        return True
    return False


def write_os_list(rows, path):
    with open(path, "w") as f:
        last = None
        for r in rows:
            if last is not None and r[0] != last:
                f.write("\n")
            last = r[0]
            f.write(" | ".join(r) + "\n")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: ./update_distros.py /path/to/distros.json /other/path/to/os_list")

    with open(sys.argv[1]) as f:
        catalog = json.load(f)

    mirror = catalog.get("mirror", "")

    # json for keeping track of hashes for latest edition distros 
    hashes_path = os.path.join(os.path.dirname(os.path.abspath(sys.argv[1])), "hashes.json")
    if os.path.exists(hashes_path):
        with open(hashes_path) as f:
            hashes = json.load(f)
    else:
        hashes = {}

    rows = []

    for d in catalog["distros"]:
        family = d["family"]
        edition = d.get("edition", "-")
        image = d["image"]
        boot = d["boot"]

        latest = d.get("latest")
        if latest:
            try:
                # a family can have several editions
                key = latest["file"]
                text = fetch(latest["checksum-url"])
                pub256, pub512 = upstreamed_hashes(text)
                entry = hashes.get(key)

                if not entry and os.path.exists(latest["path"]):
                    # calculate the hash of the local file and save it to check if an update is necessary
                    s256, s512 = hash_file(latest["path"])
                    entry = {"sha256": s256, "sha512": s512}

                if not is_current(entry, pub256, pub512):
                    url = find_iso_url(text, latest["checksum-url"])
                    s256, s512 = download(url, latest["path"], pub256, pub512)
                    entry = {"sha256": s256, "sha512": s512}

                hashes[key] = entry
                rows.append([family, edition, "latest", image, boot, entry["sha256"], f"{mirror}/{latest['file']}"])
            except Exception as e:
                print(f"[fail] {family}: {e}")

        for h in d.get("history", []):
            rows.append([family, edition, h["version"], image, boot, h.get("sha256", "-"), f"{mirror}/{h['file']}"])
    
    with open(hashes_path,"w") as f:
        json.dump(hashes, f)
    
    write_os_list(rows, sys.argv[2])
