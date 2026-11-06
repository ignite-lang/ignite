target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'tests.bounds'
source_filename = "tests.bounds"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.0 = private unnamed_addr constant [5 x i8] c"pipe\0A", align 1
@.str.1 = private unnamed_addr constant [23 x i8] c"task ended with error: ", align 1
@.str.2 = private unnamed_addr constant [26 x i8] c"tests/golden/bounds.ignite", align 1
$"tests.bounds.Echo.serve<TcpStream>" = comdat any
$"tests.bounds.Echo.serve<PipeStream>" = comdat any
$"tests.bounds.relay<PipeStream>" = comdat any
$"tests.bounds.relay<TcpStream>" = comdat any

declare void @ignite_rt_init(i32, ptr)
declare ptr @ignite_rt_alloc(i64, i64)
declare i8 @ignite_rt_spawn(ptr, ptr)
declare void @ignite_rt_spawn_ready()
declare void @ignite_rt_task_end()
declare i64 @ignite_rt_arena_mark()
declare void @ignite_rt_arena_release(i64)
declare i8 @ignite_rt_arena_due(i64, i64)
declare void @ignite_rt_arena_scratch(i8)
declare void @ignite_rt_arena_scratch_clear()
declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i32 @ignite_rt_args_port(ptr, ptr, ptr)
declare void @ignite_rt_exit(i32) noreturn
declare void @ignite_rt_fail(ptr) noreturn
declare void @ignite_rt_log(ptr)
declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)
declare i64 @ignite_rt_tcp_local_port(i64)
declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @ignite_rt_tcp_close(i64, ptr)
declare void @ignite_rt_tcp_release(i64)
declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)
declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)
declare i32 @ignite_rt_stdin_set_raw(i8, ptr)
declare i32 @ignite_rt_stdout_write_all(ptr, ptr)
declare i32 @ignite_rt_stderr_write_all(ptr, ptr)
declare void @ignite_rt_io_error_new(ptr, ptr)
declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)
declare void @ignite_rt_text_trim(ptr, ptr)
declare void @ignite_rt_text_concat(ptr, ptr, ptr)
declare i8 @ignite_rt_text_eq(ptr, ptr)
declare i8 @ignite_rt_text_starts_with(ptr, ptr)
declare void @ignite_rt_text_join(ptr, ptr, ptr)
declare void @ignite_rt_int_to_text(ptr, i64)
declare i8 @ignite_rt_int_parse(ptr, ptr)
declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @ignite_rt_store_next_id(ptr)
declare void @ignite_rt_store_put(ptr, i64, ptr)
declare i8 @ignite_rt_store_get(ptr, i64, ptr)
declare i8 @ignite_rt_store_remove(ptr, i64, ptr)
declare void @ignite_rt_store_values(ptr, ptr)
declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare i32 @_setjmp(ptr) returns_twice
declare void @"ignite_rt_fail_match"(ptr, i64, i64)
declare void @"ignite_rt_guard_pop"()
declare void @"ignite_rt_guard_push"(ptr)
declare i64 @"ignite_rt_own_mark"()
declare void @"ignite_rt_own_release"(i64)
declare i32 @ignite_rt_pipe_close(i64, ptr)
declare i32 @ignite_rt_pipe_read(i64, i64, ptr, ptr)
declare i64 @ignite_rt_pipe_received_batches(i64)
declare i32 @ignite_rt_pipe_set_idle_timeout(i64, i64, ptr)
declare i32 @ignite_rt_pipe_write_all(i64, ptr, ptr)
declare void @"ignite_rt_tcp_own"(i64)
declare void @"ignite_rt_tcp_retain"(i64)

define internal void @ignite.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
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
  %new = call ptr @ignite.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal ptr @ignite.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
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
  %fits = icmp ule i64 %end, %li
  %live = icmp ne i64 %li, 0
  %fit = and i1 %fits, %live
  %ok = and i1 %small, %fit
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @ignite_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @ignite.arena_mark() alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @ignite.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @ignite.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @ignite.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @ignite_rt_arena_state()
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
  call void @ignite_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define void @"tests.bounds.over_tcp"(ptr %sret, i64 %p0) !dbg !4 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !5
  %t2 = load i64, ptr %t1, align 8, !dbg !6
  call void @"tests.bounds.relay<TcpStream>"(ptr %t3, i64 %t2), !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t3, i64 40, i1 false), !dbg !5
  ret void, !dbg !5
}

