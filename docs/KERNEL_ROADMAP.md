# Lunix Kernel Roadmap — Path to a Generally-Usable Linux-Compatible Kernel

> Companion to [`AGENTS.md`](../AGENTS.md) (architecture map, phase history) and
> [`FULL_PROJECT_HANDOVER_REPORT.md`](FULL_PROJECT_HANDOVER_REPORT.md) (detailed handover).
> This file is the forward-looking plan: what's left, in what order, and why.
> Last updated: 2026-09-22.

**Ground rule (unchanged, non-negotiable — see AGENTS.md §7):** the kernel implements Linux
kernel facilities only, in Rust. Userspace is always real, unmodified upstream binaries
(Arch today; the goal below is *any* real distro). Never draw UI, never hand-write stand-in
binaries, never fake a subsystem's data — if something upstream needs is missing, implement it
in the kernel.

## Where we are today

Real Arch Linux `bash` boots as PID 1 on bare Rust `#![no_std]` code; `startx` launches real
`Xorg` + `JWM` + `xterm` from official Arch packages (see AGENTS.md Phase 24–25). That's a
working proof that the architecture is sound. What's missing is *breadth* (most syscalls exist
for one specific Arch userland's happy path, not for arbitrary distros/programs) and *polish*
(boot time, reliability under load, no signals).

---

## Phase 1 — Boot Time (near-term, do this first)

**Status (2026-09-22): partially done.** `/dev/vda` (VirtIO-Block, `kernel/src/drivers/virtio/blk.rs`)
is wired up, attached by `xtask` alongside the existing IDE drive, and gets Arch `bash` to a
prompt in ~4-6s vs. ~7.5s for ATA/IDE PIO alone with the same image — but it is **not** the
default root device yet (see below). Along the way, three real, independent bugs in that driver
were found and fixed (all worth keeping regardless of the open item):
- The ring layout used a hardcoded 64-entry queue size instead of the device's actual
  `QueueNum`, desyncing the avail/used ring offsets from what the device computed and silently
  corrupting every transfer (root mount via vda failed outright until fixed).
- The DMA data buffer was always exactly one 4 KiB frame regardless of transfer size — any
  request over 8 sectors told the device to DMA past that frame into unrelated physical memory.
  Fixed by sizing the buffer to the transfer and chunking `read_blocks`/`write_blocks` to 256
  sectors per request (`MAX_CHUNK_SECTORS`), mirroring the ATA driver's own run size.
- Every read/write permanently leaked its two request/data physical frames (no matching
  `free_frame`) — harmless for a few boot-time reads, but exhausts memory under a sustained
  heavy read workload. Fixed with an RAII `DmaFrames` guard.
- Also fixed: the driver never read the legacy virtio `ISR` status register, which is required
  to acknowledge/deassert the device's `INTx#` line even in a polling-only driver.
- Also fixed while investigating: `/dev/sda`'s devfs node (`fs/devfs.rs`) was a stub that
  silently discarded writes and returned zeroes on read instead of doing real block I/O — now
  routes through the real `BlockDevice`, with `/dev/vda` too.
- Also fixed: OVMF's own default-boot-file discovery intermittently drops to the interactive
  UEFI Shell instead of chainloading `\EFI\BOOT\BOOTX64.EFI`, independent of VirtIO (reproduced
  with ATA-only too) — `xtask` now writes a `startup.nsh` to the ESP so the Shell's own
  auto-run path launches the bootloader deterministically. Verified over several consecutive
  clean boots after the fix.

**Open item — why vda isn't the default root device yet:** with `/dev/vda` as root, the
`startx` → Xorg → JWM → xterm desktop reliably stalls partway through rendering (JWM's taskbar
and an xterm titlebar fragment appear, then nothing further, indefinitely) under real
multi-process concurrent read load, even after all four fixes above. The same image with the
same VirtIO device attached but *not* used for root (idle, or undetected via
`disable-legacy=on`) renders the full desktop correctly, matching pre-VirtIO behavior — so the
remaining cause is specifically triggered by sustained concurrent vda I/O, not merely the
device's presence. Suspect areas for the next attempt: the single per-device `Mutex<VirtioBlkState>`
serializing all requests behind a full busy-spin per request (fine for one reader, possibly
starves others under concurrency — a real interrupt-driven completion path, or per-CPU/queue
state, may be needed instead of pure polling), or a scheduler interaction where a thread holding
that lock gets preempted mid-request. `fs/mod.rs` currently prefers `sda` first, falls back to
`vda`; flip that back to `vda`-first (one-line change, already written and tested) once this is
root-caused, and re-run the `startx` soak test (boot, `startx`, screenshot at 30/60/90/120s)
before trusting it.

