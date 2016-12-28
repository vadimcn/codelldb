#ifndef DEBUGGEE_H
#define DEBUGGEE_H
#include <cstdio>

inline int header_fn(int x) {
    printf("x=%d\n", x); // #BPH1
    return x + 1;
}

#endif
