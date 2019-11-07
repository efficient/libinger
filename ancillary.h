#ifndef ANCILLARY_H_
#define ANCILLARY_H_

#include "error.h"

#include <stdbool.h>

bool ancillary_namespace(void);
enum error ancillary_disable_ctors_dtors(void);

#endif
