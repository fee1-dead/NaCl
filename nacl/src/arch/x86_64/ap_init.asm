CODE_SEGMENT = (1 << 3)
DATA_SEGMENT = (2 << 3)

.code16

trampoline_impl:

xor ax, ax
mov ds, ax
mov es, ax
mov ss, ax
mov fs, ax
mov gs, ax
cli
cld

// Disable IRQs
mov al, 0xFF // Out 0xFF to 0xA1 and 0x21 to disable all IRQs.
out 0xA1, al
out 0x21, al

// Enter long mode by setting the PAE and PGE bits
mov eax, 0b10100000
mov cr4, eax

// Point CR3 at the PML4.
// mov eax, [pml4]            
mov cr3, eax

// Read from the EFER MSR
mov ecx, 0xC0000080
rdmsr

// Set the Long Mode Enable (LME) bit
or eax, 0x00000100
wrmsr

// Load the global descriptor table
lgdt gdt32ptr

// Jump to Long mode assembly, using the 64 bit code segment selector.
jmp 0x8, offset longmode

gdt32ptr:
    .word gdt32_end - gdt32 - 1  # last byte in table
    .word gdt32                  # start of table
gdt32:
    .quad 0
codedesc:
    .byte 0xff
    .byte 0xff
    .byte 0
    .byte 0
    .byte 0
    .byte 0x9a
    .byte 0xcf
    .byte 0
datadesc:
    .byte 0xff
    .byte 0xff
    .byte 0
    .byte 0
    .byte 0
    .byte 0x92
    .byte 0xcf
    .byte 0
gdt32_end: