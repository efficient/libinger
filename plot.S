#include "plot.h"

procedure_linkage_override:
	add $16, %rsp
	ret

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

.globl plot_template
plot_template:
	.quad 0x0
	.quad procedure_linkage_override
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

.section .rodata
.align 8
.globl plot_size
plot_size:
	.quad plot_template_end - plot_template

.globl plot_entries_offset
plot_entries_offset:
	.quad plot_template_entries - plot_template - 8

.globl plot_entry_size
plot_entry_size:
	.quad plot_template_entry_end - plot_template_entries
