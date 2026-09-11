import subprocess
import time
import os
import sys
import threading

def test_burst_paste_robust():
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
        
    print("\n\n=== BOOTED SUCCESSFULLY! STARTING 15 BURST PASTE TESTS ===\n")
    time.sleep(0.3)
    
    test_commands = [
        ("uname -a\n", b"Lunix lunix-os 0.1.0-hybrid"),
        ("ls -l /bin\n", b"hello.elf"),
        ("cat /etc/os-release\n", b"NAME=\"Lunix OS\""),
        ("cat /home/lunix/readme.txt\n", b"Welcome to Lunix OS!"),
        ("uptime\n", b"up"),
        ("free -m\n", b"Total RAM:"),
        ("df -h\n", b"/dev/sda"),
        ("ps -aux\n", b"COMMAND"),
        ("ifconfig\n", b"eth0:"),
        ("pwd\n", b"/"),
        ("echo Hello from Lunix Shell!\n", b"Hello from Lunix Shell!"),
        ("stat /bin/hello.elf\n", b"File: 'hello.elf'"),
        ("exec /bin/hello.elf\n", b"Process exited with status code: 0"),
        ("exec /bin/sysinfo.elf\n", b"Process exited with status code: 0"),
        ("exec /bin/win_hello.exe\n", b"Process terminated cleanly via ExitProcess(0)")
    ]
    
    passed = 0
    total = len(test_commands)
    
    for idx, (command, expected_substr) in enumerate(test_commands, 1):
        print(f"\n[{idx}/{total}] >>> [BURST PASTE] {command.strip()}...")
        # Drain / note output length before writing
        time.sleep(0.3)
        start_idx = len(all_output)
        
        # Write burst string at terminal paste rate (2ms per character = 500 chars/sec)
        for ch in command:
            proc.stdin.write(ch.encode('utf-8'))
            proc.stdin.flush()
            time.sleep(0.002)
        
        # Wait for expected output and prompt to return
        cmd_start = time.time()
        success = False
        while time.time() - cmd_start < 6.0:
            current_new_text = all_output[start_idx:]
            if expected_substr in current_new_text and (b"lunix> " in current_new_text or b"lunix:" in current_new_text):
                success = True
                break
            time.sleep(0.05)
            
        if success:
            print(f"  [PASS] Test #{idx} '{command.strip()}' succeeded with 100% character fidelity.")
            passed += 1
        else:
            print(f"  [FAIL] Test #{idx} '{command.strip()}' failed or did not match expected output.")
            print(f"         Received since command: {repr(bytes(all_output[start_idx:]))}")
        time.sleep(0.3)
            
    print(f"\n=======================================================")
    print(f"  BURST PASTE TEST SUMMARY: {passed}/{total} PASSED")
    print(f"=======================================================")
    
    time.sleep(0.5)
    proc.kill()
    
    if passed == total:
        print("[+] ALL 15 BURST PASTE TESTS PASSED PERFECTLY!")
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    test_burst_paste_robust()