Other causes found by inspection, not yet addressed:

2. **ext2 reads through the block layer** — confirm `fs/ext2.rs` reads are going through a
   block cache, not re-reading cold sectors on every inode/dirent lookup during the large
   package-file walk at mount/first-touch. A simple LRU block cache (a few thousand 4 KiB
   blocks) in `fs/block.rs` would cut most of this without touching the disk format.
3. **SMP AP bring-up** (`arch/x86_64/smp/mod.rs:176-213`) has several fixed spin-loops
   (5000/2000/10000 iterations) during INIT-SIPI-SIPI; fine in absolute terms (µs-ms) but worth
   calibrating against the APIC timer instead of a raw iteration count once everything else is
   fast, so it doesn't silently break on faster/slower hosts.
4. **PCI/ACPI enumeration** — check for any per-device fixed delays; these are typically cheap
   but should be profiled once (2) is done, since it'll dominate what's left.
5. Add a simple boot-time profiler: timestamp (via the existing 1000 Hz tick or `rdtsc`) each
   `[+]` milestone already printed in `kmain`, so future regressions are visible in the serial
   log instead of "it feels slower."

**Exit criteria:** vda as default root, full `startx` desktop soak-tested and stable, boot to
the Arch bash prompt in well under the current ATA-only baseline, verified with the timestamped
log from (5).

---

## Phase 2 — POSIX/Linux Completeness (breadth beyond one Arch happy-path)

This is the bulk of "runs any Linux distro," not just Arch with this exact package set.

- **Signals, for real.** Right now `kill`/`tgkill`/`tkill` only support default-terminate
  (Phase 25). No `sigaction` delivery, no handler invocation, no `SIGCHLD` to parents, no
  `SIGSTOP`/`SIGCONT` job control. This blocks: shells' job control (`^C`, `^Z`, `fg`/`bg`),
  anything using `SIGALRM`/timers for real, `systemd`, and most daemons. Needs: per-thread
  pending/blocked masks, an in-kernel trampoline that sets up a signal frame on the user stack
  and redirects `RIP` to the handler on return-to-userspace, and `sys_rt_sigreturn` restoring
  the saved context.
- **`/proc/self/fd` and `/proc/<pid>/fd`** — symlinks into the process fd table. Currently
  missing; breaks `pacman-db-upgrade` (bash process substitution `<(...)` needs `/dev/fd/N` →
  `/proc/self/fd/N`), `readlink /proc/self/exe` callers that also want `fd`, `lsof`-likes, etc.
  Straightforward extension of the existing `procfs.rs`.
- **Process groups & sessions.** `pgrp == pid` always today (Phase 24 known gap). Needed for
  real job control and for terminals to know which process group owns the foreground.
- **`clone3`, `rseq` completeness, `pidfd_open`** — modern glibc/musl and systemd increasingly
  assume these exist; currently unimplemented (`clone3` seen as "Unimplemented syscall" in
  testing).
- **`timerfd`/`signalfd`/`eventfd2` gaps** — `epoll`/`eventfd` are real (Phase 20-ish), but
  `timerfd_create`/`signalfd` return `-ENOSYS`; several daemons and toolkits (glib mainloops)
  want them.
- **Writable root filesystem.** ext2 is currently read-only with created files living in an
  in-memory shadow map (Phase 24 known gap) — fine for a demo, not for `pacman -S`, `useradd`,
  log files, or anything that expects persistence. Implement real ext2 block/inode allocation
  and directory-entry writes (the on-disk format is already understood; `xtask/src/ext2.rs` and
  `kernel/src/fs/ext2.rs` are the two ends to extend symmetrically).
