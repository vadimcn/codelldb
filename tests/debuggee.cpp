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
    {
        int a = 30;
        int b = 40;
        const char c[] = "foobar";
        char buffer[10240] = {0};
        std::vector<std::vector<int>> v(10, {1, 2, 3, 4, 5});
        Struct s = { 1, 'a', 3.0f };
        std::vector<Struct> vs(3, { 2, 'b', 4.0f});
        std::string str = "The quick brown fox";
        int zzz = 0; // #BP3
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
    }
    return 0;
}
