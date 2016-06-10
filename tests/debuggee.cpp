#include <cstdlib>
#include <cstdio>
#include <thread>
#include <vector>
#include <string>

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

void show_env(const char* env_name) {
    const char* val = getenv(env_name);
    printf("%s=%s\n", env_name, val);
}

int main(int argc, char* argv[]) {
    if (argc > 1) { // #BP1
        std::string testcase = argv[1];
        if (testcase == "deepstack") {
            deepstack(50);
        } else if (testcase == "threads") {
            threads(15);
        } else if (testcase == "show_env") {
            show_env(argv[2]);
        } else if (testcase == "inf_loop") {
            inf_loop();
        }
    }
    return 0;
}