- **User-pointer validation.** Syscall handlers currently dereference user pointers largely on
  trust (page-fault-and-kill covers *crashes*, but not e.g. a malicious length that reads kernel
  memory into a buffer before the fault). Add a systematic "is this whole range user-mapped and
  the right permission" check at syscall entry, not just fault-time recovery.
- **`ptrace`, `wait4` full status bits, core dumps** — needed by debuggers, some package
  post-install scripts, and test frameworks that shell out and check exit reasons precisely.

**Exit criteria:** a second, unrelated distro's minimal userland (see Phase 4) boots and runs a
normal interactive session — shell job control, a text editor, and `pacman`/`apt`/`dnf`
equivalent completing a real transaction — without kernel-side workarounds specific to that
distro.

---

## Phase 3 — Storage & Filesystem Hardening

- Default to AHCI/VirtIO-blk DMA (ties into Phase 1.1).
- Real writable ext2 (ties into Phase 2), or evaluate ext4 (journaling matters more once the fs
  is writable and QEMU/host crashes are possible) as a stretch goal — same reader/writer split
  pattern as ext2 (`fs/ext2.rs` kernel side, `xtask/src/ext2.rs` image-build side) would apply.
- Block-layer cache (ties into Phase 1.2) shared by all `BlockDevice` backends.
- `/tmp` as real tmpfs (in-RAM) instead of whatever ad hoc handling exists today, so packages
  that assume tmpfs semantics (world-writable, sticky bit, cleared on boot) behave correctly.

## Phase 4 — Distro Breadth (prove "any Linux," not "this Arch")

- Pick a second target distro deliberately different from Arch's assumptions — e.g. **Debian**
  (glibc but different init expectations, dpkg instead of pacman, different FHS conventions) or
  **Alpine** (musl instead of glibc — exercises a completely different libc's syscall
  assumptions, good stress test for Phase 2 gaps). Use the same "read real `.deb`/`.apk`
  packages directly, resolve real dependency closures" approach as
  `tools/resolve_arch_deps.py`, never hand-authored stand-ins.
- Track failures the same way `startx` failures were tracked this session: real binary, real
  error, fix the kernel gap it exposes, never patch around it in the image builder.
- Once two distros run, re-derive "what's actually distro-specific vs. universal Linux kernel
  surface" and fold the universal fixes back into Phase 2's checklist for future distros.

## Phase 5 — Reliability & Resource Management

- Fix the leaked page tables per `execve` (~130 KB/exec today per Phase 24/25 known gaps) —
  walk and free the *old* address space's page-table frames in `exec_elf_replace`, not just the
  data frames.
- Audit every `unwrap()`/panic path reachable from a syscall or user-triggered fault; a
  malicious or buggy *user* program should never be able to panic the kernel (only its own
  process should die — extend the `fatal_or_kill_user` pattern everywhere it's missing).
- Stress test: fork bombs, OOM (heap/frame exhaustion), rapid exec loops, killed-mid-syscall —
  confirm graceful `-ENOMEM`/`-EAGAIN` instead of halts.
- SMP: scheduler currently BSP-only per the architecture notes; if this is still true, decide
  whether true multi-core scheduling is in scope for "usable" (probably yes, eventually — most
  real workloads assume it) or explicitly deferred with a note here.

## Phase 6 — Networking (only as far as userspace needs it)

- Confirm DHCP or a usable default (`10.0.2.15` QEMU NAT is already up) covers `pacman -Sy`
  style network installs if that's ever wanted live rather than pre-fetched; otherwise this
  stays low priority — the project's current fetch-then-mirror-offline model
  (`resolve_arch_deps.py`) sidesteps needing a fully general TCP/IP stack.
- AF_INET sockets already work for the ARP/ICMP/e1000 path from earlier phases; AF_UNIX works
  (used by X11). Revisit only if a target distro/program needs something these don't cover
  (e.g. real DNS resolution via `/etc/resolv.conf` + UDP, which many programs assume exists
  even if unused).

## Phase 7 — Polish & Housekeeping

- Delete/rewrite the stale tests that still assert the removed mock UI:
  `tests/test_tinycore_gui.py`, `tests/test_arch_gui.py` (partially — the `startx` shape is
  still valid, the assertions may not be), older milestone tests referencing deleted subsystems.
- Update `AGENTS.md` §2 workspace tree (the `subsystems/nt/` directory tree shown there is
  fully deleted now — see Phase 8 below — and `display/compositor.rs`, `display/desktop.rs`,
  `display/window.rs` are also gone).
- Keep `docs/FULL_PROJECT_HANDOVER_REPORT.md` in sync as phases land.
- Clean up `xtask` unused imports (`flate2`, `calc_crc32`) noted from earlier work.

---

## Phase 8 — The Hybrid Windows+Linux Ambition (kept as a reminder, not scheduled)

The original framing for this project was a **hybrid kernel supporting Windows and Linux
software at the kernel level simultaneously** — no driver-support or software-support gap
between the two worlds. That's the north star the user wants recorded, even though it is not
being worked on right now: everything above (Phases 1–7) is "make the Linux side actually
solid," which has to exist first regardless of whether the Windows side ever gets built.

**Honest current state:** as of this session, the entire earlier Windows subsystem —
`kernel/src/subsystems/nt/` (PE32+ loader, `ntoskrnl`/`hal` DDI shims, WDM driver loader,
Win32 `kernel32`/`ntdll` thunks) — has been **deleted from the working tree**. There is
currently zero Windows-anything in the kernel. If this is revisited, it is a from-scratch build,
not a resume.

**Why it's genuinely hard** (worth keeping in mind before committing to it):
- Two incompatible native ABIs at once: Linux's `syscall`/`sysret` (SysV registers,
  negative-errno returns) vs. Windows' `ntdll` syscall stubs (different register convention,
  `NTSTATUS` returns) — both would need to coexist in the same syscall dispatch without
  colliding on vector/MSR usage.
