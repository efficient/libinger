#include "error.h"

#include <dlfcn.h>
#include <errno.h>
#include <stddef.h>
#include <string.h>

const char *error_message(enum error error) {
	const char *res = NULL;
	switch(error) {
	case ERROR_DLADDR:
		res = "Recovering symbol information from function pointer";
		break;
	case ERROR_DLI_FNAME:
		res = "Determining path to function pointer's object file";
		break;
	case ERROR_FNAME_MISMATCH:
		res = "Given invalid path to object file (consider not passing this optional arg)";
		break;
	case ERROR_FNAME_PATH:
		res = "Determining path to program executable (check PATH environment variable)";
		break;
	case ERROR_OPEN:
		res = "Unable to open object file for reading";
		break;
	case ERROR_MMAP:
		res = "Unable to map object file section header";
		break;
	case ERROR_UNSUPPORTED_RELOCS:
		res = "Object file contains unsupported relocation type(s)";
		break;
	case ERROR_MALLOC:
		res = "Unable to allocate memory";
		break;
	case ERROR_LIB_SIZE:
		res = "Library contains too many relocations to fit its trampolines in one page";
		break;
	case ERROR_DLOPEN:
		res = "Unable to open ancillary copy of object file";
		break;
	case ERROR_MPROTECT:
		res = "Unable to alter memory page protection";
		break;
	default:
		break;
	}

	return res;
}

const char *error_explanation(enum error error) {
	const char *res = NULL;
	switch(error) {
	case ERROR_OPEN:
	case ERROR_MMAP:
	case ERROR_MALLOC:
	case ERROR_MPROTECT:
		res = strerror(errno);
		break;
	case ERROR_DLOPEN:
		res = dlerror();
		break;
	default:
		break;
	}

	return res;
}
