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

bool statics_insert(const void *);
bool statics_contains(const void *);
bool statics_remove(const void *);

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

static inline bool handle_got_has_glob_dat(const struct handle *handle) {
	assert((handle->got_start == GOT_GAP) == (handle->sgot_start == GOT_GAP));
	return handle->got_start != GOT_GAP;
}

static inline int phdr_to_mprot(const ElfW(Phdr) *ph) {
	return ((ph->p_flags & PF_R) ? PROT_READ : 0) | ((ph->p_flags & PF_W) ? PROT_WRITE : 0) |
		((ph->p_flags & PF_X) ? PROT_EXEC : 0);
}

static inline void statics_foreach_nonexec_symbol(struct handle *library,
	struct link_map *namespace,
	bool (*function)(const void *)) {
	if(library->sechdr)
		for(const ElfW(Sym) *st = library->symtab; st != library->symtab_end; ++st)
			if(st->st_value && st->st_shndx &&
				!(library->sechdr[st->st_shndx].sh_flags & SHF_EXECINSTR))
				function((const void *) (namespace->l_addr + st->st_value));
}

// Returns NULL on error.
static const char *progname(void) {
	extern const char *__progname_full;
	static char progname[PATH_MAX];
	static bool ready;

	// This can race during initialization, but it should still be correct because realpath()
	// will always populate progname with the exact same contents.
	if(!ready) {
		if(strchr(__progname_full, '/'))
			realpath(__progname_full, progname);
		else {
			char which[PATH_MAX];
			char buf[PATH_MAX];
			const char *paths = getenv("PATH");
			assert(paths && "PATH variable not present in environment");
			buf[PATH_MAX - 1] = '\0';
			which[PATH_MAX - 2] = '\0';
			strncpy(buf, paths, PATH_MAX - 1);
			for(const char *path = strtok_r(buf, ":", (char **) &paths);
				path;
				path = strtok_r(NULL, ":", (char **) &paths)) {
				strncpy(which, path, PATH_MAX - 2);
				char *bound = strchr(which, '\0');
				*bound++ = '/';
				strncat(bound, __progname_full, PATH_MAX - (bound - which) - 1);
				if(!access(which, X_OK))
					strcpy(progname, which);
			}
		}
		ready = true;
	}
	return progname;
}

