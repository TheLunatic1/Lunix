import subprocess
import time
import os
import sys
import threading

def test_milestone3():
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

    print("[*] Launching Lunix in QEMU headless mode for Milestone 3 tests...")
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
        ("ls -l /dev\n", 1.2),
        ("ls -l /proc\n", 1.2),
        ("cat /proc/version\n", 1.2),
        ("cat /proc/meminfo\n", 1.2),
        ("cat /proc/cpuinfo\n", 1.2),
        ("cat /proc/uptime\n", 1.2),
        ("exec /bin/test_devproc.elf\n", 2.5),
        ("exec /bin/win_envreg.exe\n", 2.5),
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
        ("null", "/dev/null node in /dev"),
        ("zero", "/dev/zero node in /dev"),
        ("urandom", "/dev/urandom node in /dev"),
        ("version", "/proc/version in /proc"),
        ("meminfo", "/proc/meminfo in /proc"),
        ("cpuinfo", "/proc/cpuinfo in /proc"),
        ("uptime", "/proc/uptime in /proc"),
        ("Linux version 6.8.0-lunix-hybrid", "Kernel version from /proc/version"),
        ("MemTotal:", "Physical memory totals from /proc/meminfo"),
        ("MemFree:", "Usable free memory from /proc/meminfo"),
        ("processor", "Processor core enumeration in /proc/cpuinfo"),
        ("Lunix Virtual x86_64 Processor", "Processor model from /proc/cpuinfo"),
        ("[DEVPROC] Testing Virtual Pseudo-Filesystems", "Linux test_devproc.elf execution"),
        ("Successfully opened, wrote, read (EOF), and closed /dev/null", "Linux /dev/null read/write EOF verification"),
        ("Successfully read zero bytes from /dev/zero", "Linux /dev/zero stream reading"),
        ("Successfully read random entropy from /dev/urandom", "Linux /dev/urandom entropy generation"),
        ("Successfully read /proc/version kernel identification", "Linux /proc/version file descriptor read"),
        ("Successfully read /proc/meminfo physical memory metrics", "Linux /proc/meminfo file descriptor read"),
        ("Successfully read /proc/cpuinfo processor configuration", "Linux /proc/cpuinfo file descriptor read"),
        ("Successfully read /proc/uptime system timer metrics", "Linux /proc/uptime file descriptor read"),
        ("SUCCESS: All /dev and /proc virtual filesystems verified!", "Linux pseudo-filesystems test completion"),
        ("64-bit Windows PE32+ (/bin/win_envreg.exe)", "Win32 PE32+ env/reg test execution"),
        ("Successfully queried and updated environment block!", "Win32 GetEnvironmentVariableA & SetEnvironmentVariableA"),
        ("Successfully opened, queried HKLM registry and closed key!", "Win32 RegOpenKeyExA, RegQueryValueExA & RegCloseKey"),
        ("Win32 Environment Block & In-Memory Registry verified!", "Win32 environment & registry subsystem verification"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Milestone 3 verification checks failed!"

    print("\n=======================================================")
    print("  [SUCCESS] ALL MILESTONE 3 VERIFICATION TESTS PASSED! ")
    print("=======================================================")

if __name__ == "__main__":
    test_milestone3()
