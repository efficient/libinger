#include "handle.h"

#include "plot.h"
#include "whitelist.h"

#include <sys/mman.h>
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef const struct link_map *(*dlm_t)(Lmid_t, const char *, int);

bool globals_insert(uintptr_t, uintptr_t);
bool globals_contains(uintptr_t);
uintptr_t globals_get(uintptr_t);
void globals_set(uintptr_t, uintptr_t);
bool globals_remove(uintptr_t);

struct sym_hash {
	uint32_t nbucket;
	uint32_t nchain;
	uint32_t indices[];
};

static inline const ElfW(Phdr) *segment(uintptr_t offset,
	const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end) {
	assert(offset);
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

static inline const ElfW(Phdr) *segment_unwritable(uintptr_t offset,
	const ElfW(Phdr) *phdr, const ElfW(Phdr) *phdr_end) {
	const ElfW(Phdr) *p;
	for(p = segment(offset, phdr, phdr_end); p && p->p_flags & PF_W;
		p = segment(offset, p + 1, phdr_end));
	return p;
}

static inline int prot(const ElfW(Phdr) *p) {
	assert(p);

	uint32_t pf = p->p_flags;
	return ((pf & PF_R) ? PROT_READ : 0) | ((pf & PF_W) ? PROT_WRITE : 0) |
		((pf & PF_X) ? PROT_EXEC : 0);
}

static int prot_segment(uintptr_t base, const ElfW(Phdr) *p, int grants) {
	if(!p)
		return 0;

	static uintptr_t pagemask;
	if(!pagemask)
		pagemask = sysconf(_SC_PAGESIZE) - 1;

	uintptr_t addr = base + p->p_vaddr;
	size_t offset = addr & pagemask;
	return mprotect((void *) (addr - offset), p->p_memsz + offset, prot(p) | grants);
}

// Returns NULL on error.
static const char *progname(void) {
	extern const char *__progname_full;
	static const char *progname;
	static char resolved[PATH_MAX];
	static bool ready;

	// This can race during initialization, but it should still be correct because it will
	// always populate progname with the exact same contents.
	if(!ready) {
		if(strchr(__progname_full, '/'))
			progname = __progname_full;
		else {
			char buf[PATH_MAX];
			char *paths = getenv("PATH");
			assert(paths && "PATH variable not present in environment");
			strncpy(buf, paths, sizeof buf - 1);
			for(const char *path = strtok_r(buf, ":", &paths);
				!progname && path;
				path = strtok_r(NULL, ":", &paths)) {
				snprintf(resolved, sizeof resolved, "%s/%s", path, __progname_full);
				if(!access(resolved, X_OK))
					progname = resolved;
			}
		}
		ready = true;
	}

	return progname;
}

static const char *interp_path(void) {
	static const char *interp;
	if(!interp) {
		const struct link_map *l = dlopen(NULL, RTLD_LAZY);
		const ElfW(Ehdr) *e = (ElfW(Ehdr) *) l->l_addr;
		const ElfW(Phdr) *ph = (ElfW(Phdr) *) (l->l_addr + e->e_phoff);
		const ElfW(Phdr) *pe = ph + e->e_phnum;
		const ElfW(Phdr) *p;
		for(p = ph; p->p_type != PT_INTERP; ++p)
			assert(p + 1 != pe);
		interp = (char *) (l->l_addr + p->p_vaddr);
	}
	return interp;
}

enum error handle_init(struct handle *h, const struct link_map *l, struct link_map *owner) {
	assert(h);
	assert(l);

	memset(h, 0, sizeof *h);

	h->path = l->l_name;
	if((!h->path || !*h->path) && !(h->path = progname()))
		return ERROR_FNAME_PATH;
	if(!strchr(h->path, '/') || !strcmp(h->path, interp_path()))
		// Do not attempt to operate on the vdso, whose dynamic section doesn't contain
		// valid pointers.  Skipping it is safe because it doesn't contain any whitelisted
		// symbols or any dynamic relocations (i.e., accesses to a different object file).
		//
		// Also do not attempt to operate on the dynamic linker itself, whose process image
		// copy is the subject of special mprotects() not recorded in its program header.
		// mprotect()s on itself that are not recorded in its program header.  Skipping it
		// is safe because it internally ensures there is only one copy of itself.
		return SUCCESS;

	h->baseaddr = l->l_addr;
	if(owner == l) {
		h->owned = true;
		if(owner == dlopen(NULL, RTLD_LAZY))
			// This object file is owned by the global scope.  We don't want to perform
			// redundant lookups in this false "local" scope, so forget about it.
			owner = NULL;
	}

	uint64_t flags = 0;
	uint64_t flags_1 = 0;
	const struct sym_hash *symhash = NULL;
	size_t njmpslots = 0;
	size_t nmiscrels = 0;
	for(const ElfW(Dyn) *d = l->l_ld; d->d_tag != DT_NULL; ++d)
		switch(d->d_tag) {
		case DT_FLAGS:
			flags = d->d_un.d_val;
			break;
		case DT_FLAGS_1:
			flags_1 = d->d_un.d_val;
			break;
		case DT_SYMTAB:
			h->symtab = (ElfW(Sym) *) d->d_un.d_ptr;
			break;
		case DT_HASH:
			symhash = (struct sym_hash *) d->d_un.d_ptr;
			break;
		case DT_JMPREL:
			h->jmpslots = (ElfW(Rela) *) d->d_un.d_ptr;
			break;
		case DT_PLTRELSZ:
			njmpslots = d->d_un.d_val;
			break;
		case DT_RELA:
			h->miscrels = (ElfW(Rela) *) d->d_un.d_ptr;
			break;
		case DT_RELASZ:
			nmiscrels = d->d_un.d_val;
			break;
		case DT_STRTAB:
			h->strtab = (char *) d->d_un.d_ptr;
			break;
		}
	assert(!(flags_1 & DF_1_NOOPEN) && "Dynamic section with unsupported NOOPEN flag");
	assert(h->symtab && "Dynamic section without SYMTAB entry");
	assert(h->strtab && "Dynamic section without STRTAB entry");

	// Use the symbol hash table to determine the size of the symbol table, if the former is
	// present.  Otherwise, employ the same heuristic used by GNU ld.so's dl-addr.c
	h->symtab_end = symhash ? h->symtab + symhash->nchain : (ElfW(Sym) *) h->strtab;
	if(h->jmpslots) {
		assert(njmpslots && "Dynamic section without PLTRELSZ entry");
		h->jmpslots_end = (ElfW(Rela) *) ((uintptr_t) h->jmpslots + njmpslots);
	}
	if(h->miscrels) {
		assert(nmiscrels && "Dynamic section without RELASZ entry");
		h->miscrels_end = (ElfW(Rela) *) ((uintptr_t) h->miscrels + nmiscrels);
	}

	const ElfW(Ehdr) *e = (ElfW(Ehdr) *) h->baseaddr;
	assert(!memcmp(e->e_ident, ELFMAG, SELFMAG) && "ELF header not loaded into process image");

	const ElfW(Phdr) *p = (ElfW(Phdr) *) (h->baseaddr + e->e_phoff);
	const ElfW(Phdr) *p_end = p + e->e_phnum;
	if(h->jmpslots) {
		assert(ELF64_R_TYPE(h->jmpslots->r_info) == R_X86_64_JUMP_SLOT &&
			"JMPREL table with non-JUMP_SLOT relocation entry");
		h->jmpslots_seg = segment_unwritable((uintptr_t) h->jmpslots - h->baseaddr,
			p, p_end);
		h->lazygot_seg = segment_unwritable(h->jmpslots->r_offset, p, p_end);
	}
	if(h->miscrels) {
		const ElfW(Rela) *globdat = NULL;
		for(const ElfW(Rela) *r = h->miscrels; r != h->miscrels_end; ++r)
			switch(ELF64_R_TYPE(r->r_info)) {
			case R_X86_64_GLOB_DAT:
				if(!globdat)
					globdat = r;
				break;
			case R_X86_64_COPY:
				// Because COPY relocations relocate the definition into the
				// process's shared BSS and the main executable may include
				// statically-resolved relocations to this new location, we cannot
				// move it again.  As such, we cannot intercept accesses from within
				// the executable, which always hit the caller's namespace, even
				// though they constitute accesses between different object files.
				//
				// For consistent semantics, compile the executable without COPY
				// relocations (e.g., using the -fpic compiler switch).
				fprintf(stderr,
					"%s: libgotcha warning: %s: found copy relocation: %s\n",
					progname(), h->path,
					h->strtab + h->symtab[ELF64_R_SYM(r->r_info)].st_name);
				break;
			}
		if(globdat)
			h->eagergot_seg = segment_unwritable(globdat->r_offset, p, p_end);
	}

	if(h->lazygot_seg || flags & DF_BIND_NOW || flags & DF_1_NOW)
		h->eager = true;

	h->tramps = malloc(((h->jmpslots_end - h->jmpslots) + (h->symtab_end - h->symtab)) *
		sizeof *h->tramps);
	if(!h->tramps)
		return ERROR_MALLOC;

	for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st)
		//  * Present in this object file.
		//  * Non-object data.  We don't want to hand a trampoline codepage to anyone who
		//    expects a global storage area.
		//  * Non-NULL.  Client code might NULL-check a function pointer before attempting
		//    to invoke it.
		//  * Non-duplicate.
		if(st->st_shndx != SHN_UNDEF && ELF64_ST_TYPE(st->st_info) != STT_OBJECT &&
			h->baseaddr + st->st_value &&
			globals_insert(h->baseaddr + st->st_value, 0))
			// Record our intention to install a trampoline over the eager GOT entry.
			h->tramps[h->ntramps++] = st - h->symtab;
	h->ntramps_symtab = h->ntramps;

	for(const ElfW(Rela) *r = h->jmpslots; r != h->jmpslots_end; ++r) {
		const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);

		// Does this object file define an instance of the target symbol?  If so, there's a
		// chance this call would lazily resolve to the local definition; in this case, the
		// semantics should be the same as if the definition weren't exported as a dynamic
		// symbol.  In order to distinguish whether this is a call between two different
		// object files, we need to force the call to resolve now.
		if(st->st_shndx != SHN_UNDEF) {
			uintptr_t *gotent = (uintptr_t *) (h->baseaddr + r->r_offset);
			if(*gotent == h->baseaddr + st->st_value)
				// The call has already been resolved to the local definition.  Skip
				// this relocation entry so we will not install a trampoline.
				continue;

			if(!h->eager && segment(*gotent, p, p_end)) {
				// The lazy GOT is writable and its corresponding entry points
				// somewhere in this same object file.  We already know that isn't
				// the local definition, so it must be a PLT trampoline and the
				// call must not yet have been resolved.
				if(flags & DF_SYMBOLIC) {
					// This object file specifies that it should be placed
					// ahead of the global search scope, so the call is
					// guaranteed to bind to the local definition.  Resolve it
					// eagerly to save the dynamic linker work later on, then
					// skip the relocation entry.
					*gotent = h->baseaddr + st->st_value;
					continue;
				}

				const char *sym = h->strtab + st->st_name;
				dlerror();

				// Try to resolve the symbol in the global scope.  Because we
				// ourselves cannot have been dlopen()'d, NULL here means that no
				// dlopen()'d object files will be searched.
				uintptr_t res = (uintptr_t) dlsym(NULL, sym);
				if(dlerror()) {
					// We couldn't find a definition in the global scope.  If
					// this object file was loaded at run-time instead of
					// load-time, we also need to search the dependency chain of
					// object files that were loaded by the dlopen() call that
					// brought it in.
					if(owner)
						res = (uintptr_t) dlsym(owner, sym);

					if(!owner || dlerror()) {
						// We couldn't find a definition anywhere in the
						// process image.  Hopefully no one ever invokes the
						// lazy call, because it would fail to resolve!
						// Skip this relocation entry.
						fprintf(stderr,
							"%s: libgotcha warning: %s: unresolvable lazy call: %s\n",
							progname(), h->path, sym);
						continue;
					}
				}

				// We just did a lot of work to eagerly resolve that call!  Save the result.
				*gotent = res;

				if(res == h->baseaddr + st->st_value)
					// We resolved the call to the local definition.  Skip this entry.
					continue;
			}
		}

		// The call is guaranteed not to resolve to a definition within this same object
		// file; record our intention to install a trampoline over the lazy GOT entry.
		h->tramps[h->ntramps++] = r - h->jmpslots;
	}

	if((h->tramps = realloc(h->tramps, h->ntramps * sizeof *h->tramps))) {
		if(!(h->shadow = calloc(1, sizeof *h->shadow))) {
			free(h->tramps);
			return ERROR_MALLOC;
		}
		h->shadow->override_table = -1;
		h->shadow->first_entry = -1;

		if(!(h->shadow->plot = plot_insert_lib(h))) {
			free(h->shadow);
			free(h->tramps);
			return ERROR_LIB_SIZE;
		}
	}

	for(unsigned tramp = 0; tramp < h->ntramps_symtab; ++tramp) {
		const ElfW(Sym) *st = h->symtab + h->tramps[tramp];
		uintptr_t defn = h->baseaddr + st->st_value;
		uintptr_t repl = (uintptr_t) h->shadow->plot->code + plot_entries_offset +
			(h->shadow->first_entry + tramp) * plot_entry_size;

		// Any time we see a GLOB_DAT relocation from another object file targeted against
		// this definition, we'll want to retarget it at this PLOT trampoline.
		globals_set(defn, repl);
	}
	assert(!globals_contains(0));

	return SUCCESS;
}

