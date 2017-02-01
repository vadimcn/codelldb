#include <cstdlib>
#include <cstdio>
#include <vector>
#include <string>
#include <unistd.h>

#include "dir1/debuggee.h"
#include "dir2/debuggee.h"

void deepstack(int levelsToGo) {
    if (levelsToGo > 0) {
        deepstack(levelsToGo-1);
    }
} // #BP2

void inf_loop() {
    long long i = 0;
    for (;;) {
        printf("\r%lld ", i);
        fflush(stdout);
        sleep(1);
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

void echo() {
    char buffer[1024];
    do {
        fgets(buffer, sizeof(buffer), stdin);
        fputs(buffer, stdout);
    } while (buffer[0] != '\n'); // till empty line is read
}

void vars() {
    struct Struct {
        int a;
        char b;
        float c;
    };

    int a = 10;
    int b = 20;
    for (int i = 0; i < 10; i++)
    {
        int a = 30;
        int b = 40;
        static int sss = 555;
        const char c[] = "foobar";
        char buffer[10240] = {0};
        std::vector<std::vector<int>> v(10, {i*1, i*2, i*3, i*4, i*5});
        std::vector<std::vector<int>> empty_vec;
        Struct s = { i+1, 'a', 3.0f };
        std::vector<Struct> vs(3, { i*2, 'b', 4.0f});
        std::string str1 = "The quick brown fox";
        std::string empty_str;
        int zzz = i; // #BP3
    }
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
    } else if (testcase == "echo") {
        echo();
    } else if (testcase == "vars") {
        vars();
    } else if (testcase == "header") {
        header_fn1(1);
        header_fn2(2);
    }
    return 0;
}
