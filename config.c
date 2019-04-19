#include "config.h"

#include <stdio.h>
#include <stdlib.h>

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
