#include "debuggee.h"

extern "C"
void sharedlib_entry() {
    header_fn_sharedlib(3);
}