define i64 @"tests.bounds.same"(i64 %p0) !dbg !7 {
entry:
  %t1 = alloca [8 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !8
  %t2 = load i64, ptr %t1, align 8, !dbg !8
  ret i64 %t2, !dbg !8
}

define internal void @"spawn.tests.bounds.relay<PipeStream>.1"(ptr %env) {
entry:
  %t1 = alloca [256 x i8], align 16
  %t7 = alloca [40 x i8], align 8
  %t16 = alloca [24 x i8], align 8
  %t2 = call i32 @_setjmp(ptr %t1)
  %t3 = icmp eq i32 %t2, 0
  br i1 %t3, label %L4, label %L5
L5:
  call void @ignite_rt_task_end()
  ret void
L4:
  call void @ignite_rt_guard_push(ptr %t1)
  %t6 = load i64, ptr %env, align 8
  call void @ignite_rt_tcp_own(i64 %t6)
  call void @ignite_rt_spawn_ready()
  call void @"tests.bounds.relay<PipeStream>"(ptr %t7, i64 %t6)
  %t8 = load i32, ptr %t7, align 4
  %t9 = icmp eq i32 %t8, 1
  br i1 %t9, label %L10, label %L11
L10:
  %t12 = getelementptr inbounds i8, ptr %t7, i64 8
  %t13 = load ptr, ptr %t12, align 8
  %t14 = getelementptr inbounds i8, ptr %t12, i64 8
  %t15 = load i64, ptr %t14, align 8
  %t17 = add i64 %t15, 23
  %t18 = call ptr @ignite.alloc(i64 %t17, i64 1)
  store ptr %t18, ptr %t16, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr @.str.1, i64 23, i1 false)
  %t19 = getelementptr inbounds i8, ptr %t18, i64 23
  call void @llvm.memcpy.p0.p0.i64(ptr %t19, ptr %t13, i64 %t15, i1 false)
  %t20 = getelementptr inbounds i8, ptr %t16, i64 8
  store i64 %t17, ptr %t20, align 8
  %t21 = getelementptr inbounds i8, ptr %t16, i64 16
  store i64 %t17, ptr %t21, align 8
  call void @ignite_rt_log(ptr %t16)
  br label %L11
L11:
  call void @ignite_rt_guard_pop()
  call void @ignite_rt_tcp_release(i64 %t6)
  call void @ignite_rt_task_end()
  ret void
}

define void @"tests.bounds.over_pipe"(ptr %sret, i64 %p0) !dbg !9 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t15 = alloca [24 x i8], align 8
  %t16 = alloca [40 x i8], align 8
  %t26 = alloca [8 x i8], align 8
  %t28 = alloca [8 x i8], align 8
  %t35 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !10
  %t2 = load i64, ptr %t1, align 8, !dbg !12
  call void @"tests.bounds.relay<PipeStream>"(ptr %t3, i64 %t2), !dbg !11
  %t4 = load i32, ptr %t3, align 4, !dbg !11
  %t5 = icmp eq i32 %t4, 1, !dbg !11
  br i1 %t5, label %L6, label %L7, !dbg !11
L6:
  store i32 1, ptr %sret, align 4, !dbg !11
  %t8 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !11
  %t9 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !11
  call void @llvm.memcpy.p0.p0.i64(ptr %t9, ptr %t8, i64 32, i1 false), !dbg !11
  ret void, !dbg !11
L7:
  %t10 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !11
  %t11 = load i64, ptr %t1, align 8, !dbg !14
  store ptr @.str.0, ptr %t12, align 8, !dbg !15
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !15
  store i64 5, ptr %t13, align 8, !dbg !15
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !15
  store i64 5, ptr %t14, align 8, !dbg !15
  call void @"copy.Text"(ptr %t15, ptr %t12), !dbg !15
  call void @"tests.bounds.Echo.serve<PipeStream>"(ptr %t16, i64 %t11, ptr %t15), !dbg !13
  %t17 = load i32, ptr %t16, align 4, !dbg !13
  %t18 = icmp eq i32 %t17, 1, !dbg !13
  br i1 %t18, label %L19, label %L20, !dbg !13
L19:
  store i32 1, ptr %sret, align 4, !dbg !13
  %t21 = getelementptr inbounds i8, ptr %t16, i64 8, !dbg !13
  %t22 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !13
  call void @llvm.memcpy.p0.p0.i64(ptr %t22, ptr %t21, i64 32, i1 false), !dbg !13
  ret void, !dbg !13
L20:
  %t23 = getelementptr inbounds i8, ptr %t16, i64 8, !dbg !13
  %t24 = load i64, ptr %t1, align 8, !dbg !18
  %t25 = call i64 @"tests.bounds.same"(i64 %t24), !dbg !17
  store i64 %t25, ptr %t26, align 8, !dbg !16
  %t27 = load i64, ptr %t26, align 8, !dbg !20
  store i64 %t27, ptr %t28, align 8, !dbg !19
  %t29 = call i8 @ignite_rt_spawn(ptr @"spawn.tests.bounds.relay<PipeStream>.1", ptr %t28), !dbg !19
  %t30 = icmp eq i8 %t29, 0, !dbg !19
  br i1 %t30, label %L31, label %L32, !dbg !19
L31:
  call void @ignite_rt_tcp_release(i64 %t27), !dbg !19
  br label %L32, !dbg !19
L32:
  %t33 = load i64, ptr %t1, align 8, !dbg !23
  %t34 = call i64 @ignite_rt_pipe_received_batches(i64 %t33), !dbg !23
  store i32 0, ptr %t35, align 4, !dbg !22
  %t36 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !22
  store i64 %t34, ptr %t36, align 8, !dbg !22
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t35, i64 40, i1 false), !dbg !21
  ret void, !dbg !21
}

define internal void @"spawn.tests.bounds.Echo.serve<TcpStream>.2"(ptr %env) {
entry:
  %t1 = alloca [256 x i8], align 16
  %t9 = alloca [24 x i8], align 8
  %t10 = alloca [40 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t2 = call i32 @_setjmp(ptr %t1)
  %t3 = icmp eq i32 %t2, 0
  br i1 %t3, label %L4, label %L5
L5:
  call void @ignite_rt_task_end()
  ret void
L4:
  call void @ignite_rt_guard_push(ptr %t1)
  %t6 = load i64, ptr %env, align 8
  %t7 = getelementptr inbounds i8, ptr %env, i64 8
  %t8 = load ptr, ptr %t7, align 8
  call void @"copy.Text"(ptr %t9, ptr %t8)
  call void @ignite_rt_tcp_own(i64 %t6)
  call void @ignite_rt_spawn_ready()
  call void @"tests.bounds.Echo.serve<TcpStream>"(ptr %t10, i64 %t6, ptr %t9)
  %t11 = load i32, ptr %t10, align 4
  %t12 = icmp eq i32 %t11, 1
  br i1 %t12, label %L13, label %L14
L13:
  %t15 = getelementptr inbounds i8, ptr %t10, i64 8
  %t16 = load ptr, ptr %t15, align 8
  %t17 = getelementptr inbounds i8, ptr %t15, i64 8
  %t18 = load i64, ptr %t17, align 8
  %t20 = add i64 %t18, 23
  %t21 = call ptr @ignite.alloc(i64 %t20, i64 1)
  store ptr %t21, ptr %t19, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t21, ptr @.str.1, i64 23, i1 false)
  %t22 = getelementptr inbounds i8, ptr %t21, i64 23
  call void @llvm.memcpy.p0.p0.i64(ptr %t22, ptr %t16, i64 %t18, i1 false)
  %t23 = getelementptr inbounds i8, ptr %t19, i64 8
  store i64 %t20, ptr %t23, align 8
  %t24 = getelementptr inbounds i8, ptr %t19, i64 16
  store i64 %t20, ptr %t24, align 8
  call void @ignite_rt_log(ptr %t19)
  br label %L14
L14:
  call void @ignite_rt_guard_pop()
  call void @ignite_rt_tcp_release(i64 %t6)
  call void @ignite_rt_task_end()
  ret void
}

define void @"tests.bounds.Hub.main"(ptr %sret, ptr %p0) !dbg !24 {
entry:
  %t6 = alloca [40 x i8], align 8
  %t7 = alloca [8 x i8], align 8
  %t8 = alloca [32 x i8], align 8
  %t24 = alloca [8 x i8], align 8
  %t29 = alloca [16 x i8], align 8
  %t1 = call i64 @ignite.arena_mark(), !dbg !26
  br label %L2
L2:
  %t5 = load i64, ptr %p0, align 8, !dbg !28
  %t9 = call i32 @ignite_rt_tcp_accept(i64 %t5, ptr %t7, ptr %t8), !dbg !28
  %t10 = icmp ne i32 %t9, 0, !dbg !28
  br i1 %t10, label %L11, label %L12, !dbg !28
L11:
  store i32 1, ptr %t6, align 4, !dbg !28
  %t14 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !28
  call void @llvm.memcpy.p0.p0.i64(ptr %t14, ptr %t8, i64 32, i1 false), !dbg !28
  br label %L13, !dbg !28
L12:
  store i32 0, ptr %t6, align 4, !dbg !28
  %t15 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !28
  call void @llvm.memcpy.p0.p0.i64(ptr %t15, ptr %t7, i64 8, i1 false), !dbg !28
  br label %L13, !dbg !28
L13:
  %t16 = load i32, ptr %t6, align 4, !dbg !28
  %t17 = icmp eq i32 %t16, 1, !dbg !28
  br i1 %t17, label %L18, label %L19, !dbg !28
L18:
  store i32 1, ptr %sret, align 4, !dbg !28
  %t20 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !28
  %t21 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !28
  call void @llvm.memcpy.p0.p0.i64(ptr %t21, ptr %t20, i64 32, i1 false), !dbg !28
  ret void, !dbg !28
L19:
  %t22 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !28
  %t23 = load i64, ptr %t22, align 8, !dbg !28
  store i64 %t23, ptr %t24, align 8, !dbg !27
  %t25 = call i64 @ignite_rt_own_mark(), !dbg !27
  %t26 = load i64, ptr %t24, align 8, !dbg !27
  call void @ignite_rt_tcp_own(i64 %t26), !dbg !27
  %t27 = load i64, ptr %t24, align 8, !dbg !30
  %t28 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !31
  store i64 %t27, ptr %t29, align 8, !dbg !29
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !29
  store ptr %t28, ptr %t30, align 8, !dbg !29
  call void @ignite_rt_tcp_retain(i64 %t27), !dbg !29
  %t31 = call i8 @ignite_rt_spawn(ptr @"spawn.tests.bounds.Echo.serve<TcpStream>.2", ptr %t29), !dbg !29
  %t32 = icmp eq i8 %t31, 0, !dbg !29
  br i1 %t32, label %L33, label %L34, !dbg !29
L33:
  call void @ignite_rt_tcp_release(i64 %t27), !dbg !29
  br label %L34, !dbg !29
L34:
  call void @ignite_rt_tcp_release(i64 %t27), !dbg !29
  call void @ignite_rt_own_release(i64 %t25), !dbg !26
  br label %L3
L3:
  call void @ignite.arena_release(i64 %t1), !dbg !26
  br label %L2, !dbg !26
L4:
  ret void, !dbg !25
}

define linkonce_odr void @"tests.bounds.Echo.serve<TcpStream>"(ptr %sret, i64 %p0, ptr %p1) comdat !dbg !32 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !33
  %t2 = load i64, ptr %t1, align 8, !dbg !34
  %t5 = call i32 @ignite_rt_tcp_write_all(i64 %t2, ptr %p1, ptr %t4), !dbg !34
  %t6 = icmp ne i32 %t5, 0, !dbg !34
  br i1 %t6, label %L7, label %L8, !dbg !34
L7:
  store i32 1, ptr %t3, align 4, !dbg !34
  %t10 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !34
  call void @llvm.memcpy.p0.p0.i64(ptr %t10, ptr %t4, i64 32, i1 false), !dbg !34
  br label %L9, !dbg !34
L8:
  store i32 0, ptr %t3, align 4, !dbg !34
  br label %L9, !dbg !34
L9:
  %t11 = load i32, ptr %t3, align 4, !dbg !34
  %t12 = icmp eq i32 %t11, 1, !dbg !34
  br i1 %t12, label %L13, label %L14, !dbg !34
L13:
  store i32 1, ptr %sret, align 4, !dbg !34
  %t15 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !34
  %t16 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !34
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t15, i64 32, i1 false), !dbg !34
  ret void, !dbg !34
L14:
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !34
  %t18 = load i64, ptr %t1, align 8, !dbg !38
  call void @"tests.bounds.relay<TcpStream>"(ptr %t19, i64 %t18), !dbg !37
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t19, i64 40, i1 false), !dbg !36
  ret void, !dbg !36
}

define linkonce_odr void @"tests.bounds.Echo.serve<PipeStream>"(ptr %sret, i64 %p0, ptr %p1) comdat !dbg !39 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !40
  %t2 = load i64, ptr %t1, align 8, !dbg !41
  %t5 = call i32 @ignite_rt_pipe_write_all(i64 %t2, ptr %p1, ptr %t4), !dbg !41
  %t6 = icmp ne i32 %t5, 0, !dbg !41
  br i1 %t6, label %L7, label %L8, !dbg !41
L7:
  store i32 1, ptr %t3, align 4, !dbg !41
  %t10 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !41
  call void @llvm.memcpy.p0.p0.i64(ptr %t10, ptr %t4, i64 32, i1 false), !dbg !41
  br label %L9, !dbg !41
L8:
  store i32 0, ptr %t3, align 4, !dbg !41
  br label %L9, !dbg !41
L9:
  %t11 = load i32, ptr %t3, align 4, !dbg !41
  %t12 = icmp eq i32 %t11, 1, !dbg !41
  br i1 %t12, label %L13, label %L14, !dbg !41
L13:
  store i32 1, ptr %sret, align 4, !dbg !41
  %t15 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !41
  %t16 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !41
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t15, i64 32, i1 false), !dbg !41
  ret void, !dbg !41
L14:
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !41
  %t18 = load i64, ptr %t1, align 8, !dbg !45
  call void @"tests.bounds.relay<PipeStream>"(ptr %t19, i64 %t18), !dbg !44
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t19, i64 40, i1 false), !dbg !43
  ret void, !dbg !43
}

define linkonce_odr void @"tests.bounds.relay<PipeStream>"(ptr %sret, i64 %p0) comdat !dbg !46 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  %t20 = alloca [32 x i8], align 8
  %t21 = alloca [32 x i8], align 8
  %t42 = alloca [40 x i8], align 8
  %t43 = alloca [32 x i8], align 8
  %t57 = alloca [40 x i8], align 8
  %t58 = alloca [32 x i8], align 8
  %t72 = alloca [24 x i8], align 8
  %t76 = alloca [40 x i8], align 8
  %t77 = alloca [32 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !47
  %t2 = load i64, ptr %t1, align 8, !dbg !48
  %t5 = call i32 @ignite_rt_pipe_set_idle_timeout(i64 %t2, i64 1000, ptr %t4), !dbg !48
  %t6 = icmp ne i32 %t5, 0, !dbg !48
  br i1 %t6, label %L7, label %L8, !dbg !48
L7:
  store i32 1, ptr %t3, align 4, !dbg !48
  %t10 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t10, ptr %t4, i64 32, i1 false), !dbg !48
  br label %L9, !dbg !48
L8:
  store i32 0, ptr %t3, align 4, !dbg !48
  br label %L9, !dbg !48
L9:
  %t11 = load i32, ptr %t3, align 4, !dbg !48
  %t12 = icmp eq i32 %t11, 1, !dbg !48
  br i1 %t12, label %L13, label %L14, !dbg !48
L13:
  store i32 1, ptr %sret, align 4, !dbg !48
  %t15 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !48
  %t16 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t15, i64 32, i1 false), !dbg !48
  ret void, !dbg !48
L14:
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !48
  %t18 = load i64, ptr %t1, align 8, !dbg !51
  %t22 = call i32 @ignite_rt_pipe_read(i64 %t18, i64 4096, ptr %t20, ptr %t21), !dbg !51
  %t23 = icmp ne i32 %t22, 0, !dbg !51
  br i1 %t23, label %L24, label %L25, !dbg !51
L24:
  store i32 1, ptr %t19, align 4, !dbg !51
  %t27 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t27, ptr %t21, i64 32, i1 false), !dbg !51
  br label %L26, !dbg !51
L25:
  store i32 0, ptr %t19, align 4, !dbg !51
  %t28 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t28, ptr %t20, i64 32, i1 false), !dbg !51
  br label %L26, !dbg !51
L26:
  %t29 = load i32, ptr %t19, align 4, !dbg !51
  %t30 = icmp eq i32 %t29, 1, !dbg !51
  br i1 %t30, label %L31, label %L32, !dbg !51
L31:
  store i32 1, ptr %sret, align 4, !dbg !51
  %t33 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !51
  %t34 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t34, ptr %t33, i64 32, i1 false), !dbg !51
  ret void, !dbg !51
L32:
  %t35 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !51
  %t38 = load i32, ptr %t35, align 4, !dbg !50
  %t39 = icmp eq i32 %t38, 1, !dbg !50
  br i1 %t39, label %L40, label %L37, !dbg !50
L40:
  %t41 = load i64, ptr %t1, align 8, !dbg !54
  %t44 = call i32 @ignite_rt_pipe_close(i64 %t41, ptr %t43), !dbg !54
  %t45 = icmp ne i32 %t44, 0, !dbg !54
  br i1 %t45, label %L46, label %L47, !dbg !54
L46:
  store i32 1, ptr %t42, align 4, !dbg !54
  %t49 = getelementptr inbounds i8, ptr %t42, i64 8, !dbg !54
  call void @llvm.memcpy.p0.p0.i64(ptr %t49, ptr %t43, i64 32, i1 false), !dbg !54
  br label %L48, !dbg !54
L47:
  store i32 0, ptr %t42, align 4, !dbg !54
  br label %L48, !dbg !54
L48:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t42, i64 40, i1 false), !dbg !53
  ret void, !dbg !53
L50:
  br label %L36, !dbg !50
L37:
  %t52 = load i32, ptr %t35, align 4, !dbg !50
  %t53 = icmp eq i32 %t52, 0, !dbg !50
  br i1 %t53, label %L54, label %L51, !dbg !50
L54:
  %t55 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !50
  %t56 = load i64, ptr %t1, align 8, !dbg !55
  %t59 = call i32 @ignite_rt_pipe_write_all(i64 %t56, ptr %t55, ptr %t58), !dbg !55
  %t60 = icmp ne i32 %t59, 0, !dbg !55
  br i1 %t60, label %L61, label %L62, !dbg !55
L61:
  store i32 1, ptr %t57, align 4, !dbg !55
  %t64 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !55
  call void @llvm.memcpy.p0.p0.i64(ptr %t64, ptr %t58, i64 32, i1 false), !dbg !55
  br label %L63, !dbg !55
L62:
  store i32 0, ptr %t57, align 4, !dbg !55
  br label %L63, !dbg !55
L63:
  %t65 = load i32, ptr %t57, align 4, !dbg !55
  %t66 = icmp eq i32 %t65, 1, !dbg !55
  br i1 %t66, label %L67, label %L68, !dbg !55
L67:
  store i32 1, ptr %sret, align 4, !dbg !55
  %t69 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !55
  %t70 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !55
  call void @llvm.memcpy.p0.p0.i64(ptr %t70, ptr %t69, i64 32, i1 false), !dbg !55
  ret void, !dbg !55
L68:
  %t71 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !55
  br label %L36, !dbg !50
L51:
  store ptr @.str.2, ptr %t72, align 8, !dbg !50
  %t73 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !50
  store i64 26, ptr %t73, align 8, !dbg !50
  %t74 = getelementptr inbounds i8, ptr %t72, i64 16, !dbg !50
  store i64 26, ptr %t74, align 8, !dbg !50
  call void @ignite_rt_fail_match(ptr %t72, i64 5, i64 3), !dbg !50
  unreachable, !dbg !50
L36:
  %t75 = load i64, ptr %t1, align 8, !dbg !58
  %t78 = call i32 @ignite_rt_pipe_close(i64 %t75, ptr %t77), !dbg !58
  %t79 = icmp ne i32 %t78, 0, !dbg !58
  br i1 %t79, label %L80, label %L81, !dbg !58
L80:
  store i32 1, ptr %t76, align 4, !dbg !58
  %t83 = getelementptr inbounds i8, ptr %t76, i64 8, !dbg !58
  call void @llvm.memcpy.p0.p0.i64(ptr %t83, ptr %t77, i64 32, i1 false), !dbg !58
  br label %L82, !dbg !58
L81:
  store i32 0, ptr %t76, align 4, !dbg !58
  br label %L82, !dbg !58
L82:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t76, i64 40, i1 false), !dbg !57
  ret void, !dbg !57
}

define linkonce_odr void @"tests.bounds.relay<TcpStream>"(ptr %sret, i64 %p0) comdat !dbg !59 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [32 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  %t20 = alloca [32 x i8], align 8
  %t21 = alloca [32 x i8], align 8
  %t42 = alloca [40 x i8], align 8
  %t43 = alloca [32 x i8], align 8
  %t57 = alloca [40 x i8], align 8
  %t58 = alloca [32 x i8], align 8
  %t72 = alloca [24 x i8], align 8
  %t76 = alloca [40 x i8], align 8
  %t77 = alloca [32 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !60
  %t2 = load i64, ptr %t1, align 8, !dbg !61
  %t5 = call i32 @ignite_rt_tcp_set_idle_timeout(i64 %t2, i64 1000, ptr %t4), !dbg !61
  %t6 = icmp ne i32 %t5, 0, !dbg !61
  br i1 %t6, label %L7, label %L8, !dbg !61
L7:
  store i32 1, ptr %t3, align 4, !dbg !61
  %t10 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !61
  call void @llvm.memcpy.p0.p0.i64(ptr %t10, ptr %t4, i64 32, i1 false), !dbg !61
  br label %L9, !dbg !61
L8:
  store i32 0, ptr %t3, align 4, !dbg !61
  br label %L9, !dbg !61
L9:
  %t11 = load i32, ptr %t3, align 4, !dbg !61
  %t12 = icmp eq i32 %t11, 1, !dbg !61
  br i1 %t12, label %L13, label %L14, !dbg !61
L13:
  store i32 1, ptr %sret, align 4, !dbg !61
  %t15 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !61
  %t16 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !61
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t15, i64 32, i1 false), !dbg !61
  ret void, !dbg !61
L14:
  %t17 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !61
  %t18 = load i64, ptr %t1, align 8, !dbg !64
  %t22 = call i32 @ignite_rt_tcp_read(i64 %t18, i64 4096, ptr %t20, ptr %t21), !dbg !64
  %t23 = icmp ne i32 %t22, 0, !dbg !64
  br i1 %t23, label %L24, label %L25, !dbg !64
L24:
  store i32 1, ptr %t19, align 4, !dbg !64
  %t27 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !64
  call void @llvm.memcpy.p0.p0.i64(ptr %t27, ptr %t21, i64 32, i1 false), !dbg !64
  br label %L26, !dbg !64
L25:
  store i32 0, ptr %t19, align 4, !dbg !64
  %t28 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !64
  call void @llvm.memcpy.p0.p0.i64(ptr %t28, ptr %t20, i64 32, i1 false), !dbg !64
  br label %L26, !dbg !64
L26:
  %t29 = load i32, ptr %t19, align 4, !dbg !64
  %t30 = icmp eq i32 %t29, 1, !dbg !64
  br i1 %t30, label %L31, label %L32, !dbg !64
L31:
  store i32 1, ptr %sret, align 4, !dbg !64
  %t33 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !64
  %t34 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !64
  call void @llvm.memcpy.p0.p0.i64(ptr %t34, ptr %t33, i64 32, i1 false), !dbg !64
  ret void, !dbg !64
L32:
  %t35 = getelementptr inbounds i8, ptr %t19, i64 8, !dbg !64
  %t38 = load i32, ptr %t35, align 4, !dbg !63
  %t39 = icmp eq i32 %t38, 1, !dbg !63
  br i1 %t39, label %L40, label %L37, !dbg !63
L40:
  %t41 = load i64, ptr %t1, align 8, !dbg !67
  %t44 = call i32 @ignite_rt_tcp_close(i64 %t41, ptr %t43), !dbg !67
  %t45 = icmp ne i32 %t44, 0, !dbg !67
  br i1 %t45, label %L46, label %L47, !dbg !67
L46:
  store i32 1, ptr %t42, align 4, !dbg !67
  %t49 = getelementptr inbounds i8, ptr %t42, i64 8, !dbg !67
  call void @llvm.memcpy.p0.p0.i64(ptr %t49, ptr %t43, i64 32, i1 false), !dbg !67
  br label %L48, !dbg !67
L47:
  store i32 0, ptr %t42, align 4, !dbg !67
  br label %L48, !dbg !67
L48:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t42, i64 40, i1 false), !dbg !66
  ret void, !dbg !66
