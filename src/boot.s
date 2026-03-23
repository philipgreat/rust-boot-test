.text
.global efi_main

efi_main:
    lea rax, [rip + stack_top]
    and rax, -16
    mov rsp, rax
    sub rsp, 32
    call rust_efi_main
    add rsp, 32
    ret

.bss
.balign 16
stack_bottom:
    .space 65536
stack_top:
