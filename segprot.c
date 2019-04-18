#include "segprot.h"

#include "plot.h"

#include <sys/mman.h>
#include <assert.h>
#include <stddef.h>

const ElfW(Phdr) *segment(uintptr_t offset,
	const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end) {
	assert(phdr);
	assert(phdr_end);

	if(phdr == phdr_end)
		return NULL;

	const ElfW(Phdr) *p;
	for(p = phdr; offset < p->p_vaddr || p->p_vaddr + p->p_memsz <= offset; ++p)
		if(p + 1 == phdr_end)
			return NULL;
	return p;
}

const ElfW(Phdr) *segment_unwritable(uintptr_t offset,
	const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end) {
	const ElfW(Phdr) *p;
	for(p = segment(offset, phdr, phdr_end); p && p->p_flags & PF_W;
		p = segment(offset, p + 1, phdr_end));
	return p;
}

int prot(const ElfW(Phdr) *p) {
	assert(p);

	uint32_t pf = p->p_flags;
	return ((pf & PF_R) ? PROT_READ : 0) | ((pf & PF_W) ? PROT_WRITE : 0) |
		((pf & PF_X) ? PROT_EXEC : 0);
}

int prot_segment(uintptr_t base, const ElfW(Phdr) *p, int grants) {
	if(!p)
		return 0;

	uintptr_t addr = base + p->p_vaddr;
	size_t offset = addr & (plot_pagesize() - 1);
	return mprotect((void *) (addr - offset), p->p_memsz + offset, prot(p) | grants);
}