L50:
  br label %L36, !dbg !63
L37:
  %t52 = load i32, ptr %t35, align 4, !dbg !63
  %t53 = icmp eq i32 %t52, 0, !dbg !63
  br i1 %t53, label %L54, label %L51, !dbg !63
L54:
  %t55 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !63
  %t56 = load i64, ptr %t1, align 8, !dbg !68
  %t59 = call i32 @ignite_rt_tcp_write_all(i64 %t56, ptr %t55, ptr %t58), !dbg !68
  %t60 = icmp ne i32 %t59, 0, !dbg !68
  br i1 %t60, label %L61, label %L62, !dbg !68
L61:
  store i32 1, ptr %t57, align 4, !dbg !68
  %t64 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !68
  call void @llvm.memcpy.p0.p0.i64(ptr %t64, ptr %t58, i64 32, i1 false), !dbg !68
  br label %L63, !dbg !68
L62:
  store i32 0, ptr %t57, align 4, !dbg !68
  br label %L63, !dbg !68
L63:
  %t65 = load i32, ptr %t57, align 4, !dbg !68
  %t66 = icmp eq i32 %t65, 1, !dbg !68
  br i1 %t66, label %L67, label %L68, !dbg !68
L67:
  store i32 1, ptr %sret, align 4, !dbg !68
  %t69 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !68
  %t70 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !68
  call void @llvm.memcpy.p0.p0.i64(ptr %t70, ptr %t69, i64 32, i1 false), !dbg !68
  ret void, !dbg !68
L68:
  %t71 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !68
  br label %L36, !dbg !63
L51:
  store ptr @.str.2, ptr %t72, align 8, !dbg !63
  %t73 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !63
  store i64 26, ptr %t73, align 8, !dbg !63
  %t74 = getelementptr inbounds i8, ptr %t72, i64 16, !dbg !63
  store i64 26, ptr %t74, align 8, !dbg !63
  call void @ignite_rt_fail_match(ptr %t72, i64 5, i64 3), !dbg !63
  unreachable, !dbg !63
L36:
  %t75 = load i64, ptr %t1, align 8, !dbg !71
  %t78 = call i32 @ignite_rt_tcp_close(i64 %t75, ptr %t77), !dbg !71
  %t79 = icmp ne i32 %t78, 0, !dbg !71
  br i1 %t79, label %L80, label %L81, !dbg !71
L80:
  store i32 1, ptr %t76, align 4, !dbg !71
  %t83 = getelementptr inbounds i8, ptr %t76, i64 8, !dbg !71
  call void @llvm.memcpy.p0.p0.i64(ptr %t83, ptr %t77, i64 32, i1 false), !dbg !71
  br label %L82, !dbg !71
L81:
  store i32 0, ptr %t76, align 4, !dbg !71
  br label %L82, !dbg !71
L82:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t76, i64 40, i1 false), !dbg !70
  ret void, !dbg !70
}

define internal void @"copy.Text"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @ignite.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @ignite.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

