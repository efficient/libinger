#include "handle.h"

#include "config.h"
#include "goot.h"
#include "plot.h"
#include "segprot.h"
#include "whitelist.h"

#include <sys/auxv.h>
#include <sys/stat.h>
#include <assert.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define RET 0xc3

typedef struct link_map *(*dlm_t)(Lmid_t, const char *, int);

static char nodelete[] = "/tmp/libgotcha-XXXXXX";

bool trampolines_insert(uintptr_t, uintptr_t);
bool trampolines_contains(uintptr_t);
uintptr_t trampolines_get(uintptr_t);
void trampolines_set(uintptr_t, uintptr_t);
bool trampolines_remove(uintptr_t);

struct sym_hash {
	uint32_t nbucket;
	uint32_t nchain;
	uint32_t indices[];
};

static inline void plot(const struct plot **page, size_t *entry,
	const struct handle *h, size_t index) {
	size_t p = index / PLOT_ENTRIES_PER_PAGE;
	size_t e = index % PLOT_ENTRIES_PER_PAGE;
	*page = h->plots[p];
	*entry = e;
	if((*page)->goot->identifier == h->shadow.override_table)
		*entry += h->shadow.first_entry;
}

static inline uintptr_t plot_trampoline(const struct handle *h, size_t index) {
	const struct plot *page;
	size_t entry;
	plot(&page, &entry, h, index);

	return (uintptr_t) page->code + plot_entries_offset + entry * plot_entry_size;
}

