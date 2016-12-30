#include <cstdio>

inline int header_fn1(int x) {
    printf("header_fn1(%d)\n", x); // #BPH1
    return x + 1;
}
