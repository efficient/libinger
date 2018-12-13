#include "error.h"

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
	case ERROR_FNAME_REALPATH:
		res = "Determining path to program executable (consider passing as optional arg)";
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
	default:
		break;
	}

	return res;
}

const char *error_explanation(enum error error) {
	const char *res = NULL;
	switch(error) {
	case ERROR_FNAME_REALPATH:
	case ERROR_OPEN:
	case ERROR_MMAP:
		res = strerror(errno);
		break;
	default:
		break;
	}

	return res;
}