!llvm.dbg.cu = !{!1}
!llvm.module.flags = !{!72, !73}
!0 = !DIFile(filename: "tests.bounds.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "bounds.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "tests.bounds.over_tcp", linkageName: "tests.bounds.over_tcp", scope: !3, file: !3, line: 19, type: !2, scopeLine: 19, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 19, column: 96, scope: !4)
!6 = !DILocation(line: 19, column: 102, scope: !4)
!7 = distinct !DISubprogram(name: "tests.bounds.same", linkageName: "tests.bounds.same", scope: !3, file: !3, line: 21, type: !2, scopeLine: 21, spFlags: DISPFlagDefinition, unit: !1)
!8 = !DILocation(line: 21, column: 42, scope: !7)
!9 = distinct !DISubprogram(name: "tests.bounds.over_pipe", linkageName: "tests.bounds.over_pipe", scope: !3, file: !3, line: 23, type: !2, scopeLine: 23, spFlags: DISPFlagDefinition, unit: !1)
!10 = !DILocation(line: 23, column: 86, scope: !9)
!11 = !DILocation(line: 24, column: 3, scope: !9)
!12 = !DILocation(line: 24, column: 9, scope: !9)
!13 = !DILocation(line: 25, column: 3, scope: !9)
!14 = !DILocation(line: 25, column: 14, scope: !9)
!15 = !DILocation(line: 25, column: 19, scope: !9)
!16 = !DILocation(line: 26, column: 3, scope: !9)
!17 = !DILocation(line: 26, column: 15, scope: !9)
!18 = !DILocation(line: 26, column: 20, scope: !9)
!19 = !DILocation(line: 27, column: 3, scope: !9)
!20 = !DILocation(line: 27, column: 15, scope: !9)
!21 = !DILocation(line: 28, column: 3, scope: !9)
!22 = !DILocation(line: 28, column: 10, scope: !9)
!23 = !DILocation(line: 28, column: 13, scope: !9)
!24 = distinct !DISubprogram(name: "tests.bounds.Hub.main", linkageName: "tests.bounds.Hub.main", scope: !3, file: !3, line: 37, type: !2, scopeLine: 37, spFlags: DISPFlagDefinition, unit: !1)
!25 = !DILocation(line: 37, column: 49, scope: !24)
!26 = !DILocation(line: 38, column: 3, scope: !24)
!27 = !DILocation(line: 39, column: 5, scope: !24)
!28 = !DILocation(line: 39, column: 18, scope: !24)
!29 = !DILocation(line: 40, column: 5, scope: !24)
!30 = !DILocation(line: 40, column: 22, scope: !24)
!31 = !DILocation(line: 40, column: 30, scope: !24)
!32 = distinct !DISubprogram(name: "tests.bounds.Echo.serve<TcpStream>", linkageName: "tests.bounds.Echo.serve<TcpStream>", scope: !3, file: !3, line: 13, type: !2, scopeLine: 13, spFlags: DISPFlagDefinition, unit: !1)
!33 = !DILocation(line: 13, column: 116, scope: !32)
!34 = !DILocation(line: 14, column: 5, scope: !32)
!35 = !DILocation(line: 14, column: 22, scope: !32)
!36 = !DILocation(line: 15, column: 5, scope: !32)
!37 = !DILocation(line: 15, column: 12, scope: !32)
!38 = !DILocation(line: 15, column: 18, scope: !32)
!39 = distinct !DISubprogram(name: "tests.bounds.Echo.serve<PipeStream>", linkageName: "tests.bounds.Echo.serve<PipeStream>", scope: !3, file: !3, line: 13, type: !2, scopeLine: 13, spFlags: DISPFlagDefinition, unit: !1)
!40 = !DILocation(line: 13, column: 116, scope: !39)
!41 = !DILocation(line: 14, column: 5, scope: !39)
!42 = !DILocation(line: 14, column: 22, scope: !39)
!43 = !DILocation(line: 15, column: 5, scope: !39)
!44 = !DILocation(line: 15, column: 12, scope: !39)
!45 = !DILocation(line: 15, column: 18, scope: !39)
!46 = distinct !DISubprogram(name: "tests.bounds.relay<PipeStream>", linkageName: "tests.bounds.relay<PipeStream>", scope: !3, file: !3, line: 3, type: !2, scopeLine: 3, spFlags: DISPFlagDefinition, unit: !1)
!47 = !DILocation(line: 3, column: 98, scope: !46)
!48 = !DILocation(line: 4, column: 3, scope: !46)
!49 = !DILocation(line: 4, column: 27, scope: !46)
!50 = !DILocation(line: 5, column: 3, scope: !46)
!51 = !DILocation(line: 5, column: 9, scope: !46)
!52 = !DILocation(line: 5, column: 21, scope: !46)
!53 = !DILocation(line: 6, column: 12, scope: !46)
!54 = !DILocation(line: 6, column: 19, scope: !46)
!55 = !DILocation(line: 7, column: 20, scope: !46)
!56 = !DILocation(line: 7, column: 37, scope: !46)
!57 = !DILocation(line: 9, column: 3, scope: !46)
!58 = !DILocation(line: 9, column: 10, scope: !46)
!59 = distinct !DISubprogram(name: "tests.bounds.relay<TcpStream>", linkageName: "tests.bounds.relay<TcpStream>", scope: !3, file: !3, line: 3, type: !2, scopeLine: 3, spFlags: DISPFlagDefinition, unit: !1)
!60 = !DILocation(line: 3, column: 98, scope: !59)
!61 = !DILocation(line: 4, column: 3, scope: !59)
!62 = !DILocation(line: 4, column: 27, scope: !59)
!63 = !DILocation(line: 5, column: 3, scope: !59)
!64 = !DILocation(line: 5, column: 9, scope: !59)
!65 = !DILocation(line: 5, column: 21, scope: !59)
!66 = !DILocation(line: 6, column: 12, scope: !59)
!67 = !DILocation(line: 6, column: 19, scope: !59)
!68 = !DILocation(line: 7, column: 20, scope: !59)
!69 = !DILocation(line: 7, column: 37, scope: !59)
!70 = !DILocation(line: 9, column: 3, scope: !59)
!71 = !DILocation(line: 9, column: 10, scope: !59)
!72 = !{i32 7, !"Dwarf Version", i32 5}
!73 = !{i32 2, !"Debug Info Version", i32 3}
