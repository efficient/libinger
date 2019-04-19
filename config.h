#ifndef CONFIG_H_
#define CONFIG_H_

#include <stdbool.h>
#include <stdio.h>

bool config_noglobals(void);
FILE *config_traceglobals(void);

#endif
