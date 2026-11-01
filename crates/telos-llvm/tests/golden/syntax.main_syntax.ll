; ModuleID = 'main_syntax'
source_filename = "main_syntax"

@telos.empty = private constant [8 x i8] zeroinitializer, align 8
@telos.msg.index = private unnamed_addr constant [18 x i8] c"index out of range", align 1
@telos.msg.slice = private unnamed_addr constant [18 x i8] c"slice out of range", align 1
@telos.msg.divide = private unnamed_addr constant [25 x i8] c"attempt to divide by zero", align 1
@telos.msg.match = private unnamed_addr constant [12 x i8] c"no match arm", align 1
@.str.0 = private unnamed_addr constant [9 x i8] c"127.0.0.1", align 1

declare void @telos_rt_init(i32, ptr)
declare ptr @telos_rt_alloc(i64, i64)
declare void @telos_rt_spawn(ptr, ptr)
declare void @telos_rt_spawn_ready()
declare void @telos_rt_task_end()
declare i64 @telos_rt_arena_mark()
declare void @telos_rt_arena_release(i64)
declare i8 @telos_rt_arena_due(i64, i64)
declare void @telos_rt_arena_scratch(i8)
declare void @telos_rt_arena_scratch_clear()
declare ptr @telos_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i8 @telos_rt_spawn_copy(ptr, ptr, i64)
declare i32 @telos_rt_args_port(ptr, ptr, ptr)
declare void @telos_rt_exit(i32) noreturn
declare void @telos_rt_fail(ptr) noreturn
declare void @telos_rt_panic(ptr) noreturn
declare void @telos_rt_log(ptr)
declare i32 @telos_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @telos_rt_tcp_accept(i64, ptr, ptr)
declare i64 @telos_rt_tcp_local_port(i64)
declare i32 @telos_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @telos_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @telos_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @telos_rt_tcp_close(i64, ptr)
declare void @telos_rt_tcp_release(i64)
declare i32 @telos_rt_stdin_read(i64, ptr, ptr)
declare i32 @telos_rt_stdin_is_terminal(ptr, ptr)
declare i32 @telos_rt_stdin_set_raw(i8, ptr)
declare i32 @telos_rt_stdout_write_all(ptr, ptr)
declare void @telos_rt_io_error_new(ptr, ptr)
declare void @telos_rt_text_from_bytes_lossy(ptr, ptr)
declare void @telos_rt_text_trim(ptr, ptr)
declare void @telos_rt_text_concat(ptr, ptr, ptr)
declare i8 @telos_rt_text_eq(ptr, ptr)
declare i8 @telos_rt_text_starts_with(ptr, ptr)
declare void @telos_rt_text_join(ptr, ptr, ptr)
declare void @telos_rt_int_to_text(ptr, i64)
declare i8 @telos_rt_int_parse(ptr, ptr)
declare i32 @telos_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @telos_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @telos_rt_store_next_id(ptr)
declare void @telos_rt_store_put(ptr, i64, ptr)
declare i8 @telos_rt_store_get(ptr, i64, ptr)
declare i8 @telos_rt_store_remove(ptr, i64, ptr)
declare void @telos_rt_store_values(ptr, ptr)
declare i8 @telos_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @"tests.syntax.Syntax.connect"(ptr, i64)

define internal void @telos.panic(ptr %bytes, i64 %len) cold noreturn {
entry:
  %text = alloca [24 x i8], align 8
  store ptr %bytes, ptr %text, align 8
  %lenp = getelementptr inbounds i8, ptr %text, i64 8
  store i64 %len, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %text, i64 16
  store i64 %len, ptr %capp, align 8
  call void @telos_rt_panic(ptr %text)
  unreachable
}

define internal void @telos.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @telos.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal void @telos.check_index(i64 %i, i64 %len) {
entry:
  %ok = icmp ult i64 %i, %len
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.index, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_slice(i64 %i, i64 %j, i64 %len) {
entry:
  %a = icmp sge i64 %i, 0
  %b = icmp sle i64 %i, %j
  %c = icmp sle i64 %j, %len
  %ab = and i1 %a, %b
  %ok = and i1 %ab, %c
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.slice, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_divisor(i64 %d) {
entry:
  %ok = icmp ne i64 %d, 0
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.divide, i64 25)
  unreachable
fine:
  ret void
}

define internal void @telos.no_match() cold noreturn {
entry:
  call void @telos.panic(ptr @telos.msg.match, i64 12)
  unreachable
}

define internal ptr @telos.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ult i64 %end, %li
  %ok = and i1 %small, %fits
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @telos_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @telos.arena_mark() alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @telos.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @telos.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @telos.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @telos_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define i32 @main(i32 %argc, ptr %argv) {
entry:
  %t1 = alloca [32 x i8], align 8
  %t2 = alloca [8 x i8], align 8
  %t3 = alloca [4 x i8], align 4
  %t13 = alloca [24 x i8], align 8
  %t16 = alloca [8 x i8], align 8
  %t23 = alloca [40 x i8], align 8
  call void @telos_rt_init(i32 %argc, ptr %argv)
  %t4 = call i32 @telos_rt_args_port(ptr %t2, ptr %t3, ptr %t1)
  %t5 = icmp ne i32 %t4, 0
  br i1 %t5, label %L6, label %L7
L6:
  %t8 = getelementptr inbounds i8, ptr %t1, i64 24
  store i32 2, ptr %t8, align 4
  call void @telos_rt_fail(ptr %t1)
  unreachable
L7:
  %t9 = load i32, ptr %t3, align 4
  %t10 = icmp ne i32 %t9, 0
  %t11 = load i64, ptr %t2, align 8
  %t12 = select i1 %t10, i64 %t11, i64 1
  store ptr @.str.0, ptr %t13, align 8
  %t14 = getelementptr inbounds i8, ptr %t13, i64 8
  store i64 9, ptr %t14, align 8
  %t15 = getelementptr inbounds i8, ptr %t13, i64 16
  store i64 9, ptr %t15, align 8
  %t17 = call i32 @telos_rt_tcp_connect(ptr %t13, i64 %t12, ptr %t16, ptr %t1)
  %t18 = icmp ne i32 %t17, 0
  br i1 %t18, label %L19, label %L20
L19:
  %t21 = getelementptr inbounds i8, ptr %t1, i64 24
  store i32 3, ptr %t21, align 4
  call void @telos_rt_fail(ptr %t1)
  unreachable
L20:
  %t22 = load i64, ptr %t16, align 8
  call void @"tests.syntax.Syntax.connect"(ptr %t23, i64 %t22)
  %t24 = load i32, ptr %t23, align 4
  %t25 = icmp eq i32 %t24, 1
  br i1 %t25, label %L26, label %L27
L26:
  %t28 = getelementptr inbounds i8, ptr %t23, i64 8
  %t29 = getelementptr inbounds i8, ptr %t28, i64 24
  store i32 4, ptr %t29, align 4
  call void @telos_rt_fail(ptr %t28)
  unreachable
L27:
  call void @telos_rt_exit(i32 0)
  unreachable
}
