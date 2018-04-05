#include <cstdlib>
#include <cstdio>
#include <vector>
#include <array>
#include <string>
#include <unistd.h>
#include <complex>
#include <thread>

#include "dir1/debuggee.h"
#include "dir2/debuggee.h"

extern "C"
void disassembly1();

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
#if !defined(__MINGW32__) || defined(_GLIBCXX_HAS_GTHREADS)
    std::vector<int> alive(num_threads);
    std::vector<std::thread> threads;
    for (int i = 0; i < num_threads; ++i) {
        int* am_alive = &alive[i];
        std::thread thread([am_alive](int id) {
            *am_alive = 1;
            printf("I'm thread %d\n", id);
            sleep(id % 4 + 1);
            printf("Thread %d exiting\n", id);
            *am_alive = 0;
        }, i);
        threads.push_back(std::move(thread));
    }
    sleep(1);
    for (int i = 0; i < num_threads; ++i) {
        printf("Joining %d\n", i);
        threads[i].join();
    }
#else
    sleep(1);
#endif
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

class Klazz {
    static int m1;
};
int Klazz::m1 = 42;

void vars() {
    struct Struct {
        int a;
        char b;
        float c;
        int d[4];
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
        int array_int[10] = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 };
        std::vector<std::vector<int>> vec_int(10, {i*1, i*2, i*3, i*4, i*5});
        std::vector<std::vector<int>> empty_vec;
        Struct s1 = { i+1, 'a', 3.0f, {i, i, i, i} };
        Struct s2 = { i+10, 'b', 999.0f, {i*10, i*10, i*10, i*10} };
        Struct* s_ptr = &s1;
        Struct* null_s_ptr = nullptr;
        Struct* invalid_s_ptr = (Struct*)1;
        std::vector<Struct> vec_struct(3, { i*2, 'b', 4.0f});
        std::array<int, 5> stdarr_int;
        Struct array_struct[5] = { { i*2, 'b', 4.0f} };
        std::string str1 = "The quick brown fox";
        char invalid_utf8[] = "ABC\xFF\x01\xFEXYZ";
        std::string empty_str;
        std::string* str_ptr = &str1;
        std::string& str_ref = str1;
        wchar_t wstr1[] = L"Превед йожэг!";
        std::wstring wstr2 = L"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!";
        int zzz = i; // #BP3
    }
}

void mandelbrot() {
    const int xdim = 500;
    const int ydim = 500;
    const int max_iter = 100;
    int image[xdim * ydim] = {0};
    for (int y = 0; y < ydim; ++y) {
        // /py debugvis.plot_image($image, $xdim, $ydim) if $y % 50 == 0 else False
        for (int x = 0; x < xdim; ++x) {
            std::complex<float> xy(-2.05 + x * 3.0 / xdim, -1.5 + y * 3.0 / ydim);
            std::complex<float> z(0, 0);
            int count = max_iter;
            for (int i = 0; i < max_iter; ++i) {
                z = z * z + xy;
                if (std::abs(z) >= 2) {
                    count = i;
                    break;
                }
            }
            image[y * xdim + x] = count;
        }
    }
    for (int y = 0; y < ydim; y += 10) {
        for (int x = 0; x < xdim; x += 5) {
            putchar(image[y * xdim + x] < max_iter ? '.' : '#');
        }
        putchar('\n');
    }
}

int main(int argc, char* argv[]) {
    std::vector<std::string> args;
    for (int i = 0; i < argc; ++i)
        args.push_back(argv[i]);

    if (args.size() < 2) { // #BP1
        return -1;
    }

    std::string testcase = args[1];
    if (testcase == "crash") {
        *(volatile int*)0 = 42;
    } else if (testcase == "deepstack") {
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
    } else if (testcase == "mandelbrot") {
        mandelbrot();
    } else if (testcase == "dasm") {
        disassembly1();
    }
    return 0;
}
