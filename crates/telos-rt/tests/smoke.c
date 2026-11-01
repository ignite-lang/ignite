/* C ABI smoke test for libtelos_rt.a (docs/llvm.md section 5): compiled and
 * linked with clang by tests/c_abi.rs, outside cargo. */
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { uint8_t *ptr; int64_t len; int64_t cap; } TelosList;
typedef struct { TelosList message; int32_t exit_code; } TelosIoError;
typedef struct { int32_t tag; TelosList data; } TelosInput;

void telos_rt_init(int32_t argc, char **argv);
void *telos_rt_alloc(int64_t size, int64_t align);
void telos_rt_spawn(void (*fn)(void *), void *env);
void telos_rt_spawn_ready(void);
void telos_rt_task_end(void);
int32_t telos_rt_args_port(int64_t *out_port, int32_t *present, TelosIoError *err);
void telos_rt_exit(int32_t code);
void telos_rt_fail(TelosIoError *error);
void telos_rt_panic(TelosList *message);
void telos_rt_log(TelosList *line);
int32_t telos_rt_tcp_bind(TelosList *address, int64_t port, int64_t *out, TelosIoError *err);
int32_t telos_rt_tcp_accept(int64_t listener, int64_t *out, TelosIoError *err);
int64_t telos_rt_tcp_local_port(int64_t listener);
int32_t telos_rt_tcp_connect(TelosList *address, int64_t port, int64_t *out, TelosIoError *err);
int32_t telos_rt_tcp_read(int64_t stream, int64_t maximum, TelosInput *out, TelosIoError *err);
int32_t telos_rt_tcp_write_all(int64_t stream, TelosList *bytes, TelosIoError *err);
int32_t telos_rt_tcp_close(int64_t stream, TelosIoError *err);
void telos_rt_tcp_release(int64_t handle);
int32_t telos_rt_stdin_read(int64_t maximum, TelosInput *out, TelosIoError *err);
int32_t telos_rt_stdin_is_terminal(int8_t *out, TelosIoError *err);
int32_t telos_rt_stdin_set_raw(int8_t on, TelosIoError *err);
int32_t telos_rt_stdout_write_all(TelosList *bytes, TelosIoError *err);
void telos_rt_io_error_new(TelosIoError *out, TelosList *message);
void telos_rt_text_from_bytes_lossy(TelosList *out, TelosList *bytes);
void telos_rt_text_trim(TelosList *out, TelosList *text);
void telos_rt_text_concat(TelosList *out, TelosList *a, TelosList *b);
int8_t telos_rt_text_eq(TelosList *a, TelosList *b);
int8_t telos_rt_text_starts_with(TelosList *text, TelosList *prefix);
void telos_rt_text_join(TelosList *out, TelosList *texts, TelosList *separator);
void telos_rt_int_to_text(TelosList *out, int64_t value);

static TelosList bytes(const void *data, size_t length) {
    uint8_t *ptr = telos_rt_alloc((int64_t)length, 1);
    memcpy(ptr, data, length);
    return (TelosList){ptr, (int64_t)length, (int64_t)length};
}

static TelosList lit(const char *text) { return bytes(text, strlen(text)); }

static void check(int32_t status, TelosIoError *err) {
    if (status != 0) telos_rt_fail(err);
}

static void say(TelosList text) {
    TelosIoError err;
    TelosList line, newline = lit("\n");
    telos_rt_text_concat(&line, &text, &newline);
    check(telos_rt_stdout_write_all(&line, &err), &err);
}

struct env { TelosList prefix; int64_t listener; };

