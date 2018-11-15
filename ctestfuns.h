#ifndef C_TEST_FUNS_H_
#define C_TEST_FUNS_H_

#include <functional>

void (*make_func(void))(void);
void (*make_fnc(void))(void);
std::function<void(void)> make_fn(void);

#endif
