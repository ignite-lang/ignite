/* C ABI smoke test for libignite_rt.a (docs/llvm.md section 5): compiled and
 * linked with clang by tests/c_abi.rs, outside cargo. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { uint8_t *ptr; int64_t len; int64_t cap; } IgniteList;
typedef struct { IgniteList message; int32_t exit_code; } IgniteIoError;
typedef struct { int32_t tag; IgniteList data; } IgniteInput;

void ignite_rt_init(int32_t argc, char **argv);
void *ignite_rt_alloc(int64_t size, int64_t align);
void ignite_rt_spawn(void (*fn)(void *), void *env);
void ignite_rt_spawn_ready(void);
void ignite_rt_task_end(void);
int32_t ignite_rt_args_port(int64_t *out_port, int32_t *present, IgniteIoError *err);
void ignite_rt_exit(int32_t code);
void ignite_rt_fail(IgniteIoError *error);
void ignite_rt_panic(IgniteList *message);
void ignite_rt_log(IgniteList *line);
int32_t ignite_rt_tcp_bind(IgniteList *address, int64_t port, int64_t *out, IgniteIoError *err);
int32_t ignite_rt_tcp_accept(int64_t listener, int64_t *out, IgniteIoError *err);
int64_t ignite_rt_tcp_local_port(int64_t listener);
int32_t ignite_rt_tcp_connect(IgniteList *address, int64_t port, int64_t *out, IgniteIoError *err);
int32_t ignite_rt_tcp_read(int64_t stream, int64_t maximum, IgniteInput *out, IgniteIoError *err);
int32_t ignite_rt_tcp_write_all(int64_t stream, IgniteList *bytes, IgniteIoError *err);
int32_t ignite_rt_tcp_close(int64_t stream, IgniteIoError *err);
void ignite_rt_tcp_release(int64_t handle);
int32_t ignite_rt_stdin_read(int64_t maximum, IgniteInput *out, IgniteIoError *err);
int32_t ignite_rt_stdin_is_terminal(int8_t *out, IgniteIoError *err);
int32_t ignite_rt_stdin_set_raw(int8_t on, IgniteIoError *err);
int32_t ignite_rt_stdout_write_all(IgniteList *bytes, IgniteIoError *err);
void ignite_rt_io_error_new(IgniteIoError *out, IgniteList *message);
void ignite_rt_text_from_bytes_lossy(IgniteList *out, IgniteList *bytes);
void ignite_rt_text_trim(IgniteList *out, IgniteList *text);
void ignite_rt_text_concat(IgniteList *out, IgniteList *a, IgniteList *b);
int8_t ignite_rt_text_eq(IgniteList *a, IgniteList *b);
int8_t ignite_rt_text_starts_with(IgniteList *text, IgniteList *prefix);
void ignite_rt_text_join(IgniteList *out, IgniteList *texts, IgniteList *separator);
void ignite_rt_int_to_text(IgniteList *out, int64_t value);

static IgniteList bytes(const void *data, size_t length) {
    uint8_t *ptr = ignite_rt_alloc((int64_t)length, 1);
    memcpy(ptr, data, length);
    return (IgniteList){ptr, (int64_t)length, (int64_t)length};
}

static IgniteList lit(const char *text) { return bytes(text, strlen(text)); }

static void check(int32_t status, IgniteIoError *err) {
    if (status != 0) ignite_rt_fail(err);
}

static void say(IgniteList text) {
    IgniteIoError err;
    IgniteList line, newline = lit("\n");
    ignite_rt_text_concat(&line, &text, &newline);
    check(ignite_rt_stdout_write_all(&line, &err), &err);
}

struct env { IgniteList prefix; int64_t listener; };

static void serve(void *pointer) {
    struct env *env = pointer;
    IgniteList prefix = bytes(env->prefix.ptr, (size_t)env->prefix.len);
    int64_t listener = env->listener;
    ignite_rt_spawn_ready();
    IgniteIoError err;
    int64_t stream;
    check(ignite_rt_tcp_accept(listener, &stream, &err), &err);
    IgniteInput input;
    check(ignite_rt_tcp_read(stream, 64, &input, &err), &err);
    IgniteList reply;
    ignite_rt_text_concat(&reply, &prefix, &input.data);
    check(ignite_rt_tcp_write_all(stream, &reply, &err), &err);
    check(ignite_rt_tcp_close(stream, &err), &err);
    ignite_rt_tcp_release(stream);
    ignite_rt_task_end();
}

int main(int argc, char **argv) {
    ignite_rt_init(argc, argv);
    const char *mode = argc > 1 ? argv[1] : "";
    IgniteIoError err;

    if (strcmp(mode, "port") == 0) {
        int64_t port = -1;
        int32_t present = -1;
        check(ignite_rt_args_port(&port, &present, &err), &err);
        IgniteList number, text, label = lit("port "), present_label = lit(" present ");
        ignite_rt_int_to_text(&number, port);
        ignite_rt_text_concat(&text, &label, &number);
        ignite_rt_text_concat(&text, &text, &present_label);
        ignite_rt_int_to_text(&number, present);
        ignite_rt_text_concat(&text, &text, &number);
        say(text);
        return 0;
    }
    if (strcmp(mode, "fail") == 0) {
        IgniteList message = lit("boom");
        ignite_rt_io_error_new(&err, &message);
        err.exit_code = 3;
        ignite_rt_fail(&err);
    }
    if (strcmp(mode, "panic") == 0) {
        IgniteList message = lit("index 9 out of range");
        ignite_rt_panic(&message);
    }
    if (strcmp(mode, "exit") == 0) {
        ignite_rt_exit(7);
    }

    char layout[64];
    snprintf(layout, sizeof layout, "layout %zu %zu %zu %zu", sizeof(IgniteList),
             sizeof(IgniteIoError), offsetof(IgniteInput, data), sizeof(IgniteInput));
    say(lit(layout));

    IgniteList out, a = lit("  padded\t\n"), b = lit("ign"), c = lit("ite");
    ignite_rt_text_trim(&out, &a);
    say(out);
    ignite_rt_text_concat(&out, &b, &c);
    say(out);
    IgniteList *parts = ignite_rt_alloc(3 * sizeof(IgniteList), 8);
    parts[0] = lit("x");
    parts[1] = lit("y");
    parts[2] = lit("z");
    IgniteList list = {(uint8_t *)parts, 3, 3}, separator = lit("-");
    ignite_rt_text_join(&out, &list, &separator);
    say(out);
    ignite_rt_int_to_text(&out, -1234567890123LL);
    say(out);
    IgniteList invalid = bytes("a\xff", 2);
    ignite_rt_text_from_bytes_lossy(&out, &invalid);
    say(out);
    IgniteList x = lit("same"), y = lit("same"), prefix = lit("sa");
    char flags[32];
    snprintf(flags, sizeof flags, "eq %d starts %d", ignite_rt_text_eq(&x, &y),
             ignite_rt_text_starts_with(&x, &prefix));
    say(lit(flags));

    struct env env = {lit("echo: "), 0};
    IgniteList host = lit("127.0.0.1");
    check(ignite_rt_tcp_bind(&host, 0, &env.listener, &err), &err);
    ignite_rt_spawn(serve, &env);
    int64_t stream;
    check(ignite_rt_tcp_connect(&host, ignite_rt_tcp_local_port(env.listener), &stream, &err), &err);
    IgniteList ping = lit("ping");
    check(ignite_rt_tcp_write_all(stream, &ping, &err), &err);
    IgniteInput input;
    IgniteList received = lit("");
    for (;;) {
        check(ignite_rt_tcp_read(stream, 1024, &input, &err), &err);
        if (input.tag == 1) break;
        ignite_rt_text_concat(&received, &received, &input.data);
    }
    say(received);
    ignite_rt_tcp_release(stream);
    ignite_rt_tcp_release(env.listener);

    IgniteList line = lit("logged to stderr");
    ignite_rt_log(&line);
    return 0;
}