void handle_cleanup(struct handle *h) {
	if(h->owned)
		dlclose(h);

	free(h->tramps);
	h->tramps = NULL;
}

// Setup "stubbed" shadow GOTs for the ancillary namespaces.  For use only on object files for which
// *all* defined symbols are whitelisted and we will therefore never execute the copies in ancillary
// namespaces, if they even exist at all.
static inline void handle_got_whitelist_all(struct handle *h) {
	if(!h->shadow)
		return;

	size_t len = handle_got_num_entries(h);
	uintptr_t *proxy = *h->shadow->gots + len;
	memset(proxy, 0, len);
	for(size_t namespace = 1; namespace <= NUM_SHADOW_NAMESPACES; ++namespace)
		h->shadow->gots[namespace] = proxy;

	// We must add this to the whitelist so that any lazily-resolved
	// calls from other object files also proxy to the base namespace.
	// It's fine to do it here because we won't handle_got_shadow() the
	// subsequent object files in the search list (i.e., the only ones
	// that could be interposed) until we're finished with this one.
	whitelist_so_insert(h);
}

// Setup full shadow GOTs for the ancillary namespaces.
static inline void handle_got_shadow_init(struct handle *h, Lmid_t n, uintptr_t base) {
	assert(n <= NUM_SHADOW_NAMESPACES);

	prot_segment(base, h->eagergot_seg, PROT_WRITE);
	for(const ElfW(Rela) *r = h->miscrels; r != h->miscrels_end; ++r)
		if(ELF64_R_TYPE(r->r_info) == R_X86_64_GLOB_DAT) {
			// Notice we *always* compute this address relative to the base namespace.
			uintptr_t *defn = (uintptr_t *) (h->baseaddr + r->r_offset);
			uintptr_t *got = (uintptr_t *) (base + r->r_offset);
			uintptr_t tramp = globals_get(*defn);
			if(tramp)
				// Install the corresponding PLOT trampoline over the GOT entry.
				*got = tramp;
		}
	prot_segment(base, h->eagergot_seg, 0);

	if(!h->shadow)
		return;

	for(unsigned tramp = 0; tramp < h->ntramps_symtab; ++tramp) {
		const ElfW(Sym) *st = h->symtab + h->tramps[tramp];
		uintptr_t *sgot = h->shadow->gots[n] + tramp;

		if(n && whitelist_shared_contains(h->strtab + st->st_name))
			// This symbol is shared among all namespaces.  Populate the corresponding
			// entry in the ancillary namespace's shadow GOT with a NULL sentinel to
			// indicate to the main PLOT trampoline that it should load the address from
			// the base shadow GOT instead.
			*sgot = 0;
		else
			// There exists a separate definition for each namespace.  Copy the current
			// GOT entry (which contains the address of the eagerly resolved definition)
			// into the shadow GOT.
			*sgot = base + st->st_value;
	}

	prot_segment(base, h->lazygot_seg, PROT_WRITE);
	if(!h->eager)
		// Some bindings might be resolved lazily.  Ordinarily this would cause the dynamic
		// linker to overwrite the lazy GOT entry, thereby memoizing the resolved symbol;
		// however, this would overwrite our custom trampoline.  We'll need to modify the
		// relocation table to convince it to update shadow GOT entries instead.
		prot_segment(base, h->jmpslots_seg, PROT_WRITE);
	for(unsigned tramp = h->ntramps_symtab; tramp < h->ntramps; ++tramp) {
		const ElfW(Rela) *r = h->jmpslots + h->tramps[tramp];
		const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);
		uintptr_t *got = (uintptr_t *) (base + r->r_offset);
		uintptr_t *sgot = h->shadow->gots[n] + tramp;

		if(n && whitelist_shared_contains(h->strtab + st->st_name))
			// Place a NULL sentinel in the shadow GOT.
			*sgot = 0;
		else
			// Copy the current GOT entry (which contains the address of either the
			// resolved definition or a PLT trampoline) into the shadow GOT.
			*sgot = *got;

		if(!h->eager)
			// Instruct the dynamic linker to update the *shadow* GOT entry if the PLT
			// trampoline is later invoked.
			((ElfW(Rela) *) r)->r_offset = (uintptr_t) sgot - base;

		// Install the corresponding PLOT trampoline over the GOT entry.
		*got = (uintptr_t) h->shadow->plot->code + plot_entries_offset +
			(h->shadow->first_entry + tramp) * plot_entry_size;
	}
	prot_segment(base, h->lazygot_seg, 0);
	if(!h->eager)
		prot_segment(base, h->jmpslots_seg, 0);
}

