#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

void breakpoint() {
}

void deepstack(int levels) {
    if (levels > 0) {
        deepstack(levels-1);
    }
    breakpoint();
}

void inf_loop() {
    long long i = 0;
    for (;;) {
        i += 1;
    }
}

void threads(int num_threads) {
    std::vector<std::thread*> ts;
    for (int i = 0; i < num_threads; ++i) {
        ts.push_back(new std::thread(inf_loop));
    }
}

int main(int argc, char* argv[]) {
    // #BP1
    if (argc > 1) {
        const char* testcase = argv[1];
        if (strcmp(testcase, "deepstack") == 0) {
            deepstack(50);
        } else if (strcmp(testcase, "threads") == 0) {
            threads(15);
        }
    }
    return 0;
}