static inline uintptr_t plot_trap(const struct handle *h, size_t index) {
	const struct plot *page;
	size_t entry;
	plot(&page, &entry, h, index);

	return (uintptr_t) page + plot_pagesize() + entry;
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

static enum error nodelete_clear_flag(struct handle *h) {
	if(!strcmp(nodelete + sizeof nodelete - 7, "XXXXXX"))
		mkdtemp(nodelete);

	int tagged = open(h->path, O_RDONLY);
	if(tagged < 0)
		return ERROR_OPEN;

	const char *basename = strrchr(h->path, '/');
	h->path = malloc(sizeof nodelete + strlen(basename));
	if(!h->path) {
		close(tagged);
		return ERROR_MALLOC;
	}
	sprintf(h->path, "%s%s", nodelete, basename);

	int untagged = open(h->path, O_CREAT | O_EXCL | O_RDWR, 0666);
	if(untagged < 0) {
		close(tagged);
		free(h->path);
		return ERROR_OPEN;
	}

	struct stat st;
	fstat(tagged, &st);
	ftruncate(untagged, st.st_size);

	ElfW(Ehdr) *e = mmap(NULL, st.st_size, PROT_READ | PROT_WRITE, MAP_SHARED, untagged, 0);
	close(untagged);
	if(e == MAP_FAILED) {
		close(tagged);
		free(h->path);
		return ERROR_MMAP;
	}

	for(size_t remaining = st.st_size; remaining; remaining -= read(tagged, e, remaining));
	close(tagged);

	const ElfW(Phdr) *p;
	for(p = (ElfW(Phdr) *) ((uintptr_t) e + e->e_phoff); p->p_type != PT_DYNAMIC; ++p);

	uint64_t *flags_1 = NULL;
	uintptr_t *init = NULL;
	ElfW(Dyn) *d;
	for(d = (ElfW(Dyn) *) ((uintptr_t) e + p->p_offset); d->d_tag != DT_NULL; ++d)
		switch(d->d_tag) {
		case DT_FLAGS_1:
			flags_1 = &d->d_un.d_ptr;
			break;
		case DT_INIT:
			init = &d->d_un.d_ptr;
			break;
		}

	// Clear the NODELETE flag to *allow* unloading ancillary copies of this object.
	*flags_1 &= ~DF_1_NODELETE;

	if(h->ldaccess) {
		// This object file satisfies all of:
		//  * Is marked NODELETE, indicating it "cannot" be unloaded
		//  * Accesses ld.so's internal mutable _rtld_global structure
		//  * has a legacy constructor, a la _init()
		// Such objects (e.g., libpthread.so) might monkey patch the dynamic linker by
		// overwriting its function pointers with their own.  We don't want them to create a
		// dependency between the dynamic linker and an ancillary namespace, so disable
		// their legacy constructors.
		assert(init);

		uint8_t *ret;
		for(ret = (uint8_t *) (uintptr_t) h->ldaccess; *ret != RET; ++ret);
		*init += (uintptr_t) ret - (uintptr_t) h->ldaccess;
	}

	munmap(e, st.st_size);
	return SUCCESS;
}

const char *handle_progname(void) {
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

enum error handle_init(struct handle *h, const struct link_map *l, struct link_map *owner) {
	assert(h);
	assert(l);

	memset(h, 0, sizeof *h);

	h->path = l->l_name;
	if((!h->path || !*h->path) && !(h->path = (char *) handle_progname()))
		return ERROR_FNAME_PATH;

	h->baseaddr = l->l_addr;
	h->vdso = h->baseaddr == getauxval(AT_SYSINFO_EHDR);
	if(owner == l) {
		h->owned = true;
		if(owner == dlopen(NULL, RTLD_LAZY))
			// This object file is owned by the global scope.  We don't want to perform
			// redundant lookups in this false "local" scope, so forget about it.
			owner = NULL;
	}

	uint64_t flags = 0;
	uint64_t flags_1 = 0;
	size_t soname = 0;
	const struct sym_hash *symhash = NULL;
	size_t njmpslots = 0;
	size_t nmiscrels = 0;
	uintptr_t init = 0;
	for(const ElfW(Dyn) *d = l->l_ld; d->d_tag != DT_NULL; ++d)
		switch(d->d_tag) {
		case DT_FLAGS:
			flags = d->d_un.d_val;
			break;
		case DT_FLAGS_1:
			flags_1 = d->d_un.d_val;
			break;
		case DT_SONAME:
			soname = d->d_un.d_val;
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
		case DT_INIT:
			init = d->d_un.d_val;
			break;
		}
	assert(!(flags_1 & DF_1_NOOPEN) && "Dynamic section with unsupported NOOPEN flag");
	assert(h->symtab && "Dynamic section without SYMTAB entry");
	assert(h->strtab && "Dynamic section without STRTAB entry");

	if(h->vdso) {
		// The vdso's dynamic section doesn't get relocated like other object files', so do
		// that manually here.
		h->symtab = (ElfW(Sym) *) (h->baseaddr + (uintptr_t) h->symtab);
		symhash = (struct sym_hash *) (h->baseaddr + (uintptr_t) symhash);
		h->strtab = (char *) (h->baseaddr + (uintptr_t) h->strtab);
	}

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
	h->phdr = (ElfW(Phdr) *) (h->baseaddr + e->e_phoff);
	h->phdr_end = h->phdr + e->e_phnum;
	for(const ElfW(Phdr) *p = h->phdr; p != h->phdr_end; ++p)
		if(p->p_type == PT_TLS) {
			h->tls = p;
			break;
		}

	if(h->jmpslots) {
		assert(ELF64_R_TYPE(h->jmpslots->r_info) == R_X86_64_JUMP_SLOT &&
			"JMPREL table with non-JUMP_SLOT relocation entry");
		h->jmpslots_seg = segment_unwritable((uintptr_t) h->jmpslots - h->baseaddr,
			h->phdr, h->phdr_end);
		h->lazygot_seg = segment_unwritable(h->jmpslots->r_offset, h->phdr, h->phdr_end);
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
					"%s: libgotcha warning: found copy relocation: %s\n",
					handle_progname(),
					h->strtab + h->symtab[ELF64_R_SYM(r->r_info)].st_name);
				break;
			}
		if(globdat)
			h->eagergot_seg = segment_unwritable(globdat->r_offset,
				h->phdr, h->phdr_end);
	}

	if(h->lazygot_seg || flags & DF_BIND_NOW || flags & DF_1_NOW)
		h->eager = true;

	if(soname) {
		const char *sopath = strrchr(h->path, '/');
		sopath = sopath ? sopath + 1 : h->path;
		h->sonamed = !strcmp(h->strtab + soname, sopath);
	}

	bool partial = whitelist_so_partial(h->path);

	size_t ntramps_guess = (h->symtab_end - h->symtab) + (h->jmpslots_end - h->jmpslots);
	if(ntramps_guess && !(h->tramps = malloc(ntramps_guess * sizeof *h->tramps)))
		return ERROR_MALLOC;

	for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st) {
		// For each symbol describing sommething that meets all these criteria:
		//  * Present in this object file.
		//  * Non-NULL.  Client code might NULL-check a pointer before attempting to
		//    use it.
		//  * Non-duplicate.
		if((partial || st->st_shndx != SHN_UNDEF) && h->baseaddr + st->st_value &&
			trampolines_insert(h->baseaddr + st->st_value, h->ntramps))
			// Record our intention to multiplex accesses through the eager GOT entry.
			h->tramps[h->ntramps++] = st - h->symtab;

		if(!h->ldaccess && !strcmp(h->strtab + st->st_name, "_rtld_global")) {
			// This object file accesses the dynamic linker's mutable global storage.
			if(!whitelist_so_contains(h->path) && strcmp(h->path, interp_path()))
				fprintf(stderr,
					"%s: libgotcha warning: %s: unwhitelisted GL() access\n",
					handle_progname(), h->path);
			if(init && st->st_shndx == SHN_UNDEF && flags_1 & DF_1_NODELETE)
				// This object is flagged to prevent it from ever being destructed.
				// We'll assume its constructor might modify the linker's mutable
				// global storage, causing ancillary namespace state to leak into
				// the base one.
				h->ldaccess = (void (*)(void)) (h->baseaddr + init);
		}
	}
	for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st)
		// Note that symbols that are the subject of COPY relocations are considered to be
		// in the executable rather than the object file in which they are
		// logically/programmatically defined.  These unexpected semantics may be difficult
		// to reason about.  We output a warning whenever we encounter such a relocation,
		// though, so the user has been warned.
		if((partial || st->st_shndx != SHN_UNDEF) &&
			ELF64_ST_TYPE(st->st_info) == STT_OBJECT) {
			size_t tramp = trampolines_get(h->baseaddr + st->st_value);
			const ElfW(Sym) *ol = &h->symtab[h->tramps[tramp]];
			if(ol->st_value == st->st_value) {
				if(config_noglobals() ||
					segment_unwritable(st->st_value, h->phdr, h->phdr_end)) {
					// The symbol is read-only, so we'll assume it is going to
					// match across copies of this object file.  Forget about
					// it, annulling the request to multiplex accesses.
					if(--h->ntramps) {
						size_t last = h->tramps[h->ntramps];
						h->tramps[tramp] = last;
						trampolines_set(
							h->baseaddr + h->symtab[last].st_value,
							tramp);
					}
					trampolines_remove(h->baseaddr + st->st_value);
				} else if(ELF64_ST_TYPE(ol->st_info) != STT_OBJECT)
					// The non-object symbol clashes with an *object* symbol.
					// "Promote" the original symbol to this one so we'll
					// install a global access placeholder rather than an
					// executable trampoline.
					h->tramps[tramp] = st - h->symtab;
			}
		}
	h->ntramps_symtab = h->ntramps;

	for(const ElfW(Rela) *r = h->jmpslots; r != h->jmpslots_end; ++r) {
		const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);

		// This is most likely an UND symbol, in which case it cannot be an
		// indirectly-resolved function.  In the unlikely event it's a *local*
		// indirectly-resolved function that might also be locally referenced, abort because
		// we don't currently bother to reason about that case.
		assert(ELF64_ST_TYPE(st->st_info) != STT_GNU_IFUNC);

		// Does this object file define an instance of the target symbol?  If so, there's a
		// chance this call would lazily resolve to the local definition; in this case, the
		// semantics should be the same as if the definition weren't exported as a dynamic
		// symbol.  In order to distinguish whether this is a call between two different
		// object files, we need to force the call to resolve now.
		if(partial || st->st_shndx != SHN_UNDEF) {
			uintptr_t *gotent = (uintptr_t *) (h->baseaddr + r->r_offset);
			if(*gotent == h->baseaddr + st->st_value)
				// The call has already been resolved to the local definition.  Skip
				// this relocation entry so we will not install a trampoline.
				continue;

			if(!h->eager && segment(*gotent, h->phdr, h->phdr_end)) {
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
							handle_progname(), h->path, sym);
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

	h->shadow.override_table = -1;
	h->shadow.first_entry = -1;
	if((h->tramps = realloc(h->tramps, h->ntramps * sizeof *h->tramps))) {
		if(h->ntramps_symtab &&
			!(*h->shadow.gots = malloc(h->ntramps_symtab * sizeof **h->shadow.gots))) {
			free(h->tramps);
			return ERROR_MALLOC;
		}

		plot_insert_lib(h);
		assert(h->plots);

		size_t adjustment = 0;
		for(struct plot *const *plot = h->plots;
			(*plot)->goot->identifier != h->shadow.override_table;
			++plot) {
			(*plot)->goot->adjustment = adjustment;
			adjustment += PLOT_ENTRIES_PER_PAGE;
		}
		h->shadow.last_adjustment = adjustment;
	}

	for(unsigned tramp = 0; tramp < h->ntramps_symtab; ++tramp) {
		const ElfW(Sym) *st = h->symtab + h->tramps[tramp];
		uintptr_t *sgot = *h->shadow.gots + tramp;
		uintptr_t defn = h->baseaddr + st->st_value;

		*sgot = defn;
		if(ELF64_ST_TYPE(st->st_info) == STT_GNU_IFUNC) {
			uintptr_t (*resolver)(void) = (uintptr_t (*)(void)) defn;
			*sgot = resolver();
		}

		// If this is code, we'll be replacing it with an executable PLOT trampoline;
		// otherwise, we'll instead use an inaccessible memory location to raise a fault.
		uintptr_t repl;
		if(ELF64_ST_TYPE(st->st_info) != STT_OBJECT)
			repl = plot_trampoline(h, tramp);
		else
			repl = plot_trap(h, tramp);
		assert(repl);

		// Any time we see a GLOB_DAT relocation from another object file targeted against
		// this definition, we'll want to retarget it at the chosen replacement.
		if(*sgot == defn)
			trampolines_set(defn, repl);
		else {
			trampolines_remove(defn);
			trampolines_insert(*sgot, repl);
		}
	}
	assert(!trampolines_contains(0));

	if(flags_1 & DF_1_NODELETE) {
		enum error code = nodelete_clear_flag(h);
		if(code != SUCCESS)
			return code;
	}

	return SUCCESS;
}

