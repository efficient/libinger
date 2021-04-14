#include "config.h"

#include "namespace.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

bool config_staticlink(void) {
	#pragma weak libgotcha_staticlink
	extern const bool libgotcha_staticlink;
	return &libgotcha_staticlink && libgotcha_staticlink;
}

bool config_skip(const char *progname) {
	static bool memo;
	static const char *list;
	if(!memo) {
		list = getenv("LIBGOTCHA_SKIP");
		memo = true;
	}
	if(!list)
		return false;
	return strstr(list, progname);
}

ssize_t config_numgroups(void) {
	static bool memo;
	static ssize_t res = NUM_SHADOW_NAMESPACES;
	if(!memo) {
		const char *req;
		if((req = getenv("LIBGOTCHA_NUMGROUPS"))) {
			if(!sscanf(req, "%zd", &res))
				fputs("libgotcha warning: Ignoring non-numeric number of groups\n",
					stderr);
			else if(res == 0 || res > NUM_SHADOW_NAMESPACES) {
				fprintf(stderr, "libgotcha warning: Ignoring request for a number "
					"of groups outside the supported range of (0,%d]\n",
					NUM_SHADOW_NAMESPACES);
				res = NUM_SHADOW_NAMESPACES;
			}
		}
		memo = true;
	}
	return res;
}

bool config_sharedlibc(void) {
	static bool memo;
	static bool res;
	if(!memo) {
		if((res = getenv("LIBGOTCHA_SHAREDLIBC")))
			fputs("libgotcha notice: Treating entirety of libc as shared code\n",
				stderr);
		memo = true;
	}
	return res;
}

bool config_nodynamic(void) {
	static bool memo;
	static bool res;
	if(!memo) {
		if((res = getenv("LIBGOTCHA_NODYNAMIC")))
			fputs("libgotcha notice: Not hooking _dl_open() or _dl_close() calls\n",
				stderr);
		memo = true;
	}
	return res;
}

bool config_noglobals(void) {
	static bool memo;
	static bool res;
	if(!memo) {
		if((res = getenv("LIBGOTCHA_NOGLOBALS")))
			fputs("libgotcha notice: Shadowing of global variables has been disabled\n",
				stderr);
		memo = true;
	}
	return res;
}

FILE *config_traceglobals(void) {
	static bool memo;
	static FILE *res;
	if(!memo) {
		if(getenv("LIBGOTCHA_TRACEGLOBALS")) {
			fputs("libgotcha notice: Global variable access tracing has been enabled\n",
				stderr);
			res = stderr;
		}
		memo = true;
	}
	return res;
}

bool config_abortsegv(void) {
	static bool memo;
	static bool res;
	if(!memo) {
		if((res = getenv("LIBGOTCHA_ABORTSEGV")))
			fputs("libgotcha notice: Will abort rather than calling segfault handler\n",
				stderr);
		memo = true;
	}
	return res;
}