enum error handle_got_shadow(struct handle *h) {
	assert(h);

	if(!strchr(h->path, '/') || !strcmp(h->path, interp_path()))
		// Do not attempt to operate on the vdso or the dynamic linker itself.
		return SUCCESS;

	size_t len = handle_got_num_entries(h);
	if(h->shadow) {
		*h->shadow->gots = malloc((NUM_SHADOW_NAMESPACES + 1) * len * sizeof **h->shadow->gots);
		if(!*h->shadow->gots)
			return ERROR_MALLOC;
	}

	dlm_t open = h->owned ? (dlm_t) namespace_load : namespace_get;
	handle_got_shadow_init(h, LM_ID_BASE, h->baseaddr);
	for(size_t namespace = 1; namespace <= NUM_SHADOW_NAMESPACES; ++namespace) {
		const struct link_map *l = open(namespace, h->path, RTLD_LAZY);
		if(!l) {
			// The dynamic linker does not consider preloaded object files to be
			// dependencies, so although we see them as not owned, they (and *their*
			// dependencies) are absent from ancillary namespaces.  Since such object
			// files usually correspond to debugging or other development tools rather
			// than production software, "partially" whitelist them.  Note that the
			// resulting interposition differ slightly based on the namespace of the
			// "client" (victim?) code:
			//  * Code executing on a page located in the base namespace *is*
			//    interposed by the preloaded object(s).  If the current namespace is an
			//    ancillary one, any calls so interposed are proxied to the base
			//    namespace as with normal whitelisted symbols.
			//  * Code executing on a page located in an ancillary namespace is not
			//    interposed by the preloaded object(s) because they don't even appear
			//    in said namespaces' search scopes.
			assert(getenv("LD_PRELOAD") && "Phantom dependency not from LD_PRELOAD");
			handle_got_whitelist_all(h);
			return SUCCESS;
		}

		if(h->shadow)
			h->shadow->gots[namespace] = *h->shadow->gots + len * namespace;
		handle_got_shadow_init(h, namespace, l->l_addr);
	}
	return SUCCESS;
}
