#include "handle.h"

#include "plot.h"
#include "whitelist.h"

#include <sys/mman.h>
#include <assert.h>
#include <dlfcn.h>
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
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

static size_t pagesize(void) {
	static size_t pagesize;
	static bool ready;
	if(!ready)
		pagesize = sysconf(_SC_PAGESIZE);
	return pagesize;
}

static enum error load_shadow(struct handle *h, Lmid_t n) {
	assert(h->shadow);
	assert(h->shadow->gots[n]);

	const struct plot *plot = plot_insert_lib(h);
	if(!plot)
		return ERROR_LIB_SIZE;

	struct link_map *l = h->got->l;
	const ElfW(Rela) *pltrel = h->pltrel;
	const ElfW(Rela) *epltrel = h->pltrel_end;
	struct got *got = h->got;
	struct got *sgot = h->shadow->gots[n];
	assert(!n == (n == LM_ID_BASE));
	if(n) {
		l = namespace_load(n, h->path, RTLD_LAZY);
		if(!l)
			return ERROR_DLOPEN;

		const ElfW(Dyn) *d;
		for(d = l->l_ld; d->d_tag != DT_JMPREL && d->d_tag != DT_NULL; ++d);
		pltrel = (ElfW(Rela) *) d->d_un.d_ptr;
		epltrel = h->pltrel_end - h->pltrel + pltrel;

		for(d = l->l_ld; d->d_tag != DT_PLTGOT; ++d)
			if(d == DT_NULL)
				assert(false && "Dynamic section without PLTGOT entry");
		got = (struct got *) d->d_un.d_ptr;
	}

	size_t len = handle_got_num_entries(h);
	size_t size = sizeof *got + len * sizeof *got->e;
	memcpy(sgot->e + h->got_start, got->e + h->got_start, size);

	// Although this sets up correct shadowing of preresolved symbols, it breaks pointer
	// comparison of pointers to such symbols passed across object boundaries.  In order to
	// preserve this functionality, we'd need the PLOT entry to be a global associated directly
	// with the symbol address (since otherwise we'd need to perform an expensive lookup to
	// determine its address).  Whenever we handle_cleanup()'d, we'd need to traverse the symbol
	// table in search of symbol addresses having such associated globals, deallocating them.
	for(const ElfW(Rela) *r = h->miscrel; r != h->miscrel_end; ++r)
		if(whitelist_shared_contains(h->strtab +
			h->symtab[ELF64_R_SYM(r->r_info)].st_name)) {
			ssize_t index = (const void **) (h->got->l->l_addr + r->r_offset) - (h->got->e + h->got_start);
			sgot->e[index] = h->shadow->gots[0]->e[index];
		}

	if(!sgot->l) {
		// All entries in the GOT were resolved at load time, so the dynamic linker didn't
		// bother to populate the special GOT entries.  We'll never need to call the
		// resolver function f(), but we will need the link_map l in order to deallocate
		// this (owned) library copy in the future, so manually populate it now.
		sgot->l = l;
		if(n)
			for(const ElfW(Rela) *r = pltrel; r != epltrel; ++r) {
				uintptr_t lazy = (const void **) (l->l_addr + r->r_offset) - got->e;
				if(whitelist_shared_contains(h->strtab +
					h->symtab[ELF64_R_SYM(r->r_info)].st_name))
					// This symbol is shared across all namespaces.  Populate
					// the shadow GOT entry with the sentinel NULL, meaning that
					// the trampoline should load it from the *base* shadow GOT
					// instead; this allows lazy resolution to work regardless
					// of which namespace makes the initial call to the symbol!
					sgot->e[lazy] = NULL;
			}
	} else if(pltrel) {
		// The GOT contains entries that will be resolved lazily.  This poses a problem
		// because, whenever one is resolved, the resolver function f() will automatically
		// overwrite its corresponding GOT entry to memoize the result, thereby disabling
		// our multiplexer trampoline!  We need it to rewrite the *shadow* GOT entry
		// instead, so rewrite the dynamic relocation entry to target that location.
		ElfW(Rela) *page = (ElfW(Rela) *) ((uintptr_t) pltrel & ~(pagesize() - 1));
		size_t pgsz = epltrel - page;
		if(mprotect(page, pgsz, PROT_READ | PROT_WRITE)) {
			if(n)
				dlclose(l);
			return ERROR_MPROTECT;
		}

		for(ElfW(Rela) *r = (ElfW(Rela) *) pltrel; r != epltrel; ++r) {
			uintptr_t lazy = (const void **) (l->l_addr + r->r_offset) - got->e;
			if(n && whitelist_shared_contains(h->strtab +
				h->symtab[ELF64_R_SYM(r->r_info)].st_name)) {
				// Install a sentinel shadow GOT entry, as above.  This should
				// activate the trampoline's special logic, thereby preventing us
				// from ever calling the resolver function f() with this index;
				// however, as a means of asserting this, point the relocation entry
				// to NULL so any later attempt by the dynamic linker to rewrite the
				// sentinel entry will fail.
				sgot->e[lazy] = NULL;
				lazy = -(uintptr_t) got->e;
			}
			r->r_offset = (uintptr_t) (got->e + lazy) - l->l_addr;
		}

		if(mprotect(page, pgsz, PROT_READ)) {
			if(n)
				dlclose(l);
			return ERROR_MPROTECT;
		}
	}

	for(size_t index = 0; index < len; ++index) {
		ssize_t entry = index + h->got_start;
		if(entry >= GOT_GAP)
			entry += GOT_GAP;
		h->got->e[entry] = plot->code + plot_entries_offset +
			(h->shadow->first_entry + entry) * plot_entry_size;
	}

	return SUCCESS;
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
	intptr_t first = (intptr_t) h->got - l->l_addr;
	bool whitelisted_obj = whitelist_so_contains(h->path);
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
			if(whitelisted_obj || whitelist_copy_contains(h->strtab + st->st_name))
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
	h->got_start = (const void **) (l->l_addr + first) - h->got->e;
	if(sh) {
		munmap((void *) ((uintptr_t) sh - shoff), shlen);
		close(fd);
	}

	h->got_len = h->pltrel_end - h->pltrel;

	return SUCCESS;
}

void handle_cleanup(struct handle *h) {
	if(h && h->shadow) {
		for(struct got **it = h->shadow->gots + 1,
			**end = h->shadow->gots + NUM_SHADOW_NAMESPACES + 1;
			it != end;
			++it)
			if(*it)
				dlclose((*it)->l);
		free(h->shadow->gots[0]->e + h->got_start);
		free(h->shadow);
	}
}

enum error handle_got_shadow(struct handle *h) {
	if(h->shadow)
		return SUCCESS;

	size_t len = handle_got_num_entries(h);
	size_t size = sizeof *h->got + len * sizeof *h->got->e;
	h->shadow = calloc(sizeof *h->shadow, 1);
	if(!h->shadow)
		return ERROR_CALLOC;
	h->shadow->override_table = -1u;
	h->shadow->first_entry = -1u;

	void **gots = malloc((NUM_SHADOW_NAMESPACES + 1) * size);
	if(!gots) {
		free(h->shadow);
		return ERROR_MALLOC;
	}
	for(Lmid_t namespace = 0; namespace <= NUM_SHADOW_NAMESPACES; ++namespace) {
		h->shadow->gots[namespace] = (struct got *) gots + namespace * size -
			h->got_start + GOT_GAP;

		enum error fail = load_shadow(h, namespace);
		if(fail) {
			handle_cleanup(h);
			return fail;
		}
	}

	return SUCCESS;
}
