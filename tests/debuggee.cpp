#include <cstdlib>
#include <cstdio>
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
}

bool check_env(const char* env_name, const char* expected) {
    const char* val = getenv(env_name);
    printf("%s=%s\n", env_name, val);
    return val && std::string(val) == std::string(expected);
}

int main(int argc, char* argv[]) {
    if (argc < 2) { // #BP1
        return -1;
    }
    std::string testcase = argv[1];
    if (testcase == "deepstack") {
        deepstack(50);
    } else if (testcase == "threads") {
        threads(15);
    } else if (testcase == "check_env") {
        if (argc < 4) {
            return -1;
        }
        return (int)check_env(argv[2], argv[3]);
    } else if (testcase == "inf_loop") {
        inf_loop();
    }
    return 0;
}
