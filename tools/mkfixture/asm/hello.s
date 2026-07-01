// Source for the machine code embedded as HELLO_TEXT in ../src/lib.rs.
//
// Raw Darwin BSD syscalls only (no libSystem, no dynamic linking) — this
// is original, hand-written test code, not derived from any Apple binary.
// Regenerate the bytes with:
//
//   llvm-mc -triple=aarch64-linux-gnu -filetype=obj hello.s -o hello.o
//   llvm-objcopy -O binary --only-section=.text hello.o hello.text.bin
//   od -An -tx1 hello.text.bin
//
// (aarch64-linux-gnu is used as the assembler triple purely because it's
// the target llvm-mc has on hand in this environment; the resulting
// instruction *bytes* are OS-agnostic ARM64 encodings and identical to
// what an aarch64-apple-ios triple would produce for this input.)
.text
.globl _start
_start:
    // Darwin: write(1, msg, 3)
    mov x0, #1
    adr x1, msg
    mov x2, #3
    mov x16, #4      // SYS_write
    svc #0x80
    // Darwin: exit(0)
    mov x0, #0
    mov x16, #1      // SYS_exit
    svc #0x80
msg:
    .ascii "Hi\n"