static size_t pagesize(void) {
	static volatile size_t pagesize;
	static volatile bool ready;
	if(!ready) {
		pagesize = sysconf(_SC_PAGESIZE);
		ready = true;
	}
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
	assert(l);
	if(n) {
		l = namespace_load(n, h->path, RTLD_LAZY);
		if(!l) {
			h->shadow->gots[n] = NULL;
			return ERROR_DLOPEN;
		}

		const ElfW(Dyn) *d;
		for(d = l->l_ld; d->d_tag != DT_JMPREL && d->d_tag != DT_NULL; ++d);
		pltrel = (ElfW(Rela) *) d->d_un.d_ptr;
		epltrel = h->pltrel_end - h->pltrel + pltrel;

		for(d = l->l_ld; d->d_tag != DT_PLTGOT; ++d)
			if(d == DT_NULL)
				assert(false && "Dynamic section without PLTGOT entry");
		got = (struct got *) d->d_un.d_ptr;
	}

	// Record symbols pointing to non-executable *data* so we know not to install trampolines
	// Record non-executable *data* symbols defined in the address so we know not to install
	// trampolines over GOT entries corresponding to them in this and future object files.
	//
	// Note that we don't create such a record for data symbols defined in unmirrored object
	// files: this is safe because such symbols cannot possibly be referenced from an ancillary
	// namespace, since any attempt to load a library requiring them would fail with an
	// undefined symbol error during dynamic linking.
	statics_foreach_nonexec_symbol(h, l, statics_insert);

	size_t len = handle_got_num_entries(h);
	memcpy(sgot, got, sizeof *got + h->got_len * sizeof *h->got->e);

	// Although this sets up correct shadowing of preresolved symbols, it breaks pointer
	// comparison of pointers to such symbols passed across object boundaries.  In order to
	// preserve this functionality, we'd need the PLOT entry to be a global associated directly
	// with the symbol address (since otherwise we'd need to perform an expensive lookup to
	// determine its address).  Whenever we handle_cleanup()'d, we'd need to traverse the symbol
	// table in search of symbol addresses having such associated globals, deallocating them.
	ssize_t index = h->sgot_start;
	for(const ElfW(Rela) *r = h->miscrel; r != h->miscrel_end; ++r)
		if(ELF64_R_TYPE(r->r_info) == R_X86_64_GLOB_DAT) {
			if(n && whitelist_shared_contains(h->strtab +
				h->symtab[ELF64_R_SYM(r->r_info)].st_name))
				// This symbol is shared among all namespaces.  Populate the
				// corresponding entry in the ancillary namespace's shadow GOT with
				// a NULL sentinel to indicate to the main trampoline that it should
				// load the address from the base shadow GOT instead.
				sgot->e[index++] = NULL;
			else
				// Back up the real GOT entry into the shadow GOT for this namespace
				// so we're ready to replace the GOT entry with a PLOT trampoline.
				sgot->e[index++] = *(const void **) (l->l_addr + r->r_offset);
		}
	assert(index <= GOT_GAP);

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
		void *page = NULL;
		size_t pgsz;
		if(!(h->pltprot & PROT_WRITE)) {
			page = (void *) ((uintptr_t) pltrel & ~(pagesize() - 1));
			pgsz = (uintptr_t) epltrel - (uintptr_t) page;
			if(mprotect(page, pgsz, h->pltprot | PROT_WRITE)) {
				if(n)
					dlclose(l);
				return ERROR_MPROTECT;
			}
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
			r->r_offset = (uintptr_t) (sgot->e + lazy) - l->l_addr;
		}

		if(page && mprotect(page, pgsz, h->pltprot)) {
			if(n)
				dlclose(l);
			return ERROR_MPROTECT;
		}
	}

	if(handle_got_has_glob_dat(h)) {
		void *page = NULL;
		size_t pgsz;
		if(!(h->miscprot & PROT_WRITE)) {
			page = (void *) ((uintptr_t) (got->e + h->got_start) & ~(pagesize() - 1));
			pgsz = (uintptr_t) got - (uintptr_t) page;
			assert(!(pgsz % pagesize()) && "End of immutable GOT not page aligned");
			if(mprotect(page, pgsz, h->miscprot | PROT_WRITE)) {
				if(n)
					dlclose(l);
				return ERROR_MPROTECT;
			}
		}

		if(!n) {
			ssize_t sindex = h->sgot_start;
			for(ssize_t index = h->got_start; index < GOT_GAP; ++index) {
				const void **entry = got->e + index;
				const void **sentry = sgot->e + sindex;
				if(*entry == *sentry) {
					// Only install a PLOT trampoline if:
					//   * the existing GOT entry is non-NULL (since code might
					//     NULL-check an address to decide whether to call it)
					// (and)
					//   * the entry corresponds to code rather than data (since
					//     attempting to read a trampoline would be misleading
					//     and attempting to write to it would be disastrous)
					if(*sentry && !statics_contains(*sentry))
						*entry = plot->code + plot_entries_offset + plot_entry_size *
							(sindex - h->sgot_start + h->shadow->first_entry);
					++sindex;
				}
			}
			assert(sindex == GOT_GAP);
		} else
			for(ssize_t index = h->got_start; index < GOT_GAP; ++index)
				if(plot_contains_entry(plot, h->got->e[index]))
					got->e[index] = h->got->e[index];

		if(page && mprotect(page, pgsz, h->miscprot)) {
			if(n)
				dlclose(l);
			return ERROR_MPROTECT;
		}
	}

	for(size_t index = -h->sgot_start + GOT_GAP; index < len; ++index) {
		ssize_t entry = index + h->sgot_start - GOT_GAP;
		got->e[entry] = plot->code + plot_entries_offset +
			(h->shadow->first_entry + index) * plot_entry_size;
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
		assert(!(pltrelsz % sizeof *h->pltrel) && "Relocation size entry does not divide PLTRELSZ");
		h->pltrel_end = (ElfW(Rela) *) ((uintptr_t) h->pltrel + pltrelsz);
	}

	assert(relasz && "Dynamic section without RELASZ entry");
	assert(!(relasz % sizeof *h->miscrel) && "Relocation size entry does not divide RELASZ");
	h->miscrel_end = (ElfW(Rela) *) ((uintptr_t) h->miscrel + relasz);

	// The symbol hash table is supposed to be present in all executables and shared libraries
	// according to the spec, but in practice it appears to sometimes be missing from the
	// former?! In that case, we use the trick from ld.so's dl-addr.c
	if(h->symhash)
		h->symtab_end = h->symtab + h->symhash->nchain;
	else
		h->symtab_end = (ElfW(Sym) *) h->strtab;

	const ElfW(Ehdr) *e = (ElfW(Ehdr) *) l->l_addr;
	const ElfW(Phdr) *p = (ElfW(Phdr) *) (l->l_addr + e->e_phoff);
	const ElfW(Phdr) *p_end = p + e->e_phnum;
	assert(!memcmp(e->e_ident, ELFMAG, SELFMAG) && "ELF header not loaded into process image?");
	if(e->e_shoff)
		h->sechdr = (ElfW(Shdr) *) (l->l_addr + e->e_shoff);

	if(h->pltrel) {
		for(const ElfW(Phdr) *ph = p; ph != p_end; ++ph)
			if(ph->p_type == PT_LOAD && l->l_addr + ph->p_vaddr <= (uintptr_t) h->pltrel &&
				(uintptr_t) h->pltrel_end <= l->l_addr + ph->p_vaddr + ph->p_memsz) {
				h->pltprot = phdr_to_mprot(ph);
				break;
			}
		assert(h->pltprot && "JUMP_SLOT relocations not within a single loadable segment");
	}
	for(const ElfW(Phdr) *ph = p; ph != p_end; ++ph) {
		if(ph->p_type == PT_LOAD && l->l_addr + ph->p_vaddr <= (uintptr_t) h->miscrel &&
			(uintptr_t) h->miscrel_end <= l->l_addr + ph->p_vaddr + ph->p_memsz) {
			h->miscprot = phdr_to_mprot(ph);
			break;
		}
	}
	assert(h->miscprot && "GLOB_DAT relocations not within a single loadable segment");

	// Dynamic relocation types enumerated in the switch statement in ld.so's dl-machine.h
	uintptr_t first = (uintptr_t) h->got - l->l_addr;
	const uintptr_t *last = &first;
	size_t count = 0;
	bool whitelisted_obj = whitelist_so_contains(h->path);
	for(const ElfW(Rela) *r = h->miscrel; r != h->miscrel_end; ++r)
		switch(ELF64_R_TYPE(r->r_info)) {
		case R_X86_64_GLOB_DAT:
			if(r->r_offset < first)
				first = r->r_offset;
			else if(!whitelisted_obj) {
				assert(l->l_addr + r->r_offset < (uintptr_t) h->got &&
					"Object file tagged with unsupported BIND_NOW or NOW?");

				// load_shaodw() relies on GLOB_DAT entries to be in order when it
				// sets up the shadow GOTs, so assert() that this is the case if we
				// might multiplex this object file in the future.
				assert(r->r_offset > *last);
				last = &r->r_offset;
			}
			++count;
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
			if(!h->sechdr || h->sechdr[st->st_shndx].sh_flags & SHF_WRITE)
				return ERROR_UNSUPPORTED_RELOCS;
			break;
		}
		}
	h->got_start = (const void **) (l->l_addr + first) - h->got->e;
	h->sgot_start = GOT_GAP - count;

	h->got_len = h->pltrel_end - h->pltrel;

	assert(h->got_start <= GOT_GAP);
	assert(h->sgot_start <= GOT_GAP);
	if(!whitelisted_obj)
		assert(h->got_start <= h->sgot_start);

	if(!whitelisted_obj && !h->got->l)
		h->got->l = (struct link_map *) l;

	return SUCCESS;
}

