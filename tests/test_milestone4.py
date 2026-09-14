import subprocess
import time
import os
import sys
import threading

def test_milestone4():
    qemu = r"tools\qemu\qemu-system-x86_64.exe"
    ovmf = r"tools\qemu\share\edk2-x86_64-code.fd"
    img = r"target\lunix.img"

    qemu_cmd = [
        qemu,
        "-L", r"tools\qemu\share",
        "-drive", f"if=pflash,format=raw,readonly=on,file={ovmf}",
        "-drive", f"format=raw,file={img},if=ide",
        "-netdev", "user,id=net0",
        "-device", "e1000,netdev=net0",
        "-smp", "2",
        "-serial", "stdio",
        "-display", "none",
        "-m", "512M"
    ]

    print("[*] Launching Lunix in QEMU headless mode for Milestone 4 tests...")
    proc = subprocess.Popen(
        qemu_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        cwd=r"d:\REPOSITORIES\Lunix"
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

    print("[*] Waiting for Lunix OS to boot and display prompt...")
    time.sleep(4.0)

    test_commands = [
        ("uname -a\n", 1.2),
        ("ls -l /bin\n", 1.2),
        ("exec /bin/busybox\n", 2.0),
        ("exec /bin/test_busybox.elf\n", 5.0),
    ]

    for cmd_str, wait_time in test_commands:
        print(f"\n[*] Sending command: {cmd_str.strip()}", flush=True)
        send_command(cmd_str)
        time.sleep(wait_time)

    proc.kill()
    t.join(timeout=1.0)

    output = "".join(full_output)

    print("\n\n=======================================================")
    print("                EVALUATING TEST RESULTS                ")
    print("=======================================================")

    checks = [
        ("0.1.0-hybrid", "OS release identification"),
        ("busybox", "busybox multi-call binary in /bin"),
        ("sh", "sh shell launcher in /bin"),
        ("test_busybox.elf", "test_busybox.elf verification binary in /bin"),
        ("BusyBox v1.36.1", "BusyBox multi-call banner output"),
        ("cat, clear, date, df, dmesg, echo", "BusyBox applet index list"),
        ("[BUSYBOX TEST] Real Linux Userspace Bootstrapping", "Linux test_busybox.elf execution"),
        ("Successfully initialized FS_BASE MSR & Thread-Local Storage", "Thread-Local Storage (arch_prctl / FS_BASE)"),
        ("Verified process credentials (uid=0, gid=0) and process groups", "Process credentials (getuid/getgid) & groups"),
        ("Configured POSIX signal handlers & real-time signal mask", "POSIX signal handling (rt_sigprocmask/rt_sigaction)"),
        ("Verified ELF auxiliary vectors and /proc/self/exe resolution", "ELF auxiliary vectors & readlink /proc/self/exe"),
        ("Verified access(), clock_gettime(), and statfs() subsystem", "File access(), POSIX time & statfs() metrics"),
        ("Executing BusyBox command: for i in 1 2 3; do echo item $i; done", "Shell script iteration execution"),
        ("item 1", "Shell loop item 1 output"),
        ("item 2", "Shell loop item 2 output"),
        ("item 3", "Shell loop item 3 output"),
        ("SUCCESS: Real Linux Userspace & BusyBox subsystem verified!", "Milestone 4 comprehensive verification"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Milestone 4 verification checks failed!"

    print("\n=======================================================")
    print("  [SUCCESS] ALL MILESTONE 4 VERIFICATION TESTS PASSED! ")
    print("=======================================================")

if __name__ == "__main__":
    test_milestone4()
