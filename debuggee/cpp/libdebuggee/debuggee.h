#include <cstdio>

inline int header_fn_sharedlib(int x) {
    printf("header_fn_sharedlib(%d)\n", x);
    return x + 2;
}
