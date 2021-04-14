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
.type procedure_linkage_override, @function
.globl procedure_linkage_override
procedure_linkage_override:
.cfi_startproc
.cfi_adjust_cfa_offset 16
	push %rax
.cfi_adjust_cfa_offset 8
	push %rcx
.cfi_adjust_cfa_offset 8
	push %rdx
.cfi_adjust_cfa_offset 8

	mov  24(%rsp), %rcx          // GOOT address
	mov  32(%rsp), %rdx          // trampoline index
	mov  %rcx, %rax
	mov  24(%rcx, %rdx, 8), %rcx // handle/shadow address
	add   8(%rax), %rdx          // index += adjustment
	mov   0(%rax), %rax          // identifier

	cmp   0(%rcx), %rax          // override_table == identifier
	jne  .Lpreceding_page
	sub   8(%rcx), %edx          // index -= first_entry
	movsx %edx, %rdx
	add  16(%rcx), %rdx          // index += last_adjustment
.Lpreceding_page:

	add  $24, %rcx               // gots address
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
	movq $0, (%rax)              // clear namespace
	call namespace_caller        // caller address
	mov  %rdx, (%rax)            // save namespace
	lea  hook_retpoline(%rip), %rdx
	mov  %rdx,  24(%rsp)         // stack alignment fixup "return" address
	lea  hook_trampoline(%rip), %rdx
	mov  %rdx,  32(%rsp)         // callback hook trampoline "return" address
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
.cfi_adjust_cfa_offset -8
	pop  %rcx
.cfi_adjust_cfa_offset -8
	pop  %rax
.cfi_adjust_cfa_offset -8
	add  $8, %rsp                // symbol address address
.cfi_adjust_cfa_offset -8
	ret                          // symbol address
.cfi_endproc
.size procedure_linkage_override, .-procedure_linkage_override

// Stack at entry:
//  offset contents
//  ===============
//     0   return address (hook_trampoline() function)
//     8   preserved return address
//    --   (alignment reference point)
.type hook_retpoline, @function
hook_retpoline:
.cfi_startproc
	// Fix stack alignment
	ret
.cfi_endproc
.size hook_retpoline, .-hook_retpoline

// Stack at entry:
//  offset contents
//  ===============
//     0   preserved return address
//
// Stack during execution:
//  offset contents
//  ===============
//     0   (alignment gap)
//     8   saved %rdx
//    16   saved %rax
//    24   preserved return address
.type hook_trampoline, @function
hook_trampoline:
.cfi_startproc
	push %rax
.cfi_adjust_cfa_offset 8
	push %rdx
.cfi_adjust_cfa_offset 8
	sub $8, %rsp                 // alignment
.cfi_adjust_cfa_offset 8

	call namespace_caller        // caller address
	mov  (%rax), %rcx            // namespace
	movq $0, (%rax)              // clear caller
	call namespace_thread        // namespace address
	mov  %rcx, (%rax)            // restore namespace

	mov  %rcx, %rdi              // namespace
	call namespace_trampolining  // trampolining address
	mov  (%rax), %cl             // trampolining
	test %cl, %cl
	jnz  .Lskip_hook
	mov  shared_trampoline@gotpcrel(%rip), %rcx
	mov  (%rcx), %rcx
	test %rcx, %rcx
	jz   .Lskip_hook
	movb $1, (%rax)              // lock ourselves out of here
	mov  %rax, (%rsp)            // save trampolining address

	call  *%rcx

	mov  (%rsp), %rax            // trampolining address
	movb $0, (%rax)              // "unlock," as John Bercow would say
.Lskip_hook:

	add  $8, %rsp
.cfi_adjust_cfa_offset -8
	pop  %rdx
.cfi_adjust_cfa_offset -8
	pop  %rax
.cfi_adjust_cfa_offset -8
	ret
.cfi_endproc
.size hook_trampoline, .-hook_trampoline

.type plot_is_trampoline, @function
plot_is_trampoline:
.globl plot_is_trampoline
.cfi_startproc
	mov   hook_retpoline@gotpcrel(%rip), %rax
	cmp   %rdi, %rax
	je    .Lshortcircuit

	mov   hook_trampoline@gotpcrel(%rip), %rax
	cmp   %rdi, %rax

.Lshortcircuit:
	sete  %al
	movzb %al, %eax
	ret
.cfi_endproc
.size plot_is_trampoline, .-plot_is_trampoline

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