void handle_cleanup(struct handle *h) {
	if(h->owned)
		dlclose(h);

	for(const size_t *tramp = h->tramps; tramp != h->tramps + h->ntramps_symtab; ++tramp) {
		const ElfW(Sym) *st = h->symtab + *tramp;
		trampolines_remove(h->baseaddr + st->st_value);
	}
	free(h->tramps);
	h->tramps = NULL;

	plot_remove_lib(h);
	assert(!h->plots);

	free(*h->shadow.gots);
	memset(h->shadow.gots, 0, sizeof h->shadow.gots);

	for(size_t seg = 0; seg < h->nrdwrs; ++seg)
		for(ssize_t ns = 0; ns < config_numgroups(); ++ns)
			free(h->rdwrs[seg].addrs_stored[ns]);
	free(h->rdwrs);
	h->rdwrs = NULL;
	h->nrdwrs = 0;
}

static inline bool myself(const struct handle *h) {
	return h->baseaddr == namespace_self()->l_addr;
}

// If the provided symbol is one for which we force interposition, return the address of its
// trampoline; otherwise, return fallback.
static inline uintptr_t got_trampoline(const char *sym, uintptr_t fallback) {
	intptr_t interposed = whitelist_shared_get(sym);
	return interposed != -1 && interposed ? trampolines_get(interposed) : fallback;
}

