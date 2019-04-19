#include "config.h"
#include "globals.h"

#include <signal.h>
#include <string.h>

int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact) {
	if(signum != SIGSEGV || config_noglobals())
		return sigaction(signum, act, oldact);

	struct sigaction *myact = globals_handler();
	if(oldact)
		memcpy(oldact, myact, sizeof *oldact);
	if(act)
		memcpy(myact, act, sizeof *myact);
	return 0;
}
