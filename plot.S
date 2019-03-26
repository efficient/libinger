#include "plot.h"

// Stack at entry:
//  offset contents
//  ===============
//     0   GOOT address
//     8   trampoline index
//    16   return address (must be preserved)
//
// Stack during execution:
//  offset contents
//  ===============
//     0   saved %rdx
//     8   saved %rcx
//    16   saved %rax
//    24   GOOT address
//    32   trampoline index
//    40   return address (must be preserved)
.globl procedure_linkage_override
procedure_linkage_override:
	push %rax
	push %rcx
	push %rdx

	mov  24(%rsp), %rcx          // GOOT address
	mov  32(%rsp), %rdx          // trampoline index
	mov   8(%rcx, %rdx, 8), %rcx // handle address
	lea  24(%rcx), %rcx          // shadow address
	sub   8(%rcx), %edx          // index -= first_entry
	movsx %edx, %rdx
	add  $16, %rcx               // gots address
	mov  %rcx, 24(%rsp)
	mov  %rdx, 32(%rsp)

	call namespace_thread        // namespace address
	mov  (%rax), %rdx            // namespace
	mov  (%rcx, %rdx, 8), %rcx   // got address: gots[namespace]
	mov  32(%rsp), %rdx          // GOT index
	mov  (%rcx, %rdx, 8), %rcx   // symbol address

	test %rcx, %rcx
	jnz  .Lfound_symbol
	mov  24(%rsp), %rcx          // gots address
	mov  (%rcx), %rcx            // got address: gots[BASE]
	mov  (%rcx, %rdx, 8), %rcx   // symbol address
	mov  (%rax), %rdx            // namespace
	mov  %rdx,  32(%rsp)         // save namespace
	lea  hook_trampoline(%rip), %rdx
	mov  %rdx,  24(%rsp)         // intermediate return address
	xor  %rdx, %rdx
	mov  %rdx, (%rax)            // clear namespace
	mov    (%rsp), %rdx          // saved %rdx
	mov  %rdx, -16(%rsp)         // save %rdx
	mov   8(%rsp), %rdx          // saved %rcx
	mov  %rdx,  -8(%rsp)         // save %rcx
	mov  16(%rsp), %rdx          // saved %rax
	mov  %rdx,    (%rsp)         // save %rax
	sub  $16, %rsp
.Lfound_symbol:

	mov  %rcx, 32(%rsp)          // return address
	pop  %rdx
	pop  %rcx
	pop  %rax
	add  $8, %rsp                // symbol address address
	ret                          // symbol address

// Stack at entry:
//  offset contents
//  ===============
//     0   saved namespace
//     8   preserved return address
//
// Stack during execution:
//  offset contents
//  ===============
//     0   saved %rax
//     8   alignment gap
//    16   saved namespace
//    32   preserved return address
hook_trampoline:
	sub  $8, %rsp
	push %rax

	call namespace_thread        // namespace address
	mov  16(%rsp), %rcx          // namespace
	mov  %rcx, (%rax)            // restore namespace
	mov    (%rsp), %rcx          // saved %rax
	mov  %rcx, 16(%rsp)          // save %rax
	lea  hook_ret(%rip), %rcx
	mov  %rcx,  8(%rsp)          // intermediate return address

	add  $8, %rsp
	mov  shared_trampoline@gotpcrel(%rip), %rcx
	mov  (%rcx), %rcx
	jmp  *%rcx

// Stack at entry:
//  offset contents
//  ===============
//     0   saved %rax
//     8   return address
hook_ret:
	pop  %rax
	ret                          // preserved return address

// Ideally, the assembly macro alone would suffice to generate all table entries; unfortunately, GAS
// uses a hardcoded recursion depth limit, so we have to unroll the loop to support larger tables.
#define ENTRY(id, count) \
		entry (id), ((count) / 5); \
		entry ((id) + (count) / 5), ((count) / 5); \
		entry ((id) + (count) / 5 * 2), ((count) / 5); \
		entry ((id) + (count) / 5 * 3), ((count) / 5); \
		entry ((id) + (count) / 5 * 4), ((count) / 5); \
	.if count % 5; \
		entry ((id) + (count) / 5 * 5), ((count) % 5); \
	.endif; \

.macro entry id=0, count=1
	push $\id
	jmp  plot_template_code
.iflt \id - 11
	nop
	nop
	nop
.endif
.iflt \id - 0x80
	nop
	nop
	nop
.endif
.if \count - 1
	entry (\id + 1), (\count - 1)
.endif
.endm

.section .rodata
.align 8
.globl plot_template
plot_template:
	.quad 0x0
	.quad 0x0
plot_template_code:
	push -0x16(%rip)
	push -0x14(%rip)
	ret
	nop
	nop
	nop
plot_template_entries:
	entry 0
plot_template_entry_end:
	ENTRY(1, (PLOT_ENTRIES_PER_PAGE - 1))
plot_template_end:

.align 8
.globl plot_size
plot_size:
	.quad plot_template_end - plot_template

.globl plot_entries_offset
plot_entries_offset:
	.quad plot_template_entries - plot_template_code

.globl plot_entry_size
plot_entry_size:
	.quad plot_template_entry_end - plot_template_entries