// Setup "stubbed" shadow GOTs for the ancillary namespaces.  For use only on object files for which
// *all* defined symbols are whitelisted and we will therefore never execute the copies in ancillary
// namespaces, if they even exist at all.
static inline void handle_got_whitelist_all(struct handle *h) {
	if(!*h->shadow.gots)
		return;

	size_t len = handle_got_num_entries(h);
	uintptr_t *proxy = *h->shadow.gots + len;
	memset(proxy, 0, len * sizeof *proxy);
	for(Lmid_t namespace = 1; namespace <= config_numgroups(); ++namespace)
		h->shadow.gots[namespace] = proxy;

	if(myself(h))
		return;

	// Look for JUMP_SLOT relocations against symbols we need to interpose with our own.
	prot_segment(h->baseaddr, h->lazygot_seg, PROT_WRITE);
	for(unsigned tramp = h->ntramps_symtab; tramp < h->ntramps; ++tramp) {
		const ElfW(Rela) *r = h->jmpslots + h->tramps[tramp];
		const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);
		uintptr_t interposed = got_trampoline(h->strtab + st->st_name, 0);
		if(interposed)
			// Substitute our own trampoline over the GOT entry.
			*(uintptr_t *) (h->baseaddr + r->r_offset) = interposed;
	}
	prot_segment(h->baseaddr, h->lazygot_seg, 0);

	// We must add this to the whitelist so that any lazily-resolved
	// calls from other object files also proxy to the base namespace.
	// It's fine to do it here because we won't handle_got_shadow() the
	// subsequent object files in the search list (i.e., the only ones
	// that could be interposed) until we're finished with this one.
	whitelist_so_insert(h);
}

