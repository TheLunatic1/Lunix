import subprocess
import time
import sys
import threading

def test_delays():
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
            except Exception:
                break

    t = threading.Thread(target=reader_thread, daemon=True)
    t.start()
    
    # Wait for prompt
    start = time.time()
    while time.time() - start < 10:
        if b"lunix> " in all_output:
            break
        time.sleep(0.05)
        
    print("[+] Prompt ready! Testing delays...")
    
    delays = [0.0, 0.001, 0.005, 0.01, 0.02, 0.05]
    for d in delays:
        time.sleep(0.5)
        start_idx = len(all_output)
        test_str = f"echo test_{int(d*1000)}ms\n"
        print(f"\n--- Testing delay {d*1000:.1f}ms per char: {test_str.strip()} ---")
        if d == 0.0:
            proc.stdin.write(test_str.encode())
            proc.stdin.flush()
        else:
            for ch in test_str:
                proc.stdin.write(ch.encode())
                proc.stdin.flush()
                time.sleep(d)
                
        time.sleep(1.0)
        captured = all_output[start_idx:].decode(errors='replace')
        print(f"Captured ({len(captured)} chars):\n{repr(captured)}")
        
    proc.kill()

if __name__ == "__main__":
    test_delays()
