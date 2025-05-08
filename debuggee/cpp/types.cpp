#include <vector>
#include <array>
#include <map>
#include <unordered_map>
#include <memory>
#include <string>

class Class
{
    static int ms;
    int m1 = 1;

public:
    Class() {}
    virtual ~Class() {}
};
int Class::ms = 42;

class DerivedClass : public Class
{
    int m2 = 2;
public:
    DerivedClass() {}
    ~DerivedClass() override {}
};


int global = 1234;

extern "C"
void vars()
{
    struct Struct
    {
        int a;
        char b;
        float c;
        int d[4];
    };

    struct DeepStruct
    {
        int a;
        const char *b;
        float c;
        Struct d;
        Struct e[5];
    };

    struct AnonUnion
    {
        union { int x; int w; };
        union { int y; int h; };
    };

    int a = 10;
    int b = 20;
    for (int j = 0; j < 10; j++)
    {
        int i = j;
        int a = 30;
        int b = 40;
        float pi = 3.14159265f;
        static int static_ = 555;
        const char c[] = "foobar";
        const char c2[] = { 'F', 'o', 'o', 'B', 'a', 'r' };
        int large_array[100000] = {0};
        int array_int[10] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
        int* array_int_ptr = array_int;
        std::vector<std::vector<int>> vec_int(10, {i * 1, i * 2, i * 3, i * 4, i * 5});
        std::vector<std::vector<int>> empty_vec;
        Struct s1 = {i + 1, 'a', 3.0f, {i, i, i, i}};
        Struct s2 = {i + 10, 'b', 999.0f, {i * 10, i * 10, i * 10, i * 10}};
        Struct* s_ptr = &s1;
        Struct& s_ref = s1;
        Struct** s_ptr_ptr = &s_ptr;
        Struct* null_s_ptr = nullptr;
        Struct** null_s_ptr_ptr = &null_s_ptr;
        Struct* invalid_s_ptr = (Struct *)1;
        void* void_ptr = &s1;
        void* null_void_ptr = nullptr;
        void* invalid_void_ptr = (void*)1;
        AnonUnion anon_union = { 4, 5 };
        DeepStruct ds1 = {13, "foo", 3.14f,                  //
                          {i, 'd', 4.0f, {1, 2, 3, i}},      //
                          {{i * 2, 's', 5.0f, {4, 5, 6, i}}, //
                           {i * 3, 'x', 5.5f, {3, 5, 1, i}}}};

        Class class_obj;
        DerivedClass derived_class_obj;
        Class* class_ptr = &derived_class_obj;

        std::vector<Struct> vec_struct(3, {i * 2, 'b', 4.0f});
        std::array<int, 5> stdarr_int;
        std::map<int, float> ord_map = {{1, 2.34f}, {2, 3.56f}};
        std::unordered_map<int, float> unord_map = {{1, 2.34f}, {2, 3.56f}};
        auto shared_ptr = std::make_shared<std::map<int, float>>(ord_map);

        Struct array_struct[5];
        for (int j = 0; j < 5; ++j)
            array_struct[j] = { i*2 + j, (char)('a'+ j), (float)j};
        Struct* array_struct_p = array_struct;

        const char *cstr = "The quick brown fox";
        const wchar_t *wcstr = L"The quick brown fox";
        std::string str1 = "The quick brown fox";
        char invalid_utf8[] = "ABC\xFF\x01\xFEXYZ";
        std::string empty_str;
        std::string *str_ptr = &str1;
        std::string &str_ref = str1;
        wchar_t wstr1[] = L"Превед йожэг!";
        std::wstring wstr2 = L"Ḥ̪͔̦̺E͍̹̯̭͜ C̨͙̹̖̙O̡͍̪͖ͅM̢̗͙̫̬E̜͍̟̟̮S̢̢̪̘̦!";
        int zzz = i; // #BP3
    }
}

extern "C"
void vars_update()
{
    std::vector<int> vector;
    for (int i = 0; i < 10; i++)
    {
        vector.push_back(i);
        int zzz = i; // #BP4
    }
}
