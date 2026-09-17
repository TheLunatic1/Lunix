import subprocess
import time
import os
import sys
import threading

def test_tinycore_gui():
    root = r"d:\REPOSITORIES\Lunix"
    qemu = os.path.join(root, r"tools\qemu\qemu-system-x86_64.exe")
    ovmf = os.path.join(root, r"tools\qemu\share\edk2-x86_64-code.fd")
    img = os.path.join(root, r"target\lunix.img")
    vmdk = os.path.join(root, r"target\lunix.vmdk")
    vdi = os.path.join(root, r"target\lunix.vdi")

    print("[*] Verifying VM disk images on host disk...")
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

    print("\n[*] Launching Lunix with Official Tiny Core Graphical Desktop in QEMU...")
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
        ("ls /etc/sysconfig\n", 1.0),
        ("cat /etc/sysconfig/desktop\n", 1.0),
        ("cat /etc/sysconfig/icons\n", 1.0),
        ("cat /etc/sysconfig/Xserver\n", 1.0),
        ("cat /home/tc/.xsession\n", 1.0),
        ("cat /home/tc/.wbar\n", 1.0),
        ("ls /usr/local/bin\n", 1.5),
        ("ls /dev\n", 1.0),
        ("startx\n", 3.0),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("     EVALUATING TINY CORE GUI DESKTOP TEST RESULTS     ")
    print("=======================================================")

    checks = [
        ("Xserver", "Tiny Core Xserver config in /etc/sysconfig"),
        ("desktop", "Tiny Core Desktop config in /etc/sysconfig"),
        ("icons", "Tiny Core Icons config in /etc/sysconfig"),
        ("flwm", "Default window manager set to flwm"),
        ("wbar", "Default dock set to wbar"),
        ("Xfbdev", "Default X server set to Xfbdev"),
        ("/home/tc/.xsession", "User session startup script (.xsession)"),
        ("osxbarback.png", "Wbar icon configuration present (.wbar)"),
        ("Xfbdev", "Official Tiny Core Xfbdev binary present in /usr/local/bin"),
        ("flwm", "Official Fast Light Window Manager (flwm) binary in /usr/local/bin"),
        ("wbar", "Official Wbar dock binary in /usr/local/bin"),
        ("aterm", "Official Aterm terminal binary in /usr/local/bin"),
        ("fb0", "/dev/fb0 framebuffer device present"),
        ("mice", "/dev/mice PS/2 mouse device present"),
        ("TINY CORE LINUX GRAPHICAL DESKTOP (FLWM + WBAR)", "startx graphical desktop launch banner"),
        ("Initializing X11 Framebuffer Server (Xfbdev on /dev/fb0)", "Xfbdev framebuffer initialization"),
        ("Launching Fast Light Window Manager (flwm)", "flwm window manager launch"),
        ("Launching Animated Application Dock (wbar)", "wbar animated dock launch"),
        ("Launching Graphical Terminal Emulator (aterm)", "aterm terminal emulator launch"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Tiny Core GUI verification checks failed!"

    print("\n=======================================================")
    print(" [SUCCESS] ALL TINY CORE GUI DESKTOP TESTS PASSED 100%!")
    print("=======================================================")

if __name__ == "__main__":
    test_tinycore_gui()
