import subprocess
import time
import os
import sys
import threading

def test_instant_paste():
    cmd = [
        r"tools\qemu\qemu-system-x86_64.exe",
        "-L", r"tools\qemu\share",
        "-drive", r"if=pflash,format=raw,readonly=on,file=tools\qemu\share\edk2-x86_64-code.fd",
        "-drive", r"format=raw,file=target\lunix.img,if=ide",
        "-smp", "2",
        "-serial", "stdio",
        "-display", "none",
        "-m", "512M"
    ]
    
    print("Starting QEMU process...")
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=False,
        bufsize=0
    )
    
    all_output = bytearray()
    
    def reader_thread():
        while proc.poll() is None:
            try:
                chunk = proc.stdout.read(1)
                if chunk:
                    all_output.extend(chunk)
                    sys.stdout.buffer.write(chunk)
                    sys.stdout.buffer.flush()
            except Exception:
                break

    t = threading.Thread(target=reader_thread, daemon=True)
    t.start()
    
    # 1. Wait for initial boot prompt
    start_time = time.time()
    booted = False
    while time.time() - start_time < 12:
        if b"lunix> " in all_output:
            booted = True
            break
        time.sleep(0.05)
        
    if not booted:
        print("\n[-] ERROR: Did not reach shell prompt in time!")
        proc.kill()
        sys.exit(1)
        
    print("\n\n=== BOOTED SUCCESSFULLY! STARTING INSTANT ZERO-DELAY PASTE TESTS ===\n")
    time.sleep(0.5)
    
    test_commands = [
        ("uname -a\n", b"Lunix lunix-os 0.1.0-hybrid"),
        ("ls -l /bin\n", b"hello.elf"),
        ("exec /bin/hello.elf\n", b"Hello from standalone ELF64 binary (/bin/hello.elf)"),
        ("exec /bin/sysinfo.elf\n", b"Successfully woke up in Ring 3"),
        ("exec /bin/win_hello.exe\n", b"Hello from 64-bit Windows PE32+ (/bin/win_hello.exe)"),
        ("cat /etc/os-release\n", b"NAME=\"Lunix OS\""),
        ("cat /home/lunix/readme.txt\n", b"Welcome to Lunix OS!"),
        ("uptime\n", b"up"),
        ("ifconfig\n", b"eth0:"),
        ("echo 1234567890_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ\n", b"1234567890_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ")
    ]
    
    passed = 0
    total = len(test_commands)
    
    for idx, (command, expected_substr) in enumerate(test_commands, 1):
        print(f"\n[{idx}/{total}] >>> [INSTANT ZERO-DELAY PASTE ({len(command)} bytes)]: {command.strip()}")
        time.sleep(0.3)
        start_idx = len(all_output)
        
        # Write entire string at ONCE in a single bulk call
        proc.stdin.write(command.encode('utf-8'))
        proc.stdin.flush()
        
        cmd_start = time.time()
        success = False
        while time.time() - cmd_start < 5.0:
            current_new_text = all_output[start_idx:]
            if expected_substr in current_new_text and (b"lunix> " in current_new_text or b"lunix:" in current_new_text):
                success = True
                break
            time.sleep(0.05)
            
        if success:
            print(f"[+] PASS [{idx}/{total}]")
            passed += 1
        else:
            print(f"[-] FAIL [{idx}/{total}]: Did not find '{expected_substr.decode()}'")
            print(f"Captured output: {all_output[start_idx:].decode(errors='replace')}")
            
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except Exception:
        proc.kill()
        
    print(f"\n=== TEST SUMMARY: {passed}/{total} PASSED ===")
    if passed == total:
        print("[SUCCESS] Instant paste test passed with 100% character fidelity!")
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    test_instant_paste()
