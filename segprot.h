#ifndef SEGPROT_H_
#define SEGPROT_H_

#include <sys/mman.h>
#include <link.h>
#include <stdint.h>

// Returns the first segment containing the virtual memory offset, starting from phdr (inclusive).
const ElfW(Phdr) *segment(uintptr_t offset, const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end);

// Returns the *largest* write-protected segment containing offset, starting from phdr (inclusive).
const ElfW(Phdr) *segment_unwritable(uintptr_t offset, const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end);

int prot(const ElfW(Phdr) *p);
int prot_segment(uintptr_t base, const ElfW(Phdr) *p, int grants);

#endif