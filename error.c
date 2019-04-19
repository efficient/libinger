#include "error.h"

#include <elfutils/libasm.h>
#include <dlfcn.h>
#include <errno.h>
#include <stddef.h>
#include <string.h>

const char *error_message(enum error error) {
	const char *res = NULL;
	switch(error) {
	case ERROR_FNAME_PATH:
		res = "Determining path to program executable (check PATH environment variable)";
		break;
	case ERROR_MALLOC:
		res = "Unable to allocate memory";
		break;
	case ERROR_SIGACTION:
		res = "Unable to install intermediate signal handler";
		break;
	case ERROR_LIBASM:
		res = "Unable to initialize libasm";
		break;
	case SUCCESS:
		break;
	}

	return res;
}

const char *error_explanation(enum error error) {
	const char *res = NULL;
	switch(error) {
	case ERROR_MALLOC:
	case ERROR_SIGACTION:
		res = strerror(errno);
		break;
	case ERROR_LIBASM:
		res = asm_errmsg(asm_errno());
		break;
	default:
		break;
	}

	return res;
}
