#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

void deepstack(int levelsToGo) {
    if (levelsToGo > 0) {
        deepstack(levelsToGo-1);
    }
} // #BP2

void inf_loop() {
    long long i = 0;
    for (;;) {
        i += 1;
    }
}

void threads(int num_threads) {
    std::vector<std::thread> ts;
    for (int i = 0; i < num_threads; ++i) {
        ts.emplace_back(inf_loop);
    }
    for (int i = 0; i < num_threads; ++i) {
        ts[i].join();
    }
}

int main(int argc, char* argv[]) {
    if (argc > 1) { // #BP1
        const char* testcase = argv[1];
        if (strcmp(testcase, "deepstack") == 0) {
            deepstack(50);
        } else if (strcmp(testcase, "threads") == 0) {
            threads(15);
        }
    }
    return 0;
}
