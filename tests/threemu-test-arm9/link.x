/* ARM9 Memory Layout for Nintendo 3DS - Using shared FCRAM
 * ARM9 tests are loaded at 0x21000000 in shared FCRAM
 */

MEMORY
{
    /* ARM9 region in shared FCRAM - 16MB reserved */
    RAM : ORIGIN = 0x21000000, LENGTH = 16M
}

/* Entry point */
ENTRY(_start)

/* Stack configuration - grows down from end of RAM */
_stack_size = 0x4000;  /* 16KB stack */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);  /* Top of stack (8-byte aligned) */

SECTIONS
{
    .text :
    {
        /* Put _start first */
        *(.text._start);
        *(.text .text.*);
    } > RAM

    .rodata :
    {
        *(.rodata .rodata.*);
    } > RAM

    .data :
    {
        . = ALIGN(4);
        __sdata = .;
        *(.data .data.*);
        . = ALIGN(4);
        __edata = .;
    } > RAM
    __sidata = LOADADDR(.data);

    .bss (NOLOAD) :
    {
        . = ALIGN(4);
        __sbss = .;
        *(.bss .bss.*);
        *(COMMON);
        . = ALIGN(4);
        __ebss = .;
    } > RAM

    /* Heap starts after BSS */
    . = ALIGN(4);
    __sheap = .;

    /* Discard unwanted sections */
    /DISCARD/ :
    {
        *(.ARM.exidx);
        *(.ARM.exidx.*);
        *(.ARM.extab);
        *(.ARM.extab.*);
    }
}