void handle_cleanup(struct handle *h) {
	if(!h)
		return;

	if(h->shadow) {
		for(struct got **it = h->shadow->gots + 1,
			**end = h->shadow->gots + NUM_SHADOW_NAMESPACES + 1;
			it != end;
			++it) {
			statics_foreach_nonexec_symbol(h, (*it)->l, statics_remove);
			if(*it)
				dlclose((*it)->l);
		}
		free(h->shadow->gots[0]->e + h->sgot_start);
		free(h->shadow);
	}
	if(h->got->l)
		statics_foreach_nonexec_symbol(h, h->got->l, statics_remove);
}

enum error handle_got_shadow(struct handle *h) {
	if(h->shadow)
		return SUCCESS;

	size_t len = handle_got_num_entries(h);
	size_t size = sizeof *h->got + len * sizeof *h->got->e;
	h->shadow = calloc(1, sizeof *h->shadow);
	if(!h->shadow)
		return ERROR_MALLOC;
	h->shadow->override_table = -1;
	h->shadow->first_entry = -1;

	void **gots = calloc(NUM_SHADOW_NAMESPACES + 1, size);
	if(!gots) {
		free(h->shadow);
		return ERROR_MALLOC;
	}
	for(Lmid_t namespace = 0; namespace <= NUM_SHADOW_NAMESPACES; ++namespace) {
		h->shadow->gots[namespace] = (struct got *) (
			(const void **) ((uintptr_t) gots + namespace * size) -
			h->sgot_start + GOT_GAP);

		enum error fail = load_shadow(h, namespace);
		if(fail) {
			handle_cleanup(h);
			return fail;
		}
	}

	return SUCCESS;
}
