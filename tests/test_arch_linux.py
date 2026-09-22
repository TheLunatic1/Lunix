import subprocess
import time
import os
import sys
import threading

def test_arch_linux():
    root = r"d:\REPOSITORIES\Lunix"
    qemu = os.path.join(root, r"tools\qemu\qemu-system-x86_64.exe")
    ovmf = os.path.join(root, r"tools\qemu\share\edk2-x86_64-code.fd")
    img = os.path.join(root, r"target\lunix.img")
    vmdk = os.path.join(root, r"target\lunix.vmdk")
    vdi = os.path.join(root, r"target\lunix.vdi")

    print("[*] Verifying Arch Linux VM disk images on host disk...")
    assert os.path.exists(img), f"Raw disk image missing: {img}"
    assert os.path.exists(vmdk), f"VMware Workstation disk image missing: {vmdk}"
    assert os.path.exists(vdi), f"VirtualBox disk image missing: {vdi}"
    print(f"  [+] Raw Image:  {img} ({os.path.getsize(img)} bytes)")
    print(f"  [+] VMware VMDK: {vmdk} ({os.path.getsize(vmdk)} bytes)")
    print(f"  [+] VirtualBox VDI: {vdi} ({os.path.getsize(vdi)} bytes)")

    qemu_cmd = [
        qemu,
        "-L", os.path.join(root, r"tools\qemu\share"),
        "-drive", f"if=pflash,format=raw,readonly=on,file={ovmf}",
        "-drive", f"format=raw,file={img},if=ide",
        "-smp", "2",
        "-serial", "stdio",
        "-display", "none",
        "-m", "512M"
    ]

    print("\n[*] Launching Lunix Kernel with Arch Linux Distribution in QEMU...")
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
    time.sleep(6.5)

    test_commands = [
        ("uname -a\n", 1.0),
        ("cat /etc/os-release\n", 1.0),
        ("cat /etc/arch-release\n", 1.0),
        ("cat /etc/pacman.conf\n", 1.0),
        ("cat /etc/pacman.d/mirrorlist\n", 1.0),
        ("cat /root/readme.txt\n", 1.0),
        ("pacman -V\n", 1.0),
        ("pacman -Q\n", 1.0),
        ("pacman -Sy\n", 1.0),
        ("pacman -S vim\n", 1.0),
        ("exec /bin/test_pacman.elf\n", 1.5),
        ("exec /bin/test_dir.elf\n", 1.5),
        ("exec /bin/test_pipe.elf\n", 1.5),
        ("exec /bin/test_fork.elf\n", 1.5),
        ("neofetch\n", 1.5),
        ("ls -l /var/lib/pacman/local\n", 1.5),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("      EVALUATING ARCH LINUX TEST RESULTS               ")
    print("=======================================================")

    checks = [
        ("6.8.0-arch1-1-lunix", "Arch Linux Kernel Release in uname -a"),
        ("NAME=\"Arch Linux\"", "Arch Linux /etc/os-release metadata"),
        ("ID=arch", "Arch Linux distro ID"),
        ("Arch Linux release", "Arch Linux /etc/arch-release file"),
        ("[options]", "Pacman configuration file (/etc/pacman.conf)"),
        ("https://geo.mirror.pkgbuild.com", "Pacman mirrorlist (/etc/pacman.d/mirrorlist)"),
        ("Pacman v6.1.0 - libalpm v14.0.0", "Pacman version banner (pacman -V)"),
        ("Arch Linux Package Management", "Pacman description banner"),
        ("linux-lunix 6.8.0-1", "Pacman installed package listing (pacman -Q)"),
        ("glibc 2.39-1", "Glibc package record in pacman"),
        (":: Synchronizing package databases...", "Pacman database sync (pacman -Sy)"),
        (":: Proceed with installation? [Y/n]", "Pacman package installation flow (pacman -S)"),
        ("OS: Arch Linux x86_64", "Neofetch Arch Linux system specification"),
        ("Kernel: 6.8.0-arch-lunix (100% Pure Rust)", "Neofetch bare-metal kernel specification"),
        ("[LINUX DIR TEST]", "Linux sys_getdents64 directory test"),
        ("[CHILD] Hello from cloned child process through IPC pipe!", "Linux pipe & fork IPC test"),
        ("[PARENT] Successfully reaped child process via sys_wait4!", "Linux process lifecycle & wait4 test"),
        ("[root@arch", "Interactive Arch Linux root shell prompt"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Arch Linux verification checks failed!"

    print("\n=======================================================")
    print(" [SUCCESS] ALL ARCH LINUX TESTS PASSED 100%!           ")
    print("=======================================================")

if __name__ == "__main__":
    test_arch_linux()
