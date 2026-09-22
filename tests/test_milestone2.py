import subprocess
import time
import os
import sys
import threading

def test_milestone2():
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

    print("[*] Launching Lunix in QEMU headless mode...")
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
        ("uname -a\n", 1.5),
        ("ls -l /bin\n", 1.5),
        ("exec /bin/test_pipe.elf\n", 2.5),
        ("exec /bin/test_dir.elf\n", 2.5),
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
        ("6.8.0-arch1-1-lunix", "OS release identification"),
        ("test_pipe.elf", "test_pipe.elf in /bin"),
        ("test_dir.elf", "test_dir.elf in /bin"),
        ("Hello from cloned child process through IPC pipe!", "Linux pipe write in child"),
        ("Received message from child via IPC pipe", "Linux pipe read in parent"),
        ("Successfully verified IPC pipe!", "Linux sys_pipe2 & IPC verification"),
        ("Querying /bin directory entries via sys_getdents64", "Linux sys_openat & sys_getdents64 invocation"),
        ("Found directory entry from sys_getdents64 in /bin", "Linux sys_getdents64 directory traversal"),
        ("Directory traversal test completed successfully!", "Linux directory navigation completed"),
    ]

    all_passed = True
    for needle, desc in checks:
        if needle in output:
            print(f"  [PASS] {desc}")
        else:
            print(f"  [FAIL] {desc} (Expected: '{needle}')")
            all_passed = False

    assert all_passed, "One or more Milestone 2 verification checks failed!"

    print("\n=======================================================")
    print("  [SUCCESS] ALL MILESTONE 2 VERIFICATION TESTS PASSED! ")
    print("=======================================================")

if __name__ == "__main__":
    test_milestone2()