// If we're processing an ancillary namespace and this symbol is shared among all namespaces, return
// a NULL sentinel to indicate that the main PLOT trampoline should perform a switch to the main
// namespace; otherwise, return defn.
static inline uintptr_t sgot_entry(const char *sym, Lmid_t n, uintptr_t defn) {
	return n && whitelist_shared_get(sym) != -1 ? 0 : defn;
}

// Setup full shadow GOTs for the ancillary namespaces.
static inline void handle_got_shadow_init(const struct handle *h, Lmid_t n, uintptr_t base) {
	assert(n <= config_numgroups());

	bool self = myself(h);
	bool partial = whitelist_so_partial(h->path);

	// First, update ancillary namespaces' shadow GOT entries for symbols defined in this same
	// object.  These entries are shared among all GLOB_DAT relocations throughout the entire
	// application in order to preserve pointer-comparison semantics.  They must be populated
	// before we alter the GOT entries corresponding to local such relocations, which might
	// occur within IFUNC relocations' resolver functions!
	if(*h->shadow.gots && n)
		for(unsigned tramp = 0; tramp < h->ntramps_symtab; ++tramp) {
			const ElfW(Sym) *st = h->symtab + h->tramps[tramp];
			uintptr_t *sgot = h->shadow.gots[n] + tramp;
			uintptr_t defn = base + st->st_value;

			if(ELF64_ST_TYPE(st->st_info) == STT_GNU_IFUNC) {
				uintptr_t (*resolver)(void) = (uintptr_t (*)(void)) defn;
				defn = resolver();
			}

			// No symbol's shadow GOT entry can be the same as its GOT entry; otherwise,
			// any attempt to call it will result in infinite recursion!
			assert(defn != trampolines_get(h->shadow.gots[0][tramp]));

			// Populate the shadow GOT entry.  If we're multiplexing this symbol, use
			// the address of this ancillary namespace's own definition.
			*sgot = sgot_entry(h->strtab + st->st_name, n, defn);
		}

	// Next, overwrite the GOT entries for GLOB_DAT relocations with trampolines or traps.  Skip
	// this entire step for *this* library, because we don't want to force any automatic
	// namespace switches once our own code is already running.
	if(!self) {
		prot_segment(base, h->eagergot_seg, PROT_WRITE);
		for(const ElfW(Rela) *r = h->miscrels; r != h->miscrels_end; ++r)
			if(ELF64_R_TYPE(r->r_info) == R_X86_64_GLOB_DAT) {
				const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);
				uintptr_t *got = (uintptr_t *) (base + r->r_offset);

				// We already know this symbol resolves to a different object file;
				// therefore, its symbol table entry is an UND and cannot describe an
				// indirectly-resolved function.
				assert(ELF64_ST_TYPE(st->st_info) != STT_GNU_IFUNC);

				bool redirected = false;
				if(*got && (*got != base + st->st_value || (redirected = partial))) {
					// This is not an undefined weak symbol, and the reference didn't
					// resolve back to our own object file.  Let's look up the address
					// of its program-wide multiplexing address...
					uintptr_t tramp;
					if(!n) {
						// Retrieve the trampoline from the defining library, or
						// from *this* library if it's an interposed symbol.  But
						// never do the latter for a partially-whitelisted library's
						// relocation unless we would have processed it were the
						// library fully whitelisted.
						uintptr_t uninterposed = trampolines_get(*got);
						tramp = redirected ? uninterposed :
							got_trampoline(h->strtab + st->st_name, uninterposed);

						h->globdats[r - h->miscrels] = tramp;
					} else {
						tramp = h->globdats[r - h->miscrels];
					}

					if(tramp && (!redirected || n))
						// ...and install it over the GOT entry.
						*got = tramp;
				}
			}
		prot_segment(base, h->eagergot_seg, 0);
	}

	if(!*h->shadow.gots)
		return;

	// Finally, update the GOT and shadow GOT entries corresponding to JUMP_SLOT relocations.
	const ElfW(Rela) *jmpslots = (ElfW(Rela) *) ((uintptr_t) h->jmpslots - h->baseaddr + base);
	prot_segment(base, h->lazygot_seg, PROT_WRITE);
	if(!h->eager)
		// Some bindings might be resolved lazily.  Ordinarily this would cause the dynamic
		// linker to overwrite the lazy GOT entry, thereby memoizing the resolved symbol;
		// however, this would overwrite our custom trampoline.  We'll need to modify the
		// relocation table to convince it to update shadow GOT entries instead.
		prot_segment(base, h->jmpslots_seg, PROT_WRITE);
	for(unsigned tramp = h->ntramps_symtab; tramp < h->ntramps; ++tramp) {
		const ElfW(Rela) *r = jmpslots + h->tramps[tramp];
		const ElfW(Sym) *st = h->symtab + ELF64_R_SYM(r->r_info);
		const char *sym = h->strtab + st->st_name;
		uintptr_t *got = (uintptr_t *) (base + r->r_offset);
		uintptr_t *sgot = h->shadow.gots[n] + tramp;

		// We already know this symbol resolves to a different object file; therefore, its
		// symbol table entry is an UND and cannot describe an indirectly-resolved function.
		assert(ELF64_ST_TYPE(st->st_info) != STT_GNU_IFUNC);

		bool redirected = false;
		if(*got != base + st->st_value || ((redirected = partial) && n)) {
			// Populate the shadow GOT entry.  If we're multiplexing this symbol, use
			// the current GOT entry (which contains the address of either the resolved
			// definition or a PLT trampoline).
			*sgot = sgot_entry(sym, n, *got);

			if(!h->eager)
				// Instruct the dynamic linker to update the *shadow* GOT entry if
				// the PLT trampoline is later invoked.
				((ElfW(Rela) *) r)->r_offset = (uintptr_t) sgot - base;

			// Skip updating the GOT for *this* library, because we don't want to force
			// any automatic namespace switches once our own code is already running.
			if(!self) {
				// Install our corresponding PLOT trampoline over the GOT entry.  Or reject
				// their reality and substitute the one from *this* library if it's an
				// interposed symbol.  But never do the latter for a partially-whitelisted
				// library's relocation unless we would have processed it were the library
				// fully whitelisted.
				uintptr_t uninterposed = plot_trampoline(h, tramp);
				*got = redirected ? uninterposed : got_trampoline(sym, uninterposed);
			}
		}
	}
	prot_segment(base, h->lazygot_seg, 0);
	if(!h->eager)
		prot_segment(base, h->jmpslots_seg, 0);
}