static void serve(void *pointer) {
    struct env *env = pointer;
    TelosList prefix = bytes(env->prefix.ptr, (size_t)env->prefix.len);
    int64_t listener = env->listener;
    telos_rt_spawn_ready();
    TelosIoError err;
    int64_t stream;
    check(telos_rt_tcp_accept(listener, &stream, &err), &err);
    TelosInput input;
    check(telos_rt_tcp_read(stream, 64, &input, &err), &err);
    TelosList reply;
    telos_rt_text_concat(&reply, &prefix, &input.data);
    check(telos_rt_tcp_write_all(stream, &reply, &err), &err);
    check(telos_rt_tcp_close(stream, &err), &err);
    telos_rt_tcp_release(stream);
    telos_rt_task_end();
}

int main(int argc, char **argv) {
    telos_rt_init(argc, argv);
    const char *mode = argc > 1 ? argv[1] : "";
    TelosIoError err;

    if (strcmp(mode, "port") == 0) {
        int64_t port = -1;
        int32_t present = -1;
        check(telos_rt_args_port(&port, &present, &err), &err);
        TelosList number, text, label = lit("port "), present_label = lit(" present ");
        telos_rt_int_to_text(&number, port);
        telos_rt_text_concat(&text, &label, &number);
        telos_rt_text_concat(&text, &text, &present_label);
        telos_rt_int_to_text(&number, present);
        telos_rt_text_concat(&text, &text, &number);
        say(text);
        return 0;
    }
    if (strcmp(mode, "fail") == 0) {
        TelosList message = lit("boom");
        telos_rt_io_error_new(&err, &message);
        err.exit_code = 3;
        telos_rt_fail(&err);
    }
    if (strcmp(mode, "panic") == 0) {
        TelosList message = lit("index 9 out of range");
        telos_rt_panic(&message);
    }
    if (strcmp(mode, "exit") == 0) {
        telos_rt_exit(7);
    }

    char layout[64];
    snprintf(layout, sizeof layout, "layout %zu %zu %zu %zu", sizeof(TelosList),
             sizeof(TelosIoError), offsetof(TelosInput, data), sizeof(TelosInput));
    say(lit(layout));

    TelosList out, a = lit("  padded\t\n"), b = lit("tel"), c = lit("os");
    telos_rt_text_trim(&out, &a);
    say(out);
    telos_rt_text_concat(&out, &b, &c);
    say(out);
    TelosList *parts = telos_rt_alloc(3 * sizeof(TelosList), 8);
    parts[0] = lit("x");
    parts[1] = lit("y");
    parts[2] = lit("z");
    TelosList list = {(uint8_t *)parts, 3, 3}, separator = lit("-");
    telos_rt_text_join(&out, &list, &separator);
    say(out);
    telos_rt_int_to_text(&out, -1234567890123LL);
    say(out);
    TelosList invalid = bytes("a\xff", 2);
    telos_rt_text_from_bytes_lossy(&out, &invalid);
    say(out);
    TelosList x = lit("same"), y = lit("same"), prefix = lit("sa");
    char flags[32];
    snprintf(flags, sizeof flags, "eq %d starts %d", telos_rt_text_eq(&x, &y),
             telos_rt_text_starts_with(&x, &prefix));
    say(lit(flags));

    struct env env = {lit("echo: "), 0};
    TelosList host = lit("127.0.0.1");
    check(telos_rt_tcp_bind(&host, 0, &env.listener, &err), &err);
    telos_rt_spawn(serve, &env);
    int64_t stream;
    check(telos_rt_tcp_connect(&host, telos_rt_tcp_local_port(env.listener), &stream, &err), &err);
    TelosList ping = lit("ping");
    check(telos_rt_tcp_write_all(stream, &ping, &err), &err);
    TelosInput input;
    TelosList received = lit("");
    for (;;) {
        check(telos_rt_tcp_read(stream, 1024, &input, &err), &err);
        if (input.tag == 1) break;
        telos_rt_text_concat(&received, &received, &input.data);
    }
    say(received);
    telos_rt_tcp_release(stream);
    telos_rt_tcp_release(env.listener);

    TelosList line = lit("logged to stderr");
    telos_rt_log(&line);
    return 0;
}
