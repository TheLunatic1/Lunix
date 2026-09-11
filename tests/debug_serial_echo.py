import subprocess
import time
import sys
import threading

def debug():
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
                    sys.stdout.buffer.write(chunk)
                    sys.stdout.buffer.flush()
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
        
    print("\n--- SENDING 'uname -a\\n' ---")
    time.sleep(0.5)
    start_idx = len(all_output)
    proc.stdin.write(b"uname -a\n")
    proc.stdin.flush()
    
    time.sleep(2.0)
    print("\n--- CAPTURED AFTER 2s ---")
    print(repr(bytes(all_output[start_idx:])))
    
    proc.kill()

if __name__ == "__main__":
    debug()