static inline bool handle_got_reshadow(const struct handle *h, Lmid_t n, const struct link_map **m) {
	if(!handle_is_get_safe(h))
		return true;

	dlm_t open = h->owned ? (dlm_t) dlmopen : namespace_get;
	const struct link_map *l = open(n, h->path, RTLD_LAZY);
	if(!l)
		return false;

	handle_got_shadow_init(h, n, l->l_addr);

	if(m)
		*m = l;
	return true;
}

enum error handle_got_shadow(struct handle *h) {
	assert(h);

	if(h->vdso) {
		// Do not attempt to operate on the vdso, which isn't recognized by dlopen() and
		// shouldn't be multiplexed anyway.  This is safe because all its functions are
		// reentrant and it does not contain any dynamic relocations.  We must, however,
		// "install" the base namespace's shadow GOT into every ancillary namespace so that
		// the trampoline doesn't crash and knows not to switch namespaces on inbound calls.
		assert(!h->jmpslots);
		assert(!h->miscrels);
		for(Lmid_t namespace = 1; namespace <= config_numgroups(); ++namespace)
			h->shadow.gots[namespace] = h->shadow.gots[LM_ID_BASE];
		return SUCCESS;
	}

	size_t len = handle_got_num_entries(h);
	if(len) {
		*h->shadow.gots = realloc(*h->shadow.gots,
			(config_numgroups() + 1) * len * sizeof **h->shadow.gots);
		if(!*h->shadow.gots)
			return ERROR_MALLOC;
	}

