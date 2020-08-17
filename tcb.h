#ifndef TCB_H_
#define TCB_H_

#include <stdint.h>

// Unlike other TLS variables, these must *not* persist when the TCB is manually switched.  As such,
// they must be resolved via segment selector, not the __tls_get_addr() helper!

uintptr_t *tcb_custom(void);
uintptr_t *tcb_parent(void);

#endif