- Two incompatible executable/loader formats already coexist in principle (ELF and PE32+ were
  both loaded before deletion) but a *driver model* is a much bigger surface: WDM's IRP-based
  I/O stack, PnP, IRQL-based synchronization (`DISPATCH_LEVEL`, spinlocks with IRQL raises) has
  no Linux equivalent and can't be faked with a thin shim if real `.sys` drivers are the goal —
  it needs its own IRQL-aware interrupt/scheduling story layered onto (or beside) the Linux one.
- "No driver support issue" implies real Windows drivers binding real hardware via the NT DDI —
  that's a much larger DDI surface than the handful of shims implemented previously
  (`ExAllocatePoolWithTag`, `IoCreateDevice`, etc.); production Windows drivers touch hundreds of
  `ntoskrnl`/`hal`/`win32k` exports.
- Memory model differences (Windows' VAD-based address space vs. this kernel's Linux-style VMA
  table) would need either a second parallel implementation or a careful unification.
- File system semantics differ (NTFS ACLs/streams vs. POSIX permissions) if real Windows
  userspace (not just PE binaries using a POSIX subset) is ever the goal.

**If/when this is picked up**, treat it as its own multi-phase project *after* Phase 1–7 land,
starting from: (a) a minimal PE32+ loader for user-mode-only Win32 console programs (no drivers,
no `ntoskrnl`) as the smallest real milestone, exactly like the ELF loader was bootstrapped here,
then (b) only after that's solid, evaluate whether kernel-level WDM driver hosting is worth the
IRQL/PnP investment versus, say, presenting Windows programs with a translated Linux backend
(closer to Wine's model, but at the syscall boundary instead of userspace) — that trade-off
should get its own design discussion before code, not be decided implicitly by whatever's
easiest to bolt on.

---

## Suggested order of attack

1. Phase 1 (boot time) — fast, high-visibility, makes every later iteration cycle faster too.
2. Phase 2 (signals, `/proc/self/fd`, process groups) — unblocks real shells/daemons/job control.
3. Phase 3 (storage/fs hardening, writable root) — needed before Phase 4 can do a real package
   install on a second distro.
4. Phase 4 (second distro) — the real breadth test; feeds back into Phase 2.
5. Phase 5 (reliability) — ongoing, but a dedicated pass once 1–4 land.
6. Phase 6 (networking) — only as pulled in by Phase 4's needs.
7. Phase 7 (housekeeping) — continuous, but a cleanup pass makes sense once the dust settles.
8. Phase 8 (Windows hybrid) — explicitly parked; revisit only by deliberate choice, with its own
   design pass, not as an incremental add-on.