	if(!strcmp(h->path, interp_path())) {
		// Do not attempt to operate on the dynamic linker itself, which is subjected to
		// special mprotect()s not recorded in its program header.  There is really only one
		// copy of it loaded, so skipping it is safe---provided we trampoline all inbound
		// calls to the base namespace---because its outbound calls will always be invoked
		// from the base namespace, and therefore serviced in it as well.
		handle_got_whitelist_all(h);
		return SUCCESS;
	}

	h->globdats = malloc((h->miscrels_end - h->miscrels) * sizeof *h->globdats);
	if(!h->globdats)
		return ERROR_MALLOC;

	struct restore rdwrs[h->phdr_end - h->phdr];
	size_t nrdwrs = 0;
	const ElfW(Phdr) *relro = NULL;
	for(const ElfW(Phdr) *p = h->phdr; p != h->phdr_end; ++p)
		if(p->p_type == PT_GNU_RELRO) {
			relro = p;
			break;
		}
	for(const ElfW(Phdr) *p = h->phdr; p != h->phdr_end; ++p)
		if(p->p_type == PT_LOAD && p->p_flags & PF_W) {
			uintptr_t offset = p->p_vaddr;
			size_t size = p->p_memsz;
			if(relro) {
				uintptr_t relend = relro->p_vaddr + relro->p_memsz;
				if(relro->p_vaddr <= offset && offset + size <= relend)
					continue;
				else if(offset < relro->p_vaddr && relro->p_vaddr < offset + size) {
					assert(offset + size <= relend && "split segment!");
					size = relro->p_vaddr - offset;
				} else if(offset < relend && relend < offset + size) {
					size -= relend - offset;
					offset = relend;
				}
			}

			struct restore *rdwr = rdwrs + nrdwrs++;
			rdwr->size = size;
			rdwr->off_loaded = offset;
			memset(rdwr->addrs_stored, 0, sizeof rdwr->addrs_stored);
		}

	handle_got_shadow_init(h, LM_ID_BASE, h->baseaddr);
	for(Lmid_t namespace = 1; namespace <= config_numgroups(); ++namespace) {
		if(len)
			h->shadow.gots[namespace] = *h->shadow.gots + len * namespace;

		const struct link_map *l = NULL;
		if(!handle_got_reshadow(h, namespace, &l)) {
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
			//
			// Note that, although such object files cannot possibly be in ancillary
			// namespaces' search scopes, it's still important that we zero-fill their
			// ancillary shadow tables in case the program passes the addresses of their
			// symbols (thanks to us, really their trampolines) to said namespaces:
			// without doing this, calls would result in undefined behavior, but after
			// this step, they instead result in a normal namespace switch.
			free(h->globdats);
			h->globdats = NULL;

			if(h->owned)
				return ERROR_DLMOPEN;
			else if(!getenv("LD_PRELOAD"))
				return ERROR_RUNTIME_LOADED;

			handle_got_whitelist_all(h);
			return SUCCESS;
		}

		if(l) {
			size_t idx = namespace - 1;
			for(struct restore *seg = rdwrs; seg != rdwrs + nrdwrs; ++seg)
				if((seg->addrs_stored[idx] = malloc(seg->size)))
					memcpy(seg->addrs_stored[idx],
						(void *) (l->l_addr + seg->off_loaded), seg->size);
				else
					goto oom;
		}
	}

	if(!(h->rdwrs = malloc(nrdwrs * sizeof *h->rdwrs)))
		goto oom;
	h->nrdwrs = nrdwrs;
	memcpy(h->rdwrs, rdwrs, nrdwrs * sizeof *h->rdwrs);

	return SUCCESS;

oom:
	for(size_t seg = 0; seg < nrdwrs; ++seg)
		for(ssize_t ns = 0; ns < config_numgroups(); ++ns)
			free(rdwrs[seg].addrs_stored[ns]);
	return ERROR_MALLOC;
}

bool handle_is_get_safe(const struct handle *h) {
	// Ensure this is not the vdso or the dynamic linker.
	return !h->vdso && strcmp(h->path, interp_path());
}

size_t handle_nodelete_pathlen(void) {
	static size_t size = 0;
	if(!size) {
		// Include the '/'!
		size = sizeof nodelete;
		assert(size == strlen(nodelete) + 1);
	}
	return size;
}

bool handle_is_nodelete(const struct handle *h) {
	return !strncmp(h->path, nodelete, handle_nodelete_pathlen() - 1);
}
