   .section .text
    .globl __timehandler
    .align 2
__timehandler:
    csrrw a0, mscratch, a0
    sd a1, 0(a0)
    sd a2, 8(a0)
    sd a3, 16(a0)

    li a1, 0x02004000
    ld a2, 0(a1)
    li a3, 1000000
    add a2, a2, a3
    sd a2, 0(a1)

    li a1, 2
    csrw sip, a1

    ld a3, 16(a0)
    ld a2, 8(a0)
    ld a1, 0(a0)
    csrrw a0, mscratch, a0

    mret
.section .data              
.align  2                   
.globl __timer_scratch 
__timer_scratch:
    .space 24                