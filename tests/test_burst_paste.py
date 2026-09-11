import subprocess
import time
import os
import sys

def test_burst_paste():
    qemu_cmd = [
        r"tools\qemu\qemu-system-x86_64.exe",
        "-L", r"tools\qemu\share",
        "-drive", r"if=pflash,format=raw,readonly=on,file=tools\qemu\share\edk2-x86_64-code.fd",
        "-drive", r"format=raw,file=target\lunix.img,if=ide",
        "-smp", "2",
        "-serial", "pipe:lunix_pipe",
        "-display", "none",
        "-m", "512M"
    ]
    
    # Or using standard stdio redirection with sub-second writes
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
    
    # Read until prompt is reached
    output = bytearray()
    start_time = time.time()
    booted = False
    
    import threading
    
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
    
    # Wait for prompt "lunix> "
    while time.time() - start_time < 10:
        if b"lunix> " in all_output:
            booted = True
            break
        time.sleep(0.05)
        
    if not booted:
        print("ERROR: Did not reach shell prompt in time!")
        proc.kill()
        sys.exit(1)
        
    print("\n\n=== BOOTED SUCCESSFULLY! STARTING BURST PASTE TESTS ===")
    
    test_commands = [
        "uname -a\n",
        "ls -l /bin\n",
        "cat /etc/os-release\n",
        "cat /home/lunix/readme.txt\n",
        "uptime\n",
        "free -m\n",
        "df -h\n",
        "ps -aux\n",
        "ifconfig\n",
        "pwd\n",
        "echo Hello from Lunix Shell!\n",
        "stat /bin/hello.elf\n",
        "exec /bin/hello.elf\n",
        "exec /bin/sysinfo.elf\n",
        "exec /bin/win_hello.exe\n"
    ]
    
    for command in test_commands:
        print(f"\n>>> [BURST PASTE] {command.strip()}...")
        prev_len = len(all_output)
        proc.stdin.write(command.encode('utf-8'))
        proc.stdin.flush()
        
        # Wait for prompt to reappear after command output
        cmd_start = time.time()
        while time.time() - cmd_start < 2.0:
            if b"lunix> " in all_output[prev_len:] or b"lunix:" in all_output[prev_len:]:
                break
            time.sleep(0.05)
        time.sleep(0.2)
        
    time.sleep(1.0)
    proc.kill()
    print("\n\n=== BURST PASTE TEST COMPLETED ===")

if __name__ == "__main__":
    test_burst_paste()
