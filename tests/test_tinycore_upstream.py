import subprocess
import time
import os
import sys
import threading

def test_tinycore_upstream():
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

    print("\n[*] Launching Lunix with Official Upstream Tiny Core Rootfs in QEMU...")
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
        ("uname -a\n", 1.0),
        ("ls /bin\n", 1.5),
        ("ls /lib\n", 1.0),
        ("ls /etc\n", 1.0),
        ("cat /etc/os-release\n", 1.0),
        ("cat /etc/inittab\n", 1.0),
        ("cat /etc/passwd\n", 1.0),
        ("cat /etc/issue\n", 1.0),
        ("exec /bin/test_tinycore.elf\n", 2.0),
        ("exec /bin/test_dynamic.elf\n", 2.0),
        ("exec /bin/test_devproc.elf\n", 2.0),
        ("tinycore\n", 2.0),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("      EVALUATING UPSTREAM TINY CORE TEST RESULTS       ")
    print("=======================================================")

    checks = [
        ("6.8.0-tinycore", "Tiny Core Linux Kernel Release String in uname"),
        ("Tiny Core Linux", "Tiny Core Linux /etc/os-release metadata"),
        ("::sysinit:/etc/init.d/rcS", "SysV init table (/etc/inittab)"),
        ("tc:x:1001:1001:tc:/home/tc:/bin/sh", "Tiny Core default user account (/etc/passwd)"),
        ("Tiny Core Linux v15.0", "Tiny Core login banner (/etc/issue)"),
        ("busybox", "Official BusyBox binary present in /bin"),
        ("ld-linux-x86-64.so.2", "Glibc dynamic linker present in /lib"),
        ("[TINYCORE] Initializing Tiny Core Linux v15.0", "Tiny Core Linux runtime execution"),
        ("[SYSINFO] Total RAM: 512 MB, Free RAM:", "sys_sysinfo runtime syscall (Syscall 99)"),
        ("[TID] Initialized Thread Address Space", "sys_set_tid_address runtime syscall (Syscall 218)"),
        ("[FUTEX] Fast user-space synchronization primitives verified", "sys_futex mutex synchronization (Syscall 202)"),
        ("[MMAP] Dynamic memory & shared library mapping verified", "File-backed shared library mapping (Syscall 9)"),
        ("[TC-INIT] Running /etc/init.d/rcS", "Tiny Core /sbin/init bootstrap flow"),
        ("[LD-LINUX] Runtime dynamic linker loaded", "Dynamic ELF Interpreter mapping (PT_INTERP)"),
        ("[LD-LINUX] Processed auxiliary vectors: AT_BASE, AT_ENTRY, AT_PHDR", "Dynamic loader auxiliary vector generation"),
        ("[APP] Dynamic ELF test binary reached main() after ld-linux init!", "Dynamic application execution via ld.so"),
        ("[APP] PT_INTERP dynamic linking verified successfully.", "PT_INTERP dynamic linking end-to-end"),
        ("[DEVPROC] SUCCESS: All /dev and /proc virtual filesystems verified!", "Virtual filesystem and /dev nodes verified"),
        ("TINY CORE LINUX USERSPACE (PID 1 BOOT SEQUENCE)", "Built-in tinycore shell launcher command"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Upstream Tiny Core verification checks failed!"

    print("\n=======================================================")
    print(" [SUCCESS] ALL UPSTREAM TINY CORE TESTS PASSED 100%!  ")
    print("=======================================================")

if __name__ == "__main__":
    test_tinycore_upstream()
