#include "handle.h"

#include "whitelist.h"

#include <sys/mman.h>
#include <assert.h>
#include <fcntl.h>
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PLT_ENTRY_LEN 16

struct sym_hash {
	uint32_t nbucket;
	uint32_t nchain;
	uint32_t indices[];
};

// From Book III of the TIS/ELF Specification, V1.2
static inline size_t elf_hash(const char *name) {
	size_t g, h = 0;
	while(*name) {
		h = (h << 4) + *name++;
		if((g = h & 0xf0000000))
			h ^= g >> 24;
		h &= ~g;
	}
	return h;
}

static inline const ElfW(Sym) *handle_symbol_hashtable(const struct handle *handle, const char *symbol) {
	size_t index = handle->symhash->indices[elf_hash(symbol) % handle->symhash->nbucket];
	const ElfW(Sym) *st = handle->symtab + index;
	if(!strcmp(handle->strtab + st->st_name, symbol))
		return st;

	do {
		index = handle->symhash->indices[handle->symhash->nbucket + index];
		if(index == STN_UNDEF)
			return NULL;

		st = handle->symtab + index;
	} while(strcmp(handle->strtab + st->st_name, symbol));
	return st;
}

static inline const ElfW(Sym) *handle_symbol_linkedlist(const struct handle *handle, const char *symbol) {
	for(const ElfW(Sym) *st = handle->symtab; st != handle->symtab_end; ++st)
		if(!strcmp(handle->strtab + st->st_name, symbol))
			return st;
	return NULL;
}

const ElfW(Sym) *handle_symbol(const struct handle *handle, const char *symbol) {
	assert(handle);
	assert(symbol);

	if(handle->symhash)
		return handle_symbol_hashtable(handle, symbol);
	return handle_symbol_linkedlist(handle, symbol);
}

// Returns NULL on error.
static const char *progname(void) {
	extern const char *__progname_full;
	static char progname[PATH_MAX];
	static bool ready;

	const char *res = progname;
	// This can race during initialization, but it should still be correct because realpath()
	// will always populate progname with the exact same contents.
	if(!ready && (res = realpath(__progname_full, progname)))
		ready = true;
	return res;
}

enum error handle_init(struct handle *h, const struct link_map *l) {
	assert(h);
	assert(l);

	memset(h, 0, sizeof *h);
	h->path = l->l_name;
	if((!h->path || !*h->path) && !(h->path = progname()))
		return ERROR_FNAME_REALPATH;

	size_t pltrelsz = 0;
	size_t relasz = 0;
	for(const ElfW(Dyn) *d = l->l_ld; d->d_tag != DT_NULL; ++d)
		switch(d->d_tag) {
		case DT_PLTGOT:
			h->got = (struct got *) d->d_un.d_ptr;
			break;
		case DT_JMPREL:
			h->pltrel = (ElfW(Rela) *) d->d_un.d_ptr;
			break;
		case DT_PLTRELSZ:
			pltrelsz = d->d_un.d_val;
			break;
		case DT_RELA:
			h->miscrel = (ElfW(Rela) *) d->d_un.d_ptr;
			break;
		case DT_RELASZ:
			relasz = d->d_un.d_val;
			break;
		case DT_SYMTAB:
			h->symtab = (ElfW(Sym) *) d->d_un.d_ptr;
			break;
		case DT_HASH:
			h->symhash = (struct sym_hash *) d->d_un.d_ptr;
			break;
		case DT_STRTAB:
			h->strtab = (const char *) d->d_un.d_ptr;
			break;

		case DT_PLTREL:
			assert(d->d_un.d_val == DT_RELA && "PLT uses non-Rela relocation entries");
			break;
		case DT_RELAENT:
			assert(d->d_un.d_val == sizeof *h->miscrel && "Rela entry size mismatch");
			break;
		case DT_REL:
			assert(!d->d_un.d_ptr && "Dynamic section has REL entry");
			break;
		case DT_SYMENT:
			assert(d->d_un.d_val == sizeof *h->symtab && "Sym entry size mismatch");
			break;
		}
	assert(h->got && "Dynamic section without PLTGOT entry");
	assert((!h->got->l || h->got->l == l) && "Lazy resolution with a mismatched handle in GOT");
	assert(h->miscrel && "Dynamic section without RELA entry");
	assert(h->symtab && "Dynamic section without SYMTAB entry");
	assert(h->strtab && "Dynamic section without STRTAB entry");

	if(h->pltrel) {
		assert(pltrelsz && "Dynamic section without PLTRELSZ entry");
		h->pltrel_end = (ElfW(Rela) *) ((uintptr_t) h->pltrel + pltrelsz);
	}

	assert(relasz && "Dynamic section without RELASZ entry");
	h->miscrel_end = (ElfW(Rela) *) ((uintptr_t) h->miscrel + relasz);

	// The symbol hash table is supposed to be present in all executables and shared libraries
	// according to the spec, but in practice it appears to sometimes be missing from the
	// former?! In that case, we use the trick from ld.so's dl-addr.c
	if(h->symhash)
		h->symtab_end = h->symtab + h->symhash->nchain;
	else
		h->symtab_end = (ElfW(Sym) *) h->strtab;

	// Dynamic relocation types enumerated in the switch statement in ld.so's dl-machine.h
	intptr_t first = (intptr_t) &h->got;
	const ElfW(Shdr) *sh = NULL;
	size_t shoff;
	size_t shlen;
	int fd;
	for(const ElfW(Rela) *r = h->miscrel; r != h->miscrel_end; ++r)
		switch(ELF64_R_TYPE(r->r_info)) {
		case R_X86_64_GLOB_DAT:
			if(r->r_offset < (uintptr_t) first)
				first = r->r_offset;
			break;

		case R_X86_64_JUMP_SLOT:
			assert(false && "JUMP_SLOT allocation outside JMPREL region");
			break;
		case R_X86_64_DTPMOD64:
		case R_X86_64_DTPOFF64:
		case R_X86_64_TLSDESC:
		case R_X86_64_TPOFF64:
		case R_X86_64_COPY:
		case R_X86_64_IRELATIVE: {
			const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);
			if(whitelist_copy_contains(h->strtab + st->st_name))
				continue;

			if(!sh) {
				const ElfW(Ehdr) *e = (ElfW(Ehdr) *) l->l_addr;
				assert(!memcmp(e->e_ident, ELFMAG, SELFMAG) && "Ehdr missing magic");
				assert(e->e_shoff && "Object file missing section header");
				shoff = e->e_shoff;
				assert(e->e_shentsize == sizeof *sh && "Shdr size mismatch");
				shlen = shoff + e->e_shnum * sizeof *sh;
				if((fd = open(h->path, O_RDONLY)) < 0)
					return ERROR_OPEN;
				intptr_t obj = (intptr_t)
					mmap(NULL, shlen, PROT_READ, MAP_SHARED, fd, 0);
				if(obj == (intptr_t) MAP_FAILED) {
					close(fd);
					return ERROR_MMAP;
				}
				sh = (ElfW(Shdr) *) (obj + shoff);
			}

			if(sh[st->st_shndx].sh_flags & SHF_WRITE) {
				munmap((void *) ((uintptr_t) sh - shoff), shlen);
				close(fd);
				return ERROR_UNSUPPORTED_RELOCS;
			}
			break;
		}
		}
	h->got_start = (intptr_t) h->got->e - l->l_addr - first;
	if(sh) {
		munmap((void *) ((uintptr_t) sh - shoff), shlen);
		close(fd);
	}

	return SUCCESS;
}
