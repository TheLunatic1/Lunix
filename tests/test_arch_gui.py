import subprocess
import time
import os
import sys
import threading

def test_arch_gui():
    root = r"d:\REPOSITORIES\Lunix"
    qemu = os.path.join(root, r"tools\qemu\qemu-system-x86_64.exe")
    ovmf = os.path.join(root, r"tools\qemu\share\edk2-x86_64-code.fd")
    img = os.path.join(root, r"target\lunix.img")

    print("[*] Verifying Arch Linux VM disk image on host disk...")
    assert os.path.exists(img), f"Raw disk image missing: {img}"

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

    print("\n[*] Launching Lunix Kernel with Arch Linux Desktop in QEMU...")
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
        ("help\n", 1.0),
        ("pacman -Q\n", 1.0),
        ("startx\n", 3.0),
        ("\x1b", 1.5),  # Send Escape key to toggle back to CLI
        ("ls -la /var/lib/pacman/local\n", 1.0),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("      EVALUATING ARCH LINUX GUI TEST RESULTS           ")
    print("=======================================================")

    checks = [
        ("6.8.0-arch1-1-lunix", "Arch Linux Kernel Release in uname -a"),
        ("startx / gui", "Help list shows startx and gui commands"),
        ("pacman 6.1.0-3", "Pacman installed package listing (pacman -Q)"),
        ("ARCH LINUX GRAPHICAL DESKTOP ENVIRONMENT", "Desktop initialization banner upon running startx"),
        ("Starting Window Manager (Aero Taskbar + Start Menu)", "Aero Taskbar & Start Menu initialization"),
        ("Arch Linux Desktop launched (60 FPS)", "60 FPS Desktop Compositor activation confirmation"),
        ("[root@arch", "Prompt available before and after GUI session"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Arch Linux GUI verification checks failed!"

    print("\n=======================================================")
    print(" [SUCCESS] ALL ARCH LINUX GUI TESTS PASSED 100%!       ")
    print("=======================================================")

if __name__ == "__main__":
    test_arch_gui()
