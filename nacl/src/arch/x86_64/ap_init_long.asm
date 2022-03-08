.code64
DATA_SEGMENT = (2 << 3)

longmode:
    mov ax, DATA_SEGMENT
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov sp, AP_STACK_START
    jmp ap_init
    
