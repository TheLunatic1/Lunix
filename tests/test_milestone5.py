import subprocess
import time
import os
import sys
import threading

def create_temp_disk(path, size_mb=8):
    if not os.path.exists(path):
        with open(path, "wb") as f:
            f.write(b"\x00" * (size_mb * 1024 * 1024))

def test_milestone5():
    root = r"d:\REPOSITORIES\Lunix"
    qemu = os.path.join(root, r"tools\qemu\qemu-system-x86_64.exe")
    ovmf = os.path.join(root, r"tools\qemu\share\edk2-x86_64-code.fd")
    img = os.path.join(root, r"target\lunix.img")

    sata_img = os.path.join(root, "target", "sata_test.img")
    nvme_img = os.path.join(root, "target", "nvme_test.img")
    vblk_img = os.path.join(root, "target", "virtio_test.img")

    create_temp_disk(sata_img, 8)
    create_temp_disk(nvme_img, 8)
    create_temp_disk(vblk_img, 8)

    qemu_cmd = [
        qemu,
        "-L", os.path.join(root, r"tools\qemu\share"),
        "-drive", f"if=pflash,format=raw,readonly=on,file={ovmf}",
        "-drive", f"format=raw,file={img},if=ide",
        "-device", "ahci,id=ahci0",
        "-drive", f"id=sata0,file={sata_img},if=none,format=raw",
        "-device", "ide-hd,drive=sata0,bus=ahci0.0",
        "-drive", f"file={nvme_img},if=none,id=nvm0,format=raw",
        "-device", "nvme,serial=LUNIXNVME01,drive=nvm0",
        "-drive", f"file={vblk_img},if=none,id=vblk0,format=raw",
        "-device", "virtio-blk-pci,drive=vblk0",
        "-netdev", "user,id=vnet0",
        "-device", "virtio-net-pci,netdev=vnet0",
        "-smp", "2",
        "-serial", "stdio",
        "-display", "none",
        "-m", "512M"
    ]

    print("[*] Launching Lunix with AHCI, NVMe, VirtIO, and Windows WDM in QEMU...")
    proc = subprocess.Popen(
        qemu_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        cwd=root
    )

    full_output = []

    def reader():
        while True:
            try:
                chunk = proc.stdout.read(1)
                if not chunk:
                    break
                decoded = chunk.decode('latin-1', errors='replace')
                full_output.append(decoded)
                sys.stdout.write(decoded)
                sys.stdout.flush()
            except Exception:
                break

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    def send_command(cmd_str):
        for ch in cmd_str:
            proc.stdin.write(ch.encode('utf-8'))
            proc.stdin.flush()
            time.sleep(0.008)

    print("[*] Waiting for Lunix OS to boot...")
    time.sleep(4.5)

    test_commands = [
        ("storage\n", 1.2),
        ("ahci\n", 1.2),
        ("nvme\n", 1.2),
        ("virtio\n", 1.2),
        ("nt list\n", 1.0),
        ("nt load /sys/drivers/sample_wdm.sys\n", 2.0),
        ("nt list\n", 1.2),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("         EVALUATING MILESTONE 5 TEST RESULTS           ")
    print("=======================================================")

    checks = [
        ("LUNIX STORAGE SUBSYSTEM STATUS", "Unified Storage Manager (storage command)"),
        ("/dev/sda", "ATA/IDE block device discovery"),
        ("/dev/ahci0", "AHCI SATA block device discovery"),
        ("/dev/vda", "VirtIO block device discovery"),
        ("AHCI SATA Host Controller Status", "AHCI SATA storage driver inspection"),
        ("NVM Express (NVMe) PCIe Storage Status", "NVMe PCIe SSD storage driver inspection"),
        ("VirtIO Paravirtualization Devices", "VirtIO Paravirtualized subsystem (blk & net)"),
        ("Active Windows NT WDM Drivers", "Windows NT driver manager listing"),
        ("[WDM] Loading 64-bit Windows Driver (.sys)", "Dynamic PE32+ WDM driver loader"),
        ("[WDM DRIVER] DriverEntry executed via dynamic PE32+ loader!", "DriverEntry execution via extern win64 ABI"),
        ("[WDM DRIVER] Successfully bound to ntoskrnl.exe & hal.dll DDIs", "Import resolution against ntoskrnl.exe & hal.dll"),
        ("[+] Windows Driver 'sample_wdm.sys' loaded successfully!", "WDM driver registration in DRIVER_TABLE"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Milestone 5 verification checks failed!"

    print("\n=======================================================")
    print("  [SUCCESS] ALL MILESTONE 5 VERIFICATION TESTS PASSED! ")
    print("=======================================================")

if __name__ == "__main__":
    test_milestone5()
