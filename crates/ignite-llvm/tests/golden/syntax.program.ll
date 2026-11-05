target triple = "x86_64-pc-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
; ModuleID = 'tests.syntax'
source_filename = "tests.syntax"

@ignite.empty = private constant [8 x i8] zeroinitializer, align 8
@.str.16 = private unnamed_addr constant [1 x i8] c" ", align 1
@.str.3 = private unnamed_addr constant [1 x i8] c"!", align 1
@.str.4 = private unnamed_addr constant [1 x i8] c"*", align 1
@.str.1 = private unnamed_addr constant [1 x i8] c"+", align 1
@.str.24 = private unnamed_addr constant [1 x i8] c"-", align 1
@.str.18 = private unnamed_addr constant [1 x i8] c"a", align 1
@.str.32 = private unnamed_addr constant [6 x i8] c"a b c ", align 1
@.str.28 = private unnamed_addr constant [1 x i8] c"b", align 1
@.str.8 = private unnamed_addr constant [4 x i8] c"base", align 1
@.str.29 = private unnamed_addr constant [1 x i8] c"c", align 1
@.str.38 = private unnamed_addr constant [5 x i8] c"close", align 1
@.str.36 = private unnamed_addr constant [5 x i8] c"code ", align 1
@.str.37 = private unnamed_addr constant [8 x i8] c"continue", align 1
@.str.19 = private unnamed_addr constant [1 x i8] c"e", align 1
@.str.31 = private unnamed_addr constant [5 x i8] c"false", align 1
@.str.34 = private unnamed_addr constant [1 x i8] c"h", align 1
@.str.33 = private unnamed_addr constant [3 x i8] c"hey", align 1
@.str.20 = private unnamed_addr constant [1 x i8] c"i", align 1
@.str.35 = private unnamed_addr constant [1 x i8] c"l", align 1
@.str.40 = private unnamed_addr constant [9 x i8] c"light off", align 1
@.str.41 = private unnamed_addr constant [8 x i8] c"light on", align 1
@.str.5 = private unnamed_addr constant [6 x i8] c"lowest", align 1
@.str.14 = private unnamed_addr constant [4 x i8] c"many", align 1
@.str.10 = private unnamed_addr constant [4 x i8] c"none", align 1
@.str.27 = private unnamed_addr constant [3 x i8] c"now", align 1
@.str.21 = private unnamed_addr constant [1 x i8] c"o", align 1
@.str.17 = private unnamed_addr constant [11 x i8] c"off or none", align 1
@.str.11 = private unnamed_addr constant [4 x i8] c"one ", align 1
@.str.9 = private unnamed_addr constant [5 x i8] c"other", align 1
@.str.6 = private unnamed_addr constant [6 x i8] c"other ", align 1
@.str.15 = private unnamed_addr constant [5 x i8] c"pair ", align 1
@.str.2 = private unnamed_addr constant [4 x i8] c"quit", align 1
@.str.12 = private unnamed_addr constant [18 x i8] c"quit with argument", align 1
@.str.25 = private unnamed_addr constant [5 x i8] c"seven", align 1
@.str.7 = private unnamed_addr constant [5 x i8] c"small", align 1
@.str.39 = private unnamed_addr constant [1 x i8] c"t", align 1
@.str.0 = private unnamed_addr constant [26 x i8] c"tests/golden/syntax.ignite", align 1
@.str.30 = private unnamed_addr constant [4 x i8] c"true", align 1
@.str.13 = private unnamed_addr constant [4 x i8] c"two ", align 1
@.str.22 = private unnamed_addr constant [1 x i8] c"u", align 1
@.str.26 = private unnamed_addr constant [1 x i8] c"x", align 1
@.str.23 = private unnamed_addr constant [4 x i8] c"zero", align 1

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
declare void @"ignite_rt_fail_arith"(ptr, ptr, i64, i64, i64, i64)
declare void @"ignite_rt_fail_match"(ptr, i64, i64)
declare i32 @memcmp(ptr, ptr, i64)

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

define void @"tests.syntax.Reply.action"(ptr %sret) !dbg !4 {
entry:
  %t1 = alloca [8 x i8], align 8
  store i32 0, ptr %t1, align 4, !dbg !5
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 8, i1 false), !dbg !5
  ret void, !dbg !5
}

define i64 @"tests.syntax.Reply.code"() !dbg !6 {
entry:
  %t6 = alloca [24 x i8], align 8
  %t9 = alloca [24 x i8], align 8
  %t1 = call i64 @"tests.syntax.BASE"(), !dbg !7
  %t2 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t1, i64 1), !dbg !7
  %t3 = extractvalue { i64, i1 } %t2, 1, !dbg !7
  br i1 %t3, label %L4, label %L5, !dbg !7
L4:
  store ptr @.str.0, ptr %t6, align 8, !dbg !7
  %t7 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !7
  store i64 26, ptr %t7, align 8, !dbg !7
  %t8 = getelementptr inbounds i8, ptr %t6, i64 16, !dbg !7
  store i64 26, ptr %t8, align 8, !dbg !7
  store ptr @.str.1, ptr %t9, align 8, !dbg !7
  %t10 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !7
  store i64 1, ptr %t10, align 8, !dbg !7
  %t11 = getelementptr inbounds i8, ptr %t9, i64 16, !dbg !7
  store i64 1, ptr %t11, align 8, !dbg !7
  call void @ignite_rt_fail_arith(ptr %t9, ptr %t6, i64 12, i64 15, i64 %t1, i64 1), !dbg !7
  unreachable, !dbg !7
L5:
  %t12 = extractvalue { i64, i1 } %t2, 0, !dbg !7
  ret i64 %t12, !dbg !7
}

define i64 @"tests.syntax.BASE"() !dbg !9 {
entry:
  ret i64 41, !dbg !10
}

define void @"tests.syntax.QUIT"(ptr %sret) !dbg !11 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t4 = alloca [24 x i8], align 8
  store ptr @.str.2, ptr %t1, align 8, !dbg !12
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !12
  store i64 4, ptr %t2, align 8, !dbg !12
  %t3 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !12
  store i64 4, ptr %t3, align 8, !dbg !12
  call void @"copy.Text"(ptr %t4, ptr %t1), !dbg !12
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t4, i64 24, i1 false), !dbg !12
  ret void, !dbg !12
}

define i8 @"tests.syntax.SPACE"() !dbg !13 {
entry:
  ret i8 32, !dbg !14
}

define i64 @"tests.syntax.lowest"() !dbg !15 {
entry:
  ret i64 3, !dbg !16
}

define void @"tests.syntax.shout"(ptr %sret, ptr %p0) !dbg !17 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t2 = load ptr, ptr %p0, align 8, !dbg !18
  %t3 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !18
  %t4 = load i64, ptr %t3, align 8, !dbg !18
  %t5 = add i64 %t4, 1, !dbg !18
  %t6 = call ptr @ignite.alloc(i64 %t5, i64 1), !dbg !18
  store ptr %t6, ptr %t1, align 8, !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t2, i64 %t4, i1 false), !dbg !18
  %t7 = getelementptr inbounds i8, ptr %t6, i64 %t4, !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr @.str.3, i64 1, i1 false), !dbg !18
  %t8 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !18
  store i64 %t5, ptr %t8, align 8, !dbg !18
  %t9 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !18
  store i64 %t5, ptr %t9, align 8, !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !18
  ret void, !dbg !18
}

define i64 @"tests.syntax.twice"(i64 %p0) !dbg !19 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !20
  %t2 = load i64, ptr %t1, align 8, !dbg !20
  %t3 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %t2, i64 2), !dbg !20
  %t4 = extractvalue { i64, i1 } %t3, 1, !dbg !20
  br i1 %t4, label %L5, label %L6, !dbg !20
L5:
  store ptr @.str.0, ptr %t7, align 8, !dbg !20
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !20
  store i64 26, ptr %t8, align 8, !dbg !20
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !20
  store i64 26, ptr %t9, align 8, !dbg !20
  store ptr @.str.4, ptr %t10, align 8, !dbg !20
  %t11 = getelementptr inbounds i8, ptr %t10, i64 8, !dbg !20
  store i64 1, ptr %t11, align 8, !dbg !20
  %t12 = getelementptr inbounds i8, ptr %t10, i64 16, !dbg !20
  store i64 1, ptr %t12, align 8, !dbg !20
  call void @ignite_rt_fail_arith(ptr %t10, ptr %t7, i64 30, i64 27, i64 %t2, i64 2), !dbg !20
  unreachable, !dbg !20
L6:
  %t13 = extractvalue { i64, i1 } %t3, 0, !dbg !20
  ret i64 %t13, !dbg !20
}

define void @"tests.syntax.lower_case"(ptr %sret, i64 %p0) !dbg !22 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t9 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t14 = alloca [8 x i8], align 8
  %t15 = alloca [24 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !23
  %t2 = load i64, ptr %t1, align 8, !dbg !25
  %t6 = call i64 @"tests.syntax.lowest"(), !dbg !24
  %t7 = icmp eq i64 %t2, %t6, !dbg !24
  br i1 %t7, label %L8, label %L5, !dbg !24
L8:
  store ptr @.str.5, ptr %t9, align 8, !dbg !26
  %t10 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !26
  store i64 6, ptr %t10, align 8, !dbg !26
  %t11 = getelementptr inbounds i8, ptr %t9, i64 16, !dbg !26
  store i64 6, ptr %t11, align 8, !dbg !26
  call void @"copy.Text"(ptr %t12, ptr %t9), !dbg !26
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t12, i64 24, i1 false), !dbg !24
  br label %L4, !dbg !24
L5:
  store i64 %t2, ptr %t14, align 8, !dbg !24
  %t16 = load i64, ptr %t14, align 8, !dbg !28
  call void @ignite_rt_int_to_text(ptr %t17, i64 %t16), !dbg !28
  %t18 = load ptr, ptr %t17, align 8, !dbg !27
  %t19 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !27
  %t20 = load i64, ptr %t19, align 8, !dbg !27
  %t21 = add i64 %t20, 6, !dbg !27
  %t22 = call ptr @ignite.alloc(i64 %t21, i64 1), !dbg !27
  store ptr %t22, ptr %t15, align 8, !dbg !27
  call void @llvm.memcpy.p0.p0.i64(ptr %t22, ptr @.str.6, i64 6, i1 false), !dbg !27
  %t23 = getelementptr inbounds i8, ptr %t22, i64 6, !dbg !27
  call void @llvm.memcpy.p0.p0.i64(ptr %t23, ptr %t18, i64 %t20, i1 false), !dbg !27
  %t24 = getelementptr inbounds i8, ptr %t15, i64 8, !dbg !27
  store i64 %t21, ptr %t24, align 8, !dbg !27
  %t25 = getelementptr inbounds i8, ptr %t15, i64 16, !dbg !27
  store i64 %t21, ptr %t25, align 8, !dbg !27
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t15, i64 24, i1 false), !dbg !24
  br label %L4, !dbg !24
L13:
  store ptr @.str.0, ptr %t17, align 8, !dbg !24
  %t26 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !24
  store i64 26, ptr %t26, align 8, !dbg !24
  %t27 = getelementptr inbounds i8, ptr %t17, i64 16, !dbg !24
  store i64 26, ptr %t27, align 8, !dbg !24
  call void @ignite_rt_fail_match(ptr %t17, i64 33, i64 3), !dbg !24
  unreachable, !dbg !24
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t3, i64 24, i1 false), !dbg !23
  ret void, !dbg !23
}

define void @"tests.syntax.swap"(ptr %sret, ptr %p0) !dbg !29 {
entry:
  %t1 = alloca [32 x i8], align 8
  %t5 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [32 x i8], align 8
  call void @"copy.(Int,Text)"(ptr %t1, ptr %p0), !dbg !31
  %t4 = load i64, ptr %t1, align 8, !dbg !31
  store i64 %t4, ptr %t5, align 8, !dbg !31
  %t6 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !31
  br label %L2, !dbg !31
L3:
  store ptr @.str.0, ptr %t7, align 8, !dbg !31
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !31
  store i64 26, ptr %t8, align 8, !dbg !31
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !31
  store i64 26, ptr %t9, align 8, !dbg !31
  call void @ignite_rt_fail_match(ptr %t7, i64 40, i64 7), !dbg !31
  unreachable, !dbg !31
L2:
  call void @"copy.Text"(ptr %t10, ptr %t6), !dbg !33
  %t11 = load i64, ptr %t5, align 8, !dbg !35
  %t12 = getelementptr inbounds i8, ptr %t10, i64 24, !dbg !33
  store i64 %t11, ptr %t12, align 8, !dbg !33
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t10, i64 32, i1 false), !dbg !30
  ret void, !dbg !30
}

define void @"tests.syntax.describe"(ptr %sret, i64 %p0) !dbg !36 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t15 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t25 = alloca [24 x i8], align 8
  %t28 = alloca [24 x i8], align 8
  %t29 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !37
  %t2 = load i64, ptr %t1, align 8, !dbg !39
  %t8 = icmp eq i64 %t2, 0, !dbg !38
  br i1 %t8, label %L9, label %L7, !dbg !38
L9:
  br label %L6, !dbg !38
L7:
  %t10 = icmp eq i64 %t2, 1, !dbg !38
  br i1 %t10, label %L11, label %L5, !dbg !38
L11:
  br label %L6, !dbg !38
L6:
  store ptr @.str.7, ptr %t12, align 8, !dbg !40
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !40
  store i64 5, ptr %t13, align 8, !dbg !40
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !40
  store i64 5, ptr %t14, align 8, !dbg !40
  call void @"copy.Text"(ptr %t15, ptr %t12), !dbg !40
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t15, i64 24, i1 false), !dbg !38
  br label %L4, !dbg !38
L5:
  %t17 = call i64 @"tests.syntax.BASE"(), !dbg !38
  %t18 = icmp eq i64 %t2, %t17, !dbg !38
  br i1 %t18, label %L19, label %L16, !dbg !38
L19:
  store ptr @.str.8, ptr %t20, align 8, !dbg !41
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !41
  store i64 4, ptr %t21, align 8, !dbg !41
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !41
  store i64 4, ptr %t22, align 8, !dbg !41
  call void @"copy.Text"(ptr %t23, ptr %t20), !dbg !41
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t23, i64 24, i1 false), !dbg !38
  br label %L4, !dbg !38
L16:
  store ptr @.str.9, ptr %t25, align 8, !dbg !42
  %t26 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !42
  store i64 5, ptr %t26, align 8, !dbg !42
  %t27 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !42
  store i64 5, ptr %t27, align 8, !dbg !42
  call void @"copy.Text"(ptr %t28, ptr %t25), !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t28, i64 24, i1 false), !dbg !38
  br label %L4, !dbg !38
L24:
  store ptr @.str.0, ptr %t29, align 8, !dbg !38
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !38
  store i64 26, ptr %t30, align 8, !dbg !38
  %t31 = getelementptr inbounds i8, ptr %t29, i64 16, !dbg !38
  store i64 26, ptr %t31, align 8, !dbg !38
  call void @ignite_rt_fail_match(ptr %t29, i64 45, i64 3), !dbg !38
  unreachable, !dbg !38
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t3, i64 24, i1 false), !dbg !37
  ret void, !dbg !37
}

define internal i1 @ignite.text_eq(ptr %a, ptr %b) alwaysinline {
entry:
  %lenpa = getelementptr inbounds i8, ptr %a, i64 8
  %lena = load i64, ptr %lenpa, align 8
  %lenpb = getelementptr inbounds i8, ptr %b, i64 8
  %lenb = load i64, ptr %lenpb, align 8
  %same = icmp eq i64 %lena, %lenb
  %some = icmp sgt i64 %lena, 0
  %both = and i1 %same, %some
  br i1 %both, label %bytes, label %done
bytes:
  %da = load ptr, ptr %a, align 8
  %db = load ptr, ptr %b, align 8
  %order = call i32 @memcmp(ptr %da, ptr %db, i64 %lena)
  %equal = icmp eq i32 %order, 0
  br label %done
done:
  %eq = phi i1 [ %same, %entry ], [ %equal, %bytes ]
  ret i1 %eq
}

define void @"tests.syntax.classify"(ptr %sret, ptr %p0) !dbg !43 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t11 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t33 = alloca [24 x i8], align 8
  %t37 = alloca [24 x i8], align 8
  %t40 = alloca [24 x i8], align 8
  %t48 = alloca [24 x i8], align 8
  %t63 = alloca [24 x i8], align 8
  %t66 = alloca [24 x i8], align 8
  %t67 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !45
  %t5 = load i64, ptr %t4, align 8, !dbg !45
  %t6 = icmp eq i64 %t5, 0, !dbg !45
  br i1 %t6, label %L7, label %L3, !dbg !45
L7:
  store ptr @.str.10, ptr %t8, align 8, !dbg !47
  %t9 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !47
  store i64 4, ptr %t9, align 8, !dbg !47
  %t10 = getelementptr inbounds i8, ptr %t8, i64 16, !dbg !47
  store i64 4, ptr %t10, align 8, !dbg !47
  call void @"copy.Text"(ptr %t11, ptr %t8), !dbg !47
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t11, i64 24, i1 false), !dbg !45
  br label %L2, !dbg !45
L3:
  %t13 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !45
  %t14 = load i64, ptr %t13, align 8, !dbg !45
  %t15 = icmp eq i64 %t14, 1, !dbg !45
  br i1 %t15, label %L16, label %L12, !dbg !45
L16:
  %t17 = load ptr, ptr %p0, align 8, !dbg !45
  %t19 = load ptr, ptr %t17, align 8, !dbg !48
  %t20 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !48
  %t21 = load i64, ptr %t20, align 8, !dbg !48
  %t22 = add i64 %t21, 4, !dbg !48
  %t23 = call ptr @ignite.alloc(i64 %t22, i64 1), !dbg !48
  store ptr %t23, ptr %t18, align 8, !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t23, ptr @.str.11, i64 4, i1 false), !dbg !48
  %t24 = getelementptr inbounds i8, ptr %t23, i64 4, !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t24, ptr %t19, i64 %t21, i1 false), !dbg !48
  %t25 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !48
  store i64 %t22, ptr %t25, align 8, !dbg !48
  %t26 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !48
  store i64 %t22, ptr %t26, align 8, !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t18, i64 24, i1 false), !dbg !45
  br label %L2, !dbg !45
L12:
  %t28 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !45
  %t29 = load i64, ptr %t28, align 8, !dbg !45
  %t30 = icmp eq i64 %t29, 2, !dbg !45
  br i1 %t30, label %L31, label %L27, !dbg !45
L31:
  %t32 = load ptr, ptr %p0, align 8, !dbg !45
  call void @"tests.syntax.QUIT"(ptr %t33), !dbg !45
  %t34 = call i1 @ignite.text_eq(ptr %t32, ptr %t33), !dbg !45
  br i1 %t34, label %L35, label %L27, !dbg !45
L35:
  %t36 = getelementptr inbounds i8, ptr %t32, i64 24, !dbg !45
  store ptr @.str.12, ptr %t37, align 8, !dbg !50
  %t38 = getelementptr inbounds i8, ptr %t37, i64 8, !dbg !50
  store i64 18, ptr %t38, align 8, !dbg !50
  %t39 = getelementptr inbounds i8, ptr %t37, i64 16, !dbg !50
  store i64 18, ptr %t39, align 8, !dbg !50
  call void @"copy.Text"(ptr %t40, ptr %t37), !dbg !50
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t40, i64 24, i1 false), !dbg !45
  br label %L2, !dbg !45
L27:
  %t42 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !45
  %t43 = load i64, ptr %t42, align 8, !dbg !45
  %t44 = icmp eq i64 %t43, 2, !dbg !45
  br i1 %t44, label %L45, label %L41, !dbg !45
L45:
  %t46 = load ptr, ptr %p0, align 8, !dbg !45
  %t47 = getelementptr inbounds i8, ptr %t46, i64 24, !dbg !45
  %t49 = load ptr, ptr %t46, align 8, !dbg !51
  %t50 = getelementptr inbounds i8, ptr %t46, i64 8, !dbg !51
  %t51 = load i64, ptr %t50, align 8, !dbg !51
  %t52 = load ptr, ptr %t47, align 8, !dbg !51
  %t53 = getelementptr inbounds i8, ptr %t47, i64 8, !dbg !51
  %t54 = load i64, ptr %t53, align 8, !dbg !51
  %t55 = add i64 %t51, %t54, !dbg !51
  %t56 = add i64 %t55, 4, !dbg !51
  %t57 = call ptr @ignite.alloc(i64 %t56, i64 1), !dbg !51
  store ptr %t57, ptr %t48, align 8, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t57, ptr @.str.13, i64 4, i1 false), !dbg !51
  %t58 = getelementptr inbounds i8, ptr %t57, i64 4, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t58, ptr %t49, i64 %t51, i1 false), !dbg !51
  %t59 = getelementptr inbounds i8, ptr %t58, i64 %t51, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t59, ptr %t52, i64 %t54, i1 false), !dbg !51
  %t60 = getelementptr inbounds i8, ptr %t48, i64 8, !dbg !51
  store i64 %t56, ptr %t60, align 8, !dbg !51
  %t61 = getelementptr inbounds i8, ptr %t48, i64 16, !dbg !51
  store i64 %t56, ptr %t61, align 8, !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t48, i64 24, i1 false), !dbg !45
  br label %L2, !dbg !45
L41:
  store ptr @.str.14, ptr %t63, align 8, !dbg !54
  %t64 = getelementptr inbounds i8, ptr %t63, i64 8, !dbg !54
  store i64 4, ptr %t64, align 8, !dbg !54
  %t65 = getelementptr inbounds i8, ptr %t63, i64 16, !dbg !54
  store i64 4, ptr %t65, align 8, !dbg !54
  call void @"copy.Text"(ptr %t66, ptr %t63), !dbg !54
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t66, i64 24, i1 false), !dbg !45
  br label %L2, !dbg !45
L62:
  store ptr @.str.0, ptr %t67, align 8, !dbg !45
  %t68 = getelementptr inbounds i8, ptr %t67, i64 8, !dbg !45
  store i64 26, ptr %t68, align 8, !dbg !45
  %t69 = getelementptr inbounds i8, ptr %t67, i64 16, !dbg !45
  store i64 26, ptr %t69, align 8, !dbg !45
  call void @ignite_rt_fail_match(ptr %t67, i64 53, i64 3), !dbg !45
  unreachable, !dbg !45
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !44
  ret void, !dbg !44
}

define void @"tests.syntax.nested"(ptr %sret, ptr %p0) !dbg !55 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t19 = alloca [8 x i8], align 8
  %t22 = alloca [8 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t25 = alloca [24 x i8], align 8
  %t58 = alloca [24 x i8], align 8
  %t69 = alloca [24 x i8], align 8
  %t72 = alloca [24 x i8], align 8
  %t73 = alloca [24 x i8], align 8
  %t4 = load i32, ptr %p0, align 4, !dbg !57
  %t5 = icmp eq i32 %t4, 0, !dbg !57
  br i1 %t5, label %L6, label %L3, !dbg !57
L6:
  %t7 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !57
  %t8 = load i8, ptr %t7, align 1, !dbg !57
  %t9 = trunc i8 %t8 to i1, !dbg !57
  %t10 = icmp eq i1 %t9, true, !dbg !57
  br i1 %t10, label %L11, label %L3, !dbg !57
L11:
  %t12 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !57
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !57
  %t14 = load i64, ptr %t13, align 8, !dbg !57
  %t15 = icmp eq i64 %t14, 2, !dbg !57
  br i1 %t15, label %L16, label %L3, !dbg !57
L16:
  %t17 = load ptr, ptr %t12, align 8, !dbg !57
  %t18 = load i64, ptr %t17, align 8, !dbg !57
  store i64 %t18, ptr %t19, align 8, !dbg !57
  %t20 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !57
  %t21 = load i64, ptr %t20, align 8, !dbg !57
  store i64 %t21, ptr %t22, align 8, !dbg !57
  %t24 = load i64, ptr %t19, align 8, !dbg !60
  call void @ignite_rt_int_to_text(ptr %t25, i64 %t24), !dbg !60
  %t26 = load ptr, ptr %t25, align 8, !dbg !59
  %t27 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !59
  %t28 = load i64, ptr %t27, align 8, !dbg !59
  %t29 = load i64, ptr %t22, align 8, !dbg !61
  call void @ignite_rt_int_to_text(ptr %t25, i64 %t29), !dbg !61
  %t30 = load ptr, ptr %t25, align 8, !dbg !59
  %t31 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !59
  %t32 = load i64, ptr %t31, align 8, !dbg !59
  %t33 = add i64 %t28, %t32, !dbg !59
  %t34 = add i64 %t33, 6, !dbg !59
  %t35 = call ptr @ignite.alloc(i64 %t34, i64 1), !dbg !59
  store ptr %t35, ptr %t23, align 8, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t35, ptr @.str.15, i64 5, i1 false), !dbg !59
  %t36 = getelementptr inbounds i8, ptr %t35, i64 5, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t36, ptr %t26, i64 %t28, i1 false), !dbg !59
  %t37 = getelementptr inbounds i8, ptr %t36, i64 %t28, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t37, ptr @.str.16, i64 1, i1 false), !dbg !59
  %t38 = getelementptr inbounds i8, ptr %t37, i64 1, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t38, ptr %t30, i64 %t32, i1 false), !dbg !59
  %t39 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !59
  store i64 %t34, ptr %t39, align 8, !dbg !59
  %t40 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !59
  store i64 %t34, ptr %t40, align 8, !dbg !59
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t23, i64 24, i1 false), !dbg !57
  br label %L2, !dbg !57
L3:
  %t44 = load i32, ptr %p0, align 4, !dbg !57
  %t45 = icmp eq i32 %t44, 0, !dbg !57
  br i1 %t45, label %L46, label %L43, !dbg !57
L46:
  %t47 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !57
  %t48 = load i8, ptr %t47, align 1, !dbg !57
  %t49 = trunc i8 %t48 to i1, !dbg !57
  %t50 = icmp eq i1 %t49, false, !dbg !57
  br i1 %t50, label %L51, label %L43, !dbg !57
L51:
  %t52 = getelementptr inbounds i8, ptr %t47, i64 8, !dbg !57
  br label %L42, !dbg !57
L43:
  %t53 = load i32, ptr %p0, align 4, !dbg !57
  %t54 = icmp eq i32 %t53, 1, !dbg !57
  br i1 %t54, label %L55, label %L41, !dbg !57
L55:
  br label %L42, !dbg !57
L42:
  store ptr @.str.17, ptr %t25, align 8, !dbg !62
  %t56 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !62
  store i64 11, ptr %t56, align 8, !dbg !62
  %t57 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !62
  store i64 11, ptr %t57, align 8, !dbg !62
  call void @"copy.Text"(ptr %t58, ptr %t25), !dbg !62
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t58, i64 24, i1 false), !dbg !57
  br label %L2, !dbg !57
L41:
  %t60 = load i32, ptr %p0, align 4, !dbg !57
  %t61 = icmp eq i32 %t60, 0, !dbg !57
  br i1 %t61, label %L62, label %L59, !dbg !57
L62:
  %t63 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !57
  %t64 = load i8, ptr %t63, align 1, !dbg !57
  %t65 = trunc i8 %t64 to i1, !dbg !57
  %t66 = icmp eq i1 %t65, true, !dbg !57
  br i1 %t66, label %L67, label %L59, !dbg !57
L67:
  %t68 = getelementptr inbounds i8, ptr %t63, i64 8, !dbg !57
  store ptr @.str.9, ptr %t69, align 8, !dbg !63
  %t70 = getelementptr inbounds i8, ptr %t69, i64 8, !dbg !63
  store i64 5, ptr %t70, align 8, !dbg !63
  %t71 = getelementptr inbounds i8, ptr %t69, i64 16, !dbg !63
  store i64 5, ptr %t71, align 8, !dbg !63
  call void @"copy.Text"(ptr %t72, ptr %t69), !dbg !63
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t72, i64 24, i1 false), !dbg !57
  br label %L2, !dbg !57
L59:
  store ptr @.str.0, ptr %t73, align 8, !dbg !57
  %t74 = getelementptr inbounds i8, ptr %t73, i64 8, !dbg !57
  store i64 26, ptr %t74, align 8, !dbg !57
  %t75 = getelementptr inbounds i8, ptr %t73, i64 16, !dbg !57
  store i64 26, ptr %t75, align 8, !dbg !57
  call void @ignite_rt_fail_match(ptr %t73, i64 63, i64 3), !dbg !57
  unreachable, !dbg !57
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !56
  ret void, !dbg !56
}

define i1 @"tests.syntax.vowel"(ptr %p0) !dbg !64 {
entry:
  %t1 = alloca [1 x i8], align 1
  %t6 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t24 = alloca [24 x i8], align 8
  %t29 = alloca [24 x i8], align 8
  %t37 = alloca [24 x i8], align 8
  store ptr @.str.18, ptr %t6, align 8, !dbg !66
  %t7 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !66
  store i64 1, ptr %t7, align 8, !dbg !66
  %t8 = getelementptr inbounds i8, ptr %t6, i64 16, !dbg !66
  store i64 1, ptr %t8, align 8, !dbg !66
  %t9 = call i1 @ignite.text_eq(ptr %p0, ptr %t6), !dbg !66
  br i1 %t9, label %L10, label %L5, !dbg !66
L10:
  br label %L4, !dbg !66
L5:
  store ptr @.str.19, ptr %t12, align 8, !dbg !66
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !66
  store i64 1, ptr %t13, align 8, !dbg !66
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !66
  store i64 1, ptr %t14, align 8, !dbg !66
  %t15 = call i1 @ignite.text_eq(ptr %p0, ptr %t12), !dbg !66
  br i1 %t15, label %L16, label %L11, !dbg !66
L16:
  br label %L4, !dbg !66
L11:
  store ptr @.str.20, ptr %t18, align 8, !dbg !66
  %t19 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !66
  store i64 1, ptr %t19, align 8, !dbg !66
  %t20 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !66
  store i64 1, ptr %t20, align 8, !dbg !66
  %t21 = call i1 @ignite.text_eq(ptr %p0, ptr %t18), !dbg !66
  br i1 %t21, label %L22, label %L17, !dbg !66
L22:
  br label %L4, !dbg !66
L17:
  store ptr @.str.21, ptr %t24, align 8, !dbg !66
  %t25 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !66
  store i64 1, ptr %t25, align 8, !dbg !66
  %t26 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !66
  store i64 1, ptr %t26, align 8, !dbg !66
  %t27 = call i1 @ignite.text_eq(ptr %p0, ptr %t24), !dbg !66
  br i1 %t27, label %L28, label %L23, !dbg !66
L28:
  br label %L4, !dbg !66
L23:
  store ptr @.str.22, ptr %t29, align 8, !dbg !66
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !66
  store i64 1, ptr %t30, align 8, !dbg !66
  %t31 = getelementptr inbounds i8, ptr %t29, i64 16, !dbg !66
  store i64 1, ptr %t31, align 8, !dbg !66
  %t32 = call i1 @ignite.text_eq(ptr %p0, ptr %t29), !dbg !66
  br i1 %t32, label %L33, label %L3, !dbg !66
L33:
  br label %L4, !dbg !66
L4:
  %t34 = zext i1 true to i8, !dbg !66
  store i8 %t34, ptr %t1, align 1, !dbg !66
  br label %L2, !dbg !66
L3:
  %t36 = zext i1 false to i8, !dbg !66
  store i8 %t36, ptr %t1, align 1, !dbg !66
  br label %L2, !dbg !66
L35:
  store ptr @.str.0, ptr %t37, align 8, !dbg !66
  %t38 = getelementptr inbounds i8, ptr %t37, i64 8, !dbg !66
  store i64 26, ptr %t38, align 8, !dbg !66
  %t39 = getelementptr inbounds i8, ptr %t37, i64 16, !dbg !66
  store i64 26, ptr %t39, align 8, !dbg !66
  call void @ignite_rt_fail_match(ptr %t37, i64 71, i64 3), !dbg !66
  unreachable, !dbg !66
L2:
  %t40 = load i8, ptr %t1, align 1, !dbg !66
  %t41 = trunc i8 %t40 to i1, !dbg !66
  ret i1 %t41, !dbg !65
}

define i64 @"tests.syntax.first_positive"(ptr %p0) !dbg !70 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t15 = alloca [8 x i8], align 8
  store i64 0, ptr %t1, align 8, !dbg !72
  %t2 = call i64 @ignite.arena_mark(), !dbg !72
  br label %L3
L3:
  %t7 = load i64, ptr %t1, align 8, !dbg !72
  %t8 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !72
  %t9 = load i64, ptr %t8, align 8, !dbg !72
  %t10 = icmp slt i64 %t7, %t9, !dbg !72
  br i1 %t10, label %L4, label %L6, !dbg !72
L4:
  %t11 = load ptr, ptr %p0, align 8, !dbg !72
  %t12 = mul i64 %t7, 8, !dbg !72
  %t13 = getelementptr inbounds i8, ptr %t11, i64 %t12, !dbg !72
  %t14 = load i64, ptr %t13, align 8, !dbg !72
  store i64 %t14, ptr %t15, align 8, !dbg !72
  %t16 = load i64, ptr %t15, align 8, !dbg !75
  %t17 = icmp sgt i64 %t16, 0, !dbg !75
  br i1 %t17, label %L18, label %L19, !dbg !74
L18:
  %t21 = load i64, ptr %t15, align 8, !dbg !78
  ret i64 %t21, !dbg !77
L22:
  br label %L20, !dbg !74
L19:
  br label %L20, !dbg !74
L20:
  br label %L5
L5:
  %t23 = load i64, ptr %t1, align 8, !dbg !72
  %t24 = add i64 %t23, 1, !dbg !72
  store i64 %t24, ptr %t1, align 8, !dbg !72
  call void @ignite.arena_release(i64 %t2), !dbg !72
  br label %L3, !dbg !72
L6:
  ret i64 0, !dbg !71
}

define void @"tests.syntax.sign"(ptr %sret, i64 %p0) !dbg !80 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t11 = alloca [24 x i8], align 8
  %t16 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t24 = alloca [24 x i8], align 8
  %t27 = alloca [24 x i8], align 8
  %t28 = alloca [24 x i8], align 8
  %t31 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !81
  %t2 = load i64, ptr %t1, align 8, !dbg !84
  %t6 = icmp eq i64 %t2, 0, !dbg !83
  br i1 %t6, label %L7, label %L5, !dbg !83
L7:
  store ptr @.str.23, ptr %t8, align 8, !dbg !86
  %t9 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !86
  store i64 4, ptr %t9, align 8, !dbg !86
  %t10 = getelementptr inbounds i8, ptr %t8, i64 16, !dbg !86
  store i64 4, ptr %t10, align 8, !dbg !86
  call void @"copy.Text"(ptr %t11, ptr %t8), !dbg !86
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t11, i64 24, i1 false), !dbg !85
  ret void, !dbg !85
L12:
  br label %L4, !dbg !83
L5:
  %t14 = load i64, ptr %t1, align 8, !dbg !88
  %t15 = icmp sgt i64 %t14, 0, !dbg !88
  br i1 %t15, label %L17, label %L18, !dbg !87
L17:
  store ptr @.str.1, ptr %t20, align 8, !dbg !90
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !90
  store i64 1, ptr %t21, align 8, !dbg !90
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !90
  store i64 1, ptr %t22, align 8, !dbg !90
  call void @"copy.Text"(ptr %t23, ptr %t20), !dbg !90
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t23, i64 24, i1 false), !dbg !87
  br label %L19, !dbg !87
L18:
  store ptr @.str.24, ptr %t24, align 8, !dbg !92
  %t25 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !92
  store i64 1, ptr %t25, align 8, !dbg !92
  %t26 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !92
  store i64 1, ptr %t26, align 8, !dbg !92
  call void @"copy.Text"(ptr %t27, ptr %t24), !dbg !92
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t27, i64 24, i1 false), !dbg !87
  br label %L19, !dbg !87
L19:
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t16, i64 24, i1 false), !dbg !83
  br label %L4, !dbg !83
L13:
  store ptr @.str.0, ptr %t28, align 8, !dbg !83
  %t29 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !83
  store i64 26, ptr %t29, align 8, !dbg !83
  %t30 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !83
  store i64 26, ptr %t30, align 8, !dbg !83
  call void @ignite_rt_fail_match(ptr %t28, i64 87, i64 11), !dbg !83
  unreachable, !dbg !83
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %t31, ptr %t3, i64 24, i1 false), !dbg !82
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t31, i64 24, i1 false), !dbg !81
  ret void, !dbg !81
}

define i64 @"tests.syntax.count_spaces"(ptr %p0) !dbg !94 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [8 x i8], align 8
  %t16 = alloca [1 x i8], align 1
  %t28 = alloca [24 x i8], align 8
  %t31 = alloca [24 x i8], align 8
  store i64 0, ptr %t1, align 8, !dbg !96
  store i64 0, ptr %t2, align 8, !dbg !98
  %t3 = call i64 @ignite.arena_mark(), !dbg !98
  br label %L4
L4:
  %t8 = load i64, ptr %t2, align 8, !dbg !98
  %t9 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !98
  %t10 = load i64, ptr %t9, align 8, !dbg !98
  %t11 = icmp slt i64 %t8, %t10, !dbg !98
  br i1 %t11, label %L5, label %L7, !dbg !98
L5:
  %t12 = load ptr, ptr %p0, align 8, !dbg !98
  %t13 = mul i64 %t8, 1, !dbg !98
  %t14 = getelementptr inbounds i8, ptr %t12, i64 %t13, !dbg !98
  %t15 = load i8, ptr %t14, align 1, !dbg !98
  store i8 %t15, ptr %t16, align 1, !dbg !98
  %t17 = load i8, ptr %t16, align 1, !dbg !101
  %t20 = call i8 @"tests.syntax.SPACE"(), !dbg !100
  %t21 = icmp eq i8 %t17, %t20, !dbg !100
  br i1 %t21, label %L22, label %L19, !dbg !100
L22:
  %t23 = load i64, ptr %t1, align 8, !dbg !104
  %t24 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t23, i64 1), !dbg !104
  %t25 = extractvalue { i64, i1 } %t24, 1, !dbg !104
  br i1 %t25, label %L26, label %L27, !dbg !104
L26:
  store ptr @.str.0, ptr %t28, align 8, !dbg !104
  %t29 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !104
  store i64 26, ptr %t29, align 8, !dbg !104
  %t30 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !104
  store i64 26, ptr %t30, align 8, !dbg !104
  store ptr @.str.1, ptr %t31, align 8, !dbg !104
  %t32 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !104
  store i64 1, ptr %t32, align 8, !dbg !104
  %t33 = getelementptr inbounds i8, ptr %t31, i64 16, !dbg !104
  store i64 1, ptr %t33, align 8, !dbg !104
  call void @ignite_rt_fail_arith(ptr %t31, ptr %t28, i64 99, i64 13, i64 %t23, i64 1), !dbg !104
  unreachable, !dbg !104
L27:
  %t34 = extractvalue { i64, i1 } %t24, 0, !dbg !104
  store i64 %t34, ptr %t1, align 8, !dbg !103
  br label %L18, !dbg !100
L19:
  br label %L18, !dbg !100
L35:
  store ptr @.str.0, ptr %t31, align 8, !dbg !100
  %t36 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !100
  store i64 26, ptr %t36, align 8, !dbg !100
  %t37 = getelementptr inbounds i8, ptr %t31, i64 16, !dbg !100
  store i64 26, ptr %t37, align 8, !dbg !100
  call void @ignite_rt_fail_match(ptr %t31, i64 97, i64 5), !dbg !100
  unreachable, !dbg !100
L18:
  br label %L6
L6:
  %t38 = load i64, ptr %t2, align 8, !dbg !98
  %t39 = add i64 %t38, 1, !dbg !98
  store i64 %t39, ptr %t2, align 8, !dbg !98
  call void @ignite.arena_release(i64 %t3), !dbg !98
  br label %L4, !dbg !98
L7:
  %t40 = load i64, ptr %t1, align 8, !dbg !107
  ret i64 %t40, !dbg !95
}

define void @"tests.syntax.show"(ptr %sret, ptr %p0) !dbg !108 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t9 = alloca [40 x i8], align 8
  %t10 = alloca [32 x i8], align 8
  call void @"copy.List<Byte>"(ptr %t1, ptr %p0), !dbg !110
  call void @ignite.reserve(ptr %t1, i64 1, i64 1, i64 1), !dbg !112
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !112
  %t3 = load i64, ptr %t2, align 8, !dbg !112
  %t4 = load ptr, ptr %t1, align 8, !dbg !112
  %t5 = mul i64 %t3, 1, !dbg !112
  %t6 = getelementptr inbounds i8, ptr %t4, i64 %t5, !dbg !112
  store i8 10, ptr %t6, align 1, !dbg !112
  %t7 = add i64 %t3, 1, !dbg !112
  %t8 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !112
  store i64 %t7, ptr %t8, align 8, !dbg !112
  %t11 = call i32 @ignite_rt_stdout_write_all(ptr %t1, ptr %t10), !dbg !114
  %t12 = icmp ne i32 %t11, 0, !dbg !114
  br i1 %t12, label %L13, label %L14, !dbg !114
L13:
  store i32 1, ptr %t9, align 4, !dbg !114
  %t16 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !114
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t10, i64 32, i1 false), !dbg !114
  br label %L15, !dbg !114
L14:
  store i32 0, ptr %t9, align 4, !dbg !114
  br label %L15, !dbg !114
L15:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t9, i64 40, i1 false), !dbg !109
  ret void, !dbg !109
}

define internal void @"fnval.tests.syntax.shout"(ptr %env, ptr %a0, ptr %a1) {
entry:
  call void @"tests.syntax.shout"(ptr %a0, ptr %a1)
  ret void
}

define void @"tests.syntax.Syntax.connect"(ptr %sret, i64 %p0) !dbg !116 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [32 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t8 = alloca [32 x i8], align 8
  %t13 = alloca [8 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t33 = alloca [40 x i8], align 8
  %t41 = alloca [24 x i8], align 8
  %t61 = alloca [40 x i8], align 8
  %t71 = alloca [24 x i8], align 8
  %t72 = alloca [40 x i8], align 8
  %t81 = alloca [24 x i8], align 8
  %t84 = alloca [24 x i8], align 8
  %t85 = alloca [24 x i8], align 8
  %t88 = alloca [24 x i8], align 8
  %t89 = alloca [40 x i8], align 8
  %t98 = alloca [24 x i8], align 8
  %t101 = alloca [24 x i8], align 8
  %t102 = alloca [24 x i8], align 8
  %t105 = alloca [24 x i8], align 8
  %t107 = alloca [24 x i8], align 8
  %t110 = alloca [24 x i8], align 8
  %t111 = alloca [40 x i8], align 8
  %t120 = alloca [24 x i8], align 8
  %t123 = alloca [24 x i8], align 8
  %t124 = alloca [24 x i8], align 8
  %t127 = alloca [24 x i8], align 8
  %t129 = alloca [24 x i8], align 8
  %t132 = alloca [24 x i8], align 8
  %t133 = alloca [40 x i8], align 8
  %t142 = alloca [24 x i8], align 8
  %t145 = alloca [24 x i8], align 8
  %t146 = alloca [24 x i8], align 8
  %t149 = alloca [24 x i8], align 8
  %t151 = alloca [24 x i8], align 8
  %t154 = alloca [24 x i8], align 8
  %t156 = alloca [24 x i8], align 8
  %t159 = alloca [24 x i8], align 8
  %t160 = alloca [40 x i8], align 8
  %t168 = alloca [32 x i8], align 8
  %t172 = alloca [24 x i8], align 8
  %t176 = alloca [40 x i8], align 8
  %t178 = alloca [24 x i8], align 8
  %t179 = alloca [40 x i8], align 8
  %t187 = alloca [32 x i8], align 8
  %t190 = alloca [24 x i8], align 8
  %t194 = alloca [40 x i8], align 8
  %t196 = alloca [24 x i8], align 8
  %t197 = alloca [40 x i8], align 8
  %t205 = alloca [40 x i8], align 8
  %t206 = alloca [24 x i8], align 8
  %t207 = alloca [40 x i8], align 8
  %t215 = alloca [32 x i8], align 8
  %t218 = alloca [24 x i8], align 8
  %t222 = alloca [40 x i8], align 8
  %t224 = alloca [24 x i8], align 8
  %t225 = alloca [40 x i8], align 8
  %t233 = alloca [24 x i8], align 8
  %t236 = alloca [24 x i8], align 8
  %t238 = alloca [1 x i8], align 1
  %t240 = alloca [24 x i8], align 8
  %t243 = alloca [24 x i8], align 8
  %t245 = alloca [1 x i8], align 1
  %t247 = alloca [24 x i8], align 8
  %t250 = alloca [24 x i8], align 8
  %t253 = alloca [24 x i8], align 8
  %t257 = alloca [24 x i8], align 8
  %t278 = alloca [40 x i8], align 8
  %t294 = alloca [40 x i8], align 8
  %t302 = alloca [24 x i8], align 8
  %t320 = alloca [40 x i8], align 8
  %t330 = alloca [24 x i8], align 8
  %t332 = alloca [24 x i8], align 8
  %t333 = alloca [40 x i8], align 8
  %t341 = alloca [8 x i8], align 8
  %t344 = alloca [24 x i8], align 8
  %t345 = alloca [40 x i8], align 8
  %t353 = alloca [24 x i8], align 8
  %t356 = alloca [24 x i8], align 8
  %t357 = alloca [24 x i8], align 8
  %t358 = alloca [40 x i8], align 8
  %t366 = alloca [40 x i8], align 8
  %t367 = alloca [24 x i8], align 8
  %t370 = alloca [24 x i8], align 8
  %t371 = alloca [16 x i8], align 8
  %t374 = alloca [40 x i8], align 8
  %t379 = alloca [24 x i8], align 8
  %t380 = alloca [40 x i8], align 8
  %t388 = alloca [40 x i8], align 8
  %t390 = alloca [24 x i8], align 8
  %t393 = alloca [24 x i8], align 8
  %t394 = alloca [24 x i8], align 8
  %t397 = alloca [8 x i8], align 8
  %t401 = alloca [40 x i8], align 8
  %t402 = alloca [24 x i8], align 8
  %t405 = alloca [24 x i8], align 8
  %t414 = alloca [40 x i8], align 8
  %t430 = alloca [24 x i8], align 8
  %t431 = alloca [40 x i8], align 8
  %t443 = alloca [24 x i8], align 8
  %t446 = alloca [24 x i8], align 8
  %t447 = alloca [40 x i8], align 8
  %t455 = alloca [24 x i8], align 8
  %t458 = alloca [1 x i8], align 1
  %t461 = alloca [1 x i8], align 1
  %t462 = alloca [24 x i8], align 8
  %t465 = alloca [24 x i8], align 8
  %t468 = alloca [24 x i8], align 8
  %t472 = alloca [24 x i8], align 8
  %t496 = alloca [40 x i8], align 8
  %t504 = alloca [32 x i8], align 8
  %t508 = alloca [32 x i8], align 8
  %t509 = alloca [8 x i8], align 8
  %t510 = alloca [32 x i8], align 8
  %t512 = alloca [8 x i8], align 8
  %t519 = alloca [32 x i8], align 8
  %t523 = alloca [8 x i8], align 8
  %t527 = alloca [32 x i8], align 8
  %t533 = alloca [24 x i8], align 8
  %t536 = alloca [24 x i8], align 8
  %t556 = alloca [24 x i8], align 8
  %t565 = alloca [32 x i8], align 8
  %t569 = alloca [8 x i8], align 8
  %t575 = alloca [8 x i8], align 8
  %t577 = alloca [24 x i8], align 8
  %t592 = alloca [40 x i8], align 8
  %t600 = alloca [32 x i8], align 8
  %t603 = alloca [24 x i8], align 8
  %t605 = alloca [32 x i8], align 8
  %t606 = alloca [32 x i8], align 8
  %t610 = alloca [8 x i8], align 8
  %t612 = alloca [24 x i8], align 8
  %t620 = alloca [24 x i8], align 8
  %t623 = alloca [24 x i8], align 8
  %t636 = alloca [24 x i8], align 8
  %t648 = alloca [40 x i8], align 8
  %t664 = alloca [24 x i8], align 8
  %t667 = alloca [8 x i8], align 8
  %t668 = alloca [8 x i8], align 8
  %t674 = alloca [24 x i8], align 8
  %t677 = alloca [24 x i8], align 8
  %t678 = alloca [40 x i8], align 8
  %t690 = alloca [24 x i8], align 8
  %t693 = alloca [24 x i8], align 8
  %t694 = alloca [40 x i8], align 8
  %t702 = alloca [24 x i8], align 8
  %t705 = alloca [24 x i8], align 8
  %t706 = alloca [24 x i8], align 8
  %t720 = alloca [40 x i8], align 8
  %t728 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !117
  store i64 7, ptr %t2, align 8, !dbg !120
  store ptr @.str.25, ptr %t3, align 8, !dbg !122
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !122
  store i64 5, ptr %t4, align 8, !dbg !122
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16, !dbg !122
  store i64 5, ptr %t5, align 8, !dbg !122
  call void @"copy.Text"(ptr %t6, ptr %t3), !dbg !122
  %t7 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !120
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr %t6, i64 24, i1 false), !dbg !120
  call void @"tests.syntax.swap"(ptr %t8, ptr %t2), !dbg !119
  %t11 = getelementptr inbounds i8, ptr %t8, i64 24, !dbg !118
  %t12 = load i64, ptr %t11, align 8, !dbg !118
  store i64 %t12, ptr %t13, align 8, !dbg !118
  br label %L9, !dbg !118
L10:
  store ptr @.str.0, ptr %t14, align 8, !dbg !118
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !118
  store i64 26, ptr %t15, align 8, !dbg !118
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !118
  store i64 26, ptr %t16, align 8, !dbg !118
  call void @ignite_rt_fail_match(ptr %t14, i64 115, i64 9), !dbg !118
  unreachable, !dbg !118
L9:
  call void @"copy.Text"(ptr %t18, ptr %t8), !dbg !125
  %t19 = load ptr, ptr %t18, align 8, !dbg !124
  %t20 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !124
  %t21 = load i64, ptr %t20, align 8, !dbg !124
  %t22 = load i64, ptr %t13, align 8, !dbg !126
  call void @ignite_rt_int_to_text(ptr %t18, i64 %t22), !dbg !126
  %t23 = load ptr, ptr %t18, align 8, !dbg !124
  %t24 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !124
  %t25 = load i64, ptr %t24, align 8, !dbg !124
  %t26 = add i64 %t21, %t25, !dbg !124
  %t27 = add i64 %t26, 1, !dbg !124
  %t28 = call ptr @ignite.alloc(i64 %t27, i64 1), !dbg !124
  store ptr %t28, ptr %t17, align 8, !dbg !124
  call void @llvm.memcpy.p0.p0.i64(ptr %t28, ptr %t19, i64 %t21, i1 false), !dbg !124
  %t29 = getelementptr inbounds i8, ptr %t28, i64 %t21, !dbg !124
  call void @llvm.memcpy.p0.p0.i64(ptr %t29, ptr @.str.16, i64 1, i1 false), !dbg !124
  %t30 = getelementptr inbounds i8, ptr %t29, i64 1, !dbg !124
  call void @llvm.memcpy.p0.p0.i64(ptr %t30, ptr %t23, i64 %t25, i1 false), !dbg !124
  %t31 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !124
  store i64 %t27, ptr %t31, align 8, !dbg !124
  %t32 = getelementptr inbounds i8, ptr %t17, i64 16, !dbg !124
  store i64 %t27, ptr %t32, align 8, !dbg !124
  call void @"tests.syntax.show"(ptr %t33, ptr %t17), !dbg !123
  %t34 = load i32, ptr %t33, align 4, !dbg !123
  %t35 = icmp eq i32 %t34, 1, !dbg !123
  br i1 %t35, label %L36, label %L37, !dbg !123
L36:
  store i32 1, ptr %sret, align 4, !dbg !123
  %t38 = getelementptr inbounds i8, ptr %t33, i64 8, !dbg !123
  %t39 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !123
  call void @llvm.memcpy.p0.p0.i64(ptr %t39, ptr %t38, i64 32, i1 false), !dbg !123
  ret void, !dbg !123
L37:
  %t40 = getelementptr inbounds i8, ptr %t33, i64 8, !dbg !123
  call void @"tests.syntax.describe"(ptr %t41, i64 0), !dbg !128
  %t42 = load ptr, ptr %t41, align 8, !dbg !128
  %t43 = getelementptr inbounds i8, ptr %t41, i64 8, !dbg !128
  %t44 = load i64, ptr %t43, align 8, !dbg !128
  call void @"tests.syntax.describe"(ptr %t41, i64 41), !dbg !130
  %t45 = load ptr, ptr %t41, align 8, !dbg !128
  %t46 = getelementptr inbounds i8, ptr %t41, i64 8, !dbg !128
  %t47 = load i64, ptr %t46, align 8, !dbg !128
  call void @"tests.syntax.describe"(ptr %t41, i64 9), !dbg !132
  %t48 = load ptr, ptr %t41, align 8, !dbg !128
  %t49 = getelementptr inbounds i8, ptr %t41, i64 8, !dbg !128
  %t50 = load i64, ptr %t49, align 8, !dbg !128
  %t51 = add i64 %t44, %t47, !dbg !128
  %t52 = add i64 %t51, %t50, !dbg !128
  %t53 = add i64 %t52, 2, !dbg !128
  %t54 = call ptr @ignite.alloc(i64 %t53, i64 1), !dbg !128
  store ptr %t54, ptr %t18, align 8, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t54, ptr %t42, i64 %t44, i1 false), !dbg !128
  %t55 = getelementptr inbounds i8, ptr %t54, i64 %t44, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t55, ptr @.str.16, i64 1, i1 false), !dbg !128
  %t56 = getelementptr inbounds i8, ptr %t55, i64 1, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t56, ptr %t45, i64 %t47, i1 false), !dbg !128
  %t57 = getelementptr inbounds i8, ptr %t56, i64 %t47, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t57, ptr @.str.16, i64 1, i1 false), !dbg !128
  %t58 = getelementptr inbounds i8, ptr %t57, i64 1, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t58, ptr %t48, i64 %t50, i1 false), !dbg !128
  %t59 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !128
  store i64 %t53, ptr %t59, align 8, !dbg !128
  %t60 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !128
  store i64 %t53, ptr %t60, align 8, !dbg !128
  call void @"tests.syntax.show"(ptr %t61, ptr %t18), !dbg !127
  %t62 = load i32, ptr %t61, align 4, !dbg !127
  %t63 = icmp eq i32 %t62, 1, !dbg !127
  br i1 %t63, label %L64, label %L65, !dbg !127
L64:
  store i32 1, ptr %sret, align 4, !dbg !127
  %t66 = getelementptr inbounds i8, ptr %t61, i64 8, !dbg !127
  %t67 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !127
  call void @llvm.memcpy.p0.p0.i64(ptr %t67, ptr %t66, i64 32, i1 false), !dbg !127
  ret void, !dbg !127
L65:
  %t68 = getelementptr inbounds i8, ptr %t61, i64 8, !dbg !127
  store ptr @ignite.empty, ptr %t41, align 8, !dbg !136
  %t69 = getelementptr inbounds i8, ptr %t41, i64 8, !dbg !136
  store i64 0, ptr %t69, align 8, !dbg !136
  %t70 = getelementptr inbounds i8, ptr %t41, i64 16, !dbg !136
  store i64 0, ptr %t70, align 8, !dbg !136
  call void @"tests.syntax.classify"(ptr %t71, ptr %t41), !dbg !135
  call void @"tests.syntax.show"(ptr %t72, ptr %t71), !dbg !134
  %t73 = load i32, ptr %t72, align 4, !dbg !134
  %t74 = icmp eq i32 %t73, 1, !dbg !134
  br i1 %t74, label %L75, label %L76, !dbg !134
L75:
  store i32 1, ptr %sret, align 4, !dbg !134
  %t77 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !134
  %t78 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !134
  call void @llvm.memcpy.p0.p0.i64(ptr %t78, ptr %t77, i64 32, i1 false), !dbg !134
  ret void, !dbg !134
L76:
  %t79 = getelementptr inbounds i8, ptr %t72, i64 8, !dbg !134
  %t80 = call ptr @ignite.alloc(i64 24, i64 8), !dbg !139
  store ptr @.str.26, ptr %t81, align 8, !dbg !140
  %t82 = getelementptr inbounds i8, ptr %t81, i64 8, !dbg !140
  store i64 1, ptr %t82, align 8, !dbg !140
  %t83 = getelementptr inbounds i8, ptr %t81, i64 16, !dbg !140
  store i64 1, ptr %t83, align 8, !dbg !140
  call void @"copy.Text"(ptr %t84, ptr %t81), !dbg !140
  call void @llvm.memcpy.p0.p0.i64(ptr %t80, ptr %t84, i64 24, i1 false), !dbg !139
  store ptr %t80, ptr %t85, align 8, !dbg !139
  %t86 = getelementptr inbounds i8, ptr %t85, i64 8, !dbg !139
  store i64 1, ptr %t86, align 8, !dbg !139
  %t87 = getelementptr inbounds i8, ptr %t85, i64 16, !dbg !139
  store i64 1, ptr %t87, align 8, !dbg !139
  call void @"tests.syntax.classify"(ptr %t88, ptr %t85), !dbg !138
  call void @"tests.syntax.show"(ptr %t89, ptr %t88), !dbg !137
  %t90 = load i32, ptr %t89, align 4, !dbg !137
  %t91 = icmp eq i32 %t90, 1, !dbg !137
  br i1 %t91, label %L92, label %L93, !dbg !137
L92:
  store i32 1, ptr %sret, align 4, !dbg !137
  %t94 = getelementptr inbounds i8, ptr %t89, i64 8, !dbg !137
  %t95 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !137
  call void @llvm.memcpy.p0.p0.i64(ptr %t95, ptr %t94, i64 32, i1 false), !dbg !137
  ret void, !dbg !137
L93:
  %t96 = getelementptr inbounds i8, ptr %t89, i64 8, !dbg !137
  %t97 = call ptr @ignite.alloc(i64 48, i64 8), !dbg !143
  store ptr @.str.2, ptr %t98, align 8, !dbg !144
  %t99 = getelementptr inbounds i8, ptr %t98, i64 8, !dbg !144
  store i64 4, ptr %t99, align 8, !dbg !144
  %t100 = getelementptr inbounds i8, ptr %t98, i64 16, !dbg !144
  store i64 4, ptr %t100, align 8, !dbg !144
  call void @"copy.Text"(ptr %t101, ptr %t98), !dbg !144
  call void @llvm.memcpy.p0.p0.i64(ptr %t97, ptr %t101, i64 24, i1 false), !dbg !143
  store ptr @.str.27, ptr %t102, align 8, !dbg !145
  %t103 = getelementptr inbounds i8, ptr %t102, i64 8, !dbg !145
  store i64 3, ptr %t103, align 8, !dbg !145
  %t104 = getelementptr inbounds i8, ptr %t102, i64 16, !dbg !145
  store i64 3, ptr %t104, align 8, !dbg !145
  call void @"copy.Text"(ptr %t105, ptr %t102), !dbg !145
  %t106 = getelementptr inbounds i8, ptr %t97, i64 24, !dbg !143
  call void @llvm.memcpy.p0.p0.i64(ptr %t106, ptr %t105, i64 24, i1 false), !dbg !143
  store ptr %t97, ptr %t107, align 8, !dbg !143
  %t108 = getelementptr inbounds i8, ptr %t107, i64 8, !dbg !143
  store i64 2, ptr %t108, align 8, !dbg !143
  %t109 = getelementptr inbounds i8, ptr %t107, i64 16, !dbg !143
  store i64 2, ptr %t109, align 8, !dbg !143
  call void @"tests.syntax.classify"(ptr %t110, ptr %t107), !dbg !142
  call void @"tests.syntax.show"(ptr %t111, ptr %t110), !dbg !141
  %t112 = load i32, ptr %t111, align 4, !dbg !141
  %t113 = icmp eq i32 %t112, 1, !dbg !141
  br i1 %t113, label %L114, label %L115, !dbg !141
L114:
  store i32 1, ptr %sret, align 4, !dbg !141
  %t116 = getelementptr inbounds i8, ptr %t111, i64 8, !dbg !141
  %t117 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !141
  call void @llvm.memcpy.p0.p0.i64(ptr %t117, ptr %t116, i64 32, i1 false), !dbg !141
  ret void, !dbg !141
L115:
  %t118 = getelementptr inbounds i8, ptr %t111, i64 8, !dbg !141
  %t119 = call ptr @ignite.alloc(i64 48, i64 8), !dbg !148
  store ptr @.str.18, ptr %t120, align 8, !dbg !149
  %t121 = getelementptr inbounds i8, ptr %t120, i64 8, !dbg !149
  store i64 1, ptr %t121, align 8, !dbg !149
  %t122 = getelementptr inbounds i8, ptr %t120, i64 16, !dbg !149
  store i64 1, ptr %t122, align 8, !dbg !149
  call void @"copy.Text"(ptr %t123, ptr %t120), !dbg !149
  call void @llvm.memcpy.p0.p0.i64(ptr %t119, ptr %t123, i64 24, i1 false), !dbg !148
  store ptr @.str.28, ptr %t124, align 8, !dbg !150
  %t125 = getelementptr inbounds i8, ptr %t124, i64 8, !dbg !150
  store i64 1, ptr %t125, align 8, !dbg !150
  %t126 = getelementptr inbounds i8, ptr %t124, i64 16, !dbg !150
  store i64 1, ptr %t126, align 8, !dbg !150
  call void @"copy.Text"(ptr %t127, ptr %t124), !dbg !150
  %t128 = getelementptr inbounds i8, ptr %t119, i64 24, !dbg !148
  call void @llvm.memcpy.p0.p0.i64(ptr %t128, ptr %t127, i64 24, i1 false), !dbg !148
  store ptr %t119, ptr %t129, align 8, !dbg !148
  %t130 = getelementptr inbounds i8, ptr %t129, i64 8, !dbg !148
  store i64 2, ptr %t130, align 8, !dbg !148
  %t131 = getelementptr inbounds i8, ptr %t129, i64 16, !dbg !148
  store i64 2, ptr %t131, align 8, !dbg !148
  call void @"tests.syntax.classify"(ptr %t132, ptr %t129), !dbg !147
  call void @"tests.syntax.show"(ptr %t133, ptr %t132), !dbg !146
  %t134 = load i32, ptr %t133, align 4, !dbg !146
  %t135 = icmp eq i32 %t134, 1, !dbg !146
  br i1 %t135, label %L136, label %L137, !dbg !146
L136:
  store i32 1, ptr %sret, align 4, !dbg !146
  %t138 = getelementptr inbounds i8, ptr %t133, i64 8, !dbg !146
  %t139 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !146
  call void @llvm.memcpy.p0.p0.i64(ptr %t139, ptr %t138, i64 32, i1 false), !dbg !146
  ret void, !dbg !146
L137:
  %t140 = getelementptr inbounds i8, ptr %t133, i64 8, !dbg !146
  %t141 = call ptr @ignite.alloc(i64 72, i64 8), !dbg !153
  store ptr @.str.18, ptr %t142, align 8, !dbg !154
  %t143 = getelementptr inbounds i8, ptr %t142, i64 8, !dbg !154
  store i64 1, ptr %t143, align 8, !dbg !154
  %t144 = getelementptr inbounds i8, ptr %t142, i64 16, !dbg !154
  store i64 1, ptr %t144, align 8, !dbg !154
  call void @"copy.Text"(ptr %t145, ptr %t142), !dbg !154
  call void @llvm.memcpy.p0.p0.i64(ptr %t141, ptr %t145, i64 24, i1 false), !dbg !153
  store ptr @.str.28, ptr %t146, align 8, !dbg !155
  %t147 = getelementptr inbounds i8, ptr %t146, i64 8, !dbg !155
  store i64 1, ptr %t147, align 8, !dbg !155
  %t148 = getelementptr inbounds i8, ptr %t146, i64 16, !dbg !155
  store i64 1, ptr %t148, align 8, !dbg !155
  call void @"copy.Text"(ptr %t149, ptr %t146), !dbg !155
  %t150 = getelementptr inbounds i8, ptr %t141, i64 24, !dbg !153
  call void @llvm.memcpy.p0.p0.i64(ptr %t150, ptr %t149, i64 24, i1 false), !dbg !153
  store ptr @.str.29, ptr %t151, align 8, !dbg !156
  %t152 = getelementptr inbounds i8, ptr %t151, i64 8, !dbg !156
  store i64 1, ptr %t152, align 8, !dbg !156
  %t153 = getelementptr inbounds i8, ptr %t151, i64 16, !dbg !156
  store i64 1, ptr %t153, align 8, !dbg !156
  call void @"copy.Text"(ptr %t154, ptr %t151), !dbg !156
  %t155 = getelementptr inbounds i8, ptr %t141, i64 48, !dbg !153
  call void @llvm.memcpy.p0.p0.i64(ptr %t155, ptr %t154, i64 24, i1 false), !dbg !153
  store ptr %t141, ptr %t156, align 8, !dbg !153
  %t157 = getelementptr inbounds i8, ptr %t156, i64 8, !dbg !153
  store i64 3, ptr %t157, align 8, !dbg !153
  %t158 = getelementptr inbounds i8, ptr %t156, i64 16, !dbg !153
  store i64 3, ptr %t158, align 8, !dbg !153
  call void @"tests.syntax.classify"(ptr %t159, ptr %t156), !dbg !152
  call void @"tests.syntax.show"(ptr %t160, ptr %t159), !dbg !151
  %t161 = load i32, ptr %t160, align 4, !dbg !151
  %t162 = icmp eq i32 %t161, 1, !dbg !151
  br i1 %t162, label %L163, label %L164, !dbg !151
L163:
  store i32 1, ptr %sret, align 4, !dbg !151
  %t165 = getelementptr inbounds i8, ptr %t160, i64 8, !dbg !151
  %t166 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !151
  call void @llvm.memcpy.p0.p0.i64(ptr %t166, ptr %t165, i64 32, i1 false), !dbg !151
  ret void, !dbg !151
L164:
  %t167 = getelementptr inbounds i8, ptr %t160, i64 8, !dbg !151
  %t169 = zext i1 true to i8, !dbg !160
  store i8 %t169, ptr %t168, align 1, !dbg !160
  %t170 = call ptr @ignite.alloc(i64 16, i64 8), !dbg !162
  store i64 1, ptr %t170, align 8, !dbg !162
  %t171 = getelementptr inbounds i8, ptr %t170, i64 8, !dbg !162
  store i64 2, ptr %t171, align 8, !dbg !162
  store ptr %t170, ptr %t172, align 8, !dbg !162
  %t173 = getelementptr inbounds i8, ptr %t172, i64 8, !dbg !162
  store i64 2, ptr %t173, align 8, !dbg !162
  %t174 = getelementptr inbounds i8, ptr %t172, i64 16, !dbg !162
  store i64 2, ptr %t174, align 8, !dbg !162
  %t175 = getelementptr inbounds i8, ptr %t168, i64 8, !dbg !160
  call void @llvm.memcpy.p0.p0.i64(ptr %t175, ptr %t172, i64 24, i1 false), !dbg !160
  store i32 0, ptr %t176, align 4, !dbg !159
  %t177 = getelementptr inbounds i8, ptr %t176, i64 8, !dbg !159
  call void @llvm.memcpy.p0.p0.i64(ptr %t177, ptr %t168, i64 32, i1 false), !dbg !159
  call void @"tests.syntax.nested"(ptr %t178, ptr %t176), !dbg !158
  call void @"tests.syntax.show"(ptr %t179, ptr %t178), !dbg !157
  %t180 = load i32, ptr %t179, align 4, !dbg !157
  %t181 = icmp eq i32 %t180, 1, !dbg !157
  br i1 %t181, label %L182, label %L183, !dbg !157
L182:
  store i32 1, ptr %sret, align 4, !dbg !157
  %t184 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !157
  %t185 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !157
  call void @llvm.memcpy.p0.p0.i64(ptr %t185, ptr %t184, i64 32, i1 false), !dbg !157
  ret void, !dbg !157
L183:
  %t186 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !157
  %t188 = zext i1 false to i8, !dbg !168
  store i8 %t188, ptr %t187, align 1, !dbg !168
  %t189 = call ptr @ignite.alloc(i64 8, i64 8), !dbg !170
  store i64 1, ptr %t189, align 8, !dbg !170
  store ptr %t189, ptr %t190, align 8, !dbg !170
  %t191 = getelementptr inbounds i8, ptr %t190, i64 8, !dbg !170
  store i64 1, ptr %t191, align 8, !dbg !170
  %t192 = getelementptr inbounds i8, ptr %t190, i64 16, !dbg !170
  store i64 1, ptr %t192, align 8, !dbg !170
  %t193 = getelementptr inbounds i8, ptr %t187, i64 8, !dbg !168
  call void @llvm.memcpy.p0.p0.i64(ptr %t193, ptr %t190, i64 24, i1 false), !dbg !168
  store i32 0, ptr %t194, align 4, !dbg !167
  %t195 = getelementptr inbounds i8, ptr %t194, i64 8, !dbg !167
  call void @llvm.memcpy.p0.p0.i64(ptr %t195, ptr %t187, i64 32, i1 false), !dbg !167
  call void @"tests.syntax.nested"(ptr %t196, ptr %t194), !dbg !166
  call void @"tests.syntax.show"(ptr %t197, ptr %t196), !dbg !165
  %t198 = load i32, ptr %t197, align 4, !dbg !165
  %t199 = icmp eq i32 %t198, 1, !dbg !165
  br i1 %t199, label %L200, label %L201, !dbg !165
L200:
  store i32 1, ptr %sret, align 4, !dbg !165
  %t202 = getelementptr inbounds i8, ptr %t197, i64 8, !dbg !165
  %t203 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !165
  call void @llvm.memcpy.p0.p0.i64(ptr %t203, ptr %t202, i64 32, i1 false), !dbg !165
  ret void, !dbg !165
L201:
  %t204 = getelementptr inbounds i8, ptr %t197, i64 8, !dbg !165
  store i32 1, ptr %t205, align 4, !dbg !174
  call void @"tests.syntax.nested"(ptr %t206, ptr %t205), !dbg !173
  call void @"tests.syntax.show"(ptr %t207, ptr %t206), !dbg !172
  %t208 = load i32, ptr %t207, align 4, !dbg !172
  %t209 = icmp eq i32 %t208, 1, !dbg !172
  br i1 %t209, label %L210, label %L211, !dbg !172
L210:
  store i32 1, ptr %sret, align 4, !dbg !172
  %t212 = getelementptr inbounds i8, ptr %t207, i64 8, !dbg !172
  %t213 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !172
  call void @llvm.memcpy.p0.p0.i64(ptr %t213, ptr %t212, i64 32, i1 false), !dbg !172
  ret void, !dbg !172
L211:
  %t214 = getelementptr inbounds i8, ptr %t207, i64 8, !dbg !172
  %t216 = zext i1 true to i8, !dbg !178
  store i8 %t216, ptr %t215, align 1, !dbg !178
  %t217 = call ptr @ignite.alloc(i64 8, i64 8), !dbg !180
  store i64 3, ptr %t217, align 8, !dbg !180
  store ptr %t217, ptr %t218, align 8, !dbg !180
  %t219 = getelementptr inbounds i8, ptr %t218, i64 8, !dbg !180
  store i64 1, ptr %t219, align 8, !dbg !180
  %t220 = getelementptr inbounds i8, ptr %t218, i64 16, !dbg !180
  store i64 1, ptr %t220, align 8, !dbg !180
  %t221 = getelementptr inbounds i8, ptr %t215, i64 8, !dbg !178
  call void @llvm.memcpy.p0.p0.i64(ptr %t221, ptr %t218, i64 24, i1 false), !dbg !178
  store i32 0, ptr %t222, align 4, !dbg !177
  %t223 = getelementptr inbounds i8, ptr %t222, i64 8, !dbg !177
  call void @llvm.memcpy.p0.p0.i64(ptr %t223, ptr %t215, i64 32, i1 false), !dbg !177
  call void @"tests.syntax.nested"(ptr %t224, ptr %t222), !dbg !176
  call void @"tests.syntax.show"(ptr %t225, ptr %t224), !dbg !175
  %t226 = load i32, ptr %t225, align 4, !dbg !175
  %t227 = icmp eq i32 %t226, 1, !dbg !175
  br i1 %t227, label %L228, label %L229, !dbg !175
L228:
  store i32 1, ptr %sret, align 4, !dbg !175
  %t230 = getelementptr inbounds i8, ptr %t225, i64 8, !dbg !175
  %t231 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !175
  call void @llvm.memcpy.p0.p0.i64(ptr %t231, ptr %t230, i64 32, i1 false), !dbg !175
  ret void, !dbg !175
L229:
  %t232 = getelementptr inbounds i8, ptr %t225, i64 8, !dbg !175
  store ptr @.str.19, ptr %t233, align 8, !dbg !184
  %t234 = getelementptr inbounds i8, ptr %t233, i64 8, !dbg !184
  store i64 1, ptr %t234, align 8, !dbg !184
  %t235 = getelementptr inbounds i8, ptr %t233, i64 16, !dbg !184
  store i64 1, ptr %t235, align 8, !dbg !184
  call void @"copy.Text"(ptr %t236, ptr %t233), !dbg !184
  %t237 = call i1 @"tests.syntax.vowel"(ptr %t236), !dbg !183
  %t239 = zext i1 %t237 to i8, !dbg !182
  store i8 %t239, ptr %t238, align 1, !dbg !182
  store ptr @.str.26, ptr %t240, align 8, !dbg !187
  %t241 = getelementptr inbounds i8, ptr %t240, i64 8, !dbg !187
  store i64 1, ptr %t241, align 8, !dbg !187
  %t242 = getelementptr inbounds i8, ptr %t240, i64 16, !dbg !187
  store i64 1, ptr %t242, align 8, !dbg !187
  call void @"copy.Text"(ptr %t243, ptr %t240), !dbg !187
  %t244 = call i1 @"tests.syntax.vowel"(ptr %t243), !dbg !186
  %t246 = zext i1 %t244 to i8, !dbg !185
  store i8 %t246, ptr %t245, align 1, !dbg !185
  %t248 = load i8, ptr %t238, align 1, !dbg !190
  %t249 = trunc i8 %t248 to i1, !dbg !190
  store ptr @.str.30, ptr %t250, align 8, !dbg !190
  %t251 = getelementptr inbounds i8, ptr %t250, i64 8, !dbg !190
  store i64 4, ptr %t251, align 8, !dbg !190
  %t252 = getelementptr inbounds i8, ptr %t250, i64 16, !dbg !190
  store i64 4, ptr %t252, align 8, !dbg !190
  store ptr @.str.31, ptr %t253, align 8, !dbg !190
  %t254 = getelementptr inbounds i8, ptr %t253, i64 8, !dbg !190
  store i64 5, ptr %t254, align 8, !dbg !190
  %t255 = getelementptr inbounds i8, ptr %t253, i64 16, !dbg !190
  store i64 5, ptr %t255, align 8, !dbg !190
  %t256 = select i1 %t249, ptr %t250, ptr %t253, !dbg !190
  call void @"copy.Text"(ptr %t257, ptr %t256), !dbg !190
  %t258 = load ptr, ptr %t257, align 8, !dbg !189
  %t259 = getelementptr inbounds i8, ptr %t257, i64 8, !dbg !189
  %t260 = load i64, ptr %t259, align 8, !dbg !189
  %t261 = load i8, ptr %t245, align 1, !dbg !191
  %t262 = trunc i8 %t261 to i1, !dbg !191
  store ptr @.str.30, ptr %t257, align 8, !dbg !191
  %t263 = getelementptr inbounds i8, ptr %t257, i64 8, !dbg !191
  store i64 4, ptr %t263, align 8, !dbg !191
  %t264 = getelementptr inbounds i8, ptr %t257, i64 16, !dbg !191
  store i64 4, ptr %t264, align 8, !dbg !191
  store ptr @.str.31, ptr %t253, align 8, !dbg !191
  %t265 = getelementptr inbounds i8, ptr %t253, i64 8, !dbg !191
  store i64 5, ptr %t265, align 8, !dbg !191
  %t266 = getelementptr inbounds i8, ptr %t253, i64 16, !dbg !191
  store i64 5, ptr %t266, align 8, !dbg !191
  %t267 = select i1 %t262, ptr %t257, ptr %t253, !dbg !191
  call void @"copy.Text"(ptr %t250, ptr %t267), !dbg !191
  %t268 = load ptr, ptr %t250, align 8, !dbg !189
  %t269 = getelementptr inbounds i8, ptr %t250, i64 8, !dbg !189
  %t270 = load i64, ptr %t269, align 8, !dbg !189
  %t271 = add i64 %t260, %t270, !dbg !189
  %t272 = add i64 %t271, 1, !dbg !189
  %t273 = call ptr @ignite.alloc(i64 %t272, i64 1), !dbg !189
  store ptr %t273, ptr %t247, align 8, !dbg !189
  call void @llvm.memcpy.p0.p0.i64(ptr %t273, ptr %t258, i64 %t260, i1 false), !dbg !189
  %t274 = getelementptr inbounds i8, ptr %t273, i64 %t260, !dbg !189
  call void @llvm.memcpy.p0.p0.i64(ptr %t274, ptr @.str.16, i64 1, i1 false), !dbg !189
  %t275 = getelementptr inbounds i8, ptr %t274, i64 1, !dbg !189
  call void @llvm.memcpy.p0.p0.i64(ptr %t275, ptr %t268, i64 %t270, i1 false), !dbg !189
  %t276 = getelementptr inbounds i8, ptr %t247, i64 8, !dbg !189
  store i64 %t272, ptr %t276, align 8, !dbg !189
  %t277 = getelementptr inbounds i8, ptr %t247, i64 16, !dbg !189
  store i64 %t272, ptr %t277, align 8, !dbg !189
  call void @"tests.syntax.show"(ptr %t278, ptr %t247), !dbg !188
  %t279 = load i32, ptr %t278, align 4, !dbg !188
  %t280 = icmp eq i32 %t279, 1, !dbg !188
  br i1 %t280, label %L281, label %L282, !dbg !188
L281:
  store i32 1, ptr %sret, align 4, !dbg !188
  %t283 = getelementptr inbounds i8, ptr %t278, i64 8, !dbg !188
  %t284 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !188
  call void @llvm.memcpy.p0.p0.i64(ptr %t284, ptr %t283, i64 32, i1 false), !dbg !188
  ret void, !dbg !188
L282:
  %t285 = getelementptr inbounds i8, ptr %t278, i64 8, !dbg !188
  %t286 = call ptr @ignite.alloc(i64 32, i64 8), !dbg !194
  %t287 = sub i64 0, 1, !dbg !195
  store i64 %t287, ptr %t286, align 8, !dbg !194
  %t288 = getelementptr inbounds i8, ptr %t286, i64 8, !dbg !194
  store i64 0, ptr %t288, align 8, !dbg !194
  %t289 = getelementptr inbounds i8, ptr %t286, i64 16, !dbg !194
  store i64 5, ptr %t289, align 8, !dbg !194
  %t290 = getelementptr inbounds i8, ptr %t286, i64 24, !dbg !194
  store i64 6, ptr %t290, align 8, !dbg !194
  store ptr %t286, ptr %t250, align 8, !dbg !194
  %t291 = getelementptr inbounds i8, ptr %t250, i64 8, !dbg !194
  store i64 4, ptr %t291, align 8, !dbg !194
  %t292 = getelementptr inbounds i8, ptr %t250, i64 16, !dbg !194
  store i64 4, ptr %t292, align 8, !dbg !194
  %t293 = call i64 @"tests.syntax.first_positive"(ptr %t250), !dbg !193
  call void @ignite_rt_int_to_text(ptr %t253, i64 %t293), !dbg !193
  call void @"tests.syntax.show"(ptr %t294, ptr %t253), !dbg !192
  %t295 = load i32, ptr %t294, align 4, !dbg !192
  %t296 = icmp eq i32 %t295, 1, !dbg !192
  br i1 %t296, label %L297, label %L298, !dbg !192
L297:
  store i32 1, ptr %sret, align 4, !dbg !192
  %t299 = getelementptr inbounds i8, ptr %t294, i64 8, !dbg !192
  %t300 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !192
  call void @llvm.memcpy.p0.p0.i64(ptr %t300, ptr %t299, i64 32, i1 false), !dbg !192
  ret void, !dbg !192
L298:
  %t301 = getelementptr inbounds i8, ptr %t294, i64 8, !dbg !192
  call void @"tests.syntax.sign"(ptr %t302, i64 0), !dbg !201
  %t303 = load ptr, ptr %t302, align 8, !dbg !201
  %t304 = getelementptr inbounds i8, ptr %t302, i64 8, !dbg !201
  %t305 = load i64, ptr %t304, align 8, !dbg !201
  call void @"tests.syntax.sign"(ptr %t302, i64 3), !dbg !203
  %t306 = load ptr, ptr %t302, align 8, !dbg !201
  %t307 = getelementptr inbounds i8, ptr %t302, i64 8, !dbg !201
  %t308 = load i64, ptr %t307, align 8, !dbg !201
  %t309 = sub i64 0, 3, !dbg !206
  call void @"tests.syntax.sign"(ptr %t302, i64 %t309), !dbg !205
  %t310 = load ptr, ptr %t302, align 8, !dbg !201
  %t311 = getelementptr inbounds i8, ptr %t302, i64 8, !dbg !201
  %t312 = load i64, ptr %t311, align 8, !dbg !201
  %t313 = add i64 %t305, %t308, !dbg !201
  %t314 = add i64 %t313, %t312, !dbg !201
  %t315 = call ptr @ignite.alloc(i64 %t314, i64 1), !dbg !201
  store ptr %t315, ptr %t257, align 8, !dbg !201
  call void @llvm.memcpy.p0.p0.i64(ptr %t315, ptr %t303, i64 %t305, i1 false), !dbg !201
  %t316 = getelementptr inbounds i8, ptr %t315, i64 %t305, !dbg !201
  call void @llvm.memcpy.p0.p0.i64(ptr %t316, ptr %t306, i64 %t308, i1 false), !dbg !201
  %t317 = getelementptr inbounds i8, ptr %t316, i64 %t308, !dbg !201
  call void @llvm.memcpy.p0.p0.i64(ptr %t317, ptr %t310, i64 %t312, i1 false), !dbg !201
  %t318 = getelementptr inbounds i8, ptr %t257, i64 8, !dbg !201
  store i64 %t314, ptr %t318, align 8, !dbg !201
  %t319 = getelementptr inbounds i8, ptr %t257, i64 16, !dbg !201
  store i64 %t314, ptr %t319, align 8, !dbg !201
  call void @"tests.syntax.show"(ptr %t320, ptr %t257), !dbg !200
  %t321 = load i32, ptr %t320, align 4, !dbg !200
  %t322 = icmp eq i32 %t321, 1, !dbg !200
  br i1 %t322, label %L323, label %L324, !dbg !200
L323:
  store i32 1, ptr %sret, align 4, !dbg !200
  %t325 = getelementptr inbounds i8, ptr %t320, i64 8, !dbg !200
  %t326 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !200
  call void @llvm.memcpy.p0.p0.i64(ptr %t326, ptr %t325, i64 32, i1 false), !dbg !200
  ret void, !dbg !200
L324:
  %t327 = getelementptr inbounds i8, ptr %t320, i64 8, !dbg !200
  store ptr @.str.32, ptr %t302, align 8, !dbg !210
  %t328 = getelementptr inbounds i8, ptr %t302, i64 8, !dbg !210
  store i64 6, ptr %t328, align 8, !dbg !210
  %t329 = getelementptr inbounds i8, ptr %t302, i64 16, !dbg !210
  store i64 6, ptr %t329, align 8, !dbg !210
  call void @"copy.Text"(ptr %t330, ptr %t302), !dbg !210
  %t331 = call i64 @"tests.syntax.count_spaces"(ptr %t330), !dbg !209
  call void @ignite_rt_int_to_text(ptr %t332, i64 %t331), !dbg !209
  call void @"tests.syntax.show"(ptr %t333, ptr %t332), !dbg !208
  %t334 = load i32, ptr %t333, align 4, !dbg !208
  %t335 = icmp eq i32 %t334, 1, !dbg !208
  br i1 %t335, label %L336, label %L337, !dbg !208
L336:
  store i32 1, ptr %sret, align 4, !dbg !208
  %t338 = getelementptr inbounds i8, ptr %t333, i64 8, !dbg !208
  %t339 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !208
  call void @llvm.memcpy.p0.p0.i64(ptr %t339, ptr %t338, i64 32, i1 false), !dbg !208
  ret void, !dbg !208
L337:
  %t340 = getelementptr inbounds i8, ptr %t333, i64 8, !dbg !208
  store i64 21, ptr %t341, align 8, !dbg !211
  %t342 = load i64, ptr %t341, align 8, !dbg !214
  %t343 = call i64 @"tests.syntax.twice"(i64 %t342), !dbg !214
  call void @ignite_rt_int_to_text(ptr %t344, i64 %t343), !dbg !214
  call void @"tests.syntax.show"(ptr %t345, ptr %t344), !dbg !213
  %t346 = load i32, ptr %t345, align 4, !dbg !213
  %t347 = icmp eq i32 %t346, 1, !dbg !213
  br i1 %t347, label %L348, label %L349, !dbg !213
L348:
  store i32 1, ptr %sret, align 4, !dbg !213
  %t350 = getelementptr inbounds i8, ptr %t345, i64 8, !dbg !213
  %t351 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !213
  call void @llvm.memcpy.p0.p0.i64(ptr %t351, ptr %t350, i64 32, i1 false), !dbg !213
  ret void, !dbg !213
L349:
  %t352 = getelementptr inbounds i8, ptr %t345, i64 8, !dbg !213
  store ptr @.str.33, ptr %t353, align 8, !dbg !216
  %t354 = getelementptr inbounds i8, ptr %t353, i64 8, !dbg !216
  store i64 3, ptr %t354, align 8, !dbg !216
  %t355 = getelementptr inbounds i8, ptr %t353, i64 16, !dbg !216
  store i64 3, ptr %t355, align 8, !dbg !216
  call void @"copy.Text"(ptr %t356, ptr %t353), !dbg !216
  call void @"tests.syntax.shout"(ptr %t357, ptr %t356), !dbg !216
  call void @"tests.syntax.show"(ptr %t358, ptr %t357), !dbg !215
  %t359 = load i32, ptr %t358, align 4, !dbg !215
  %t360 = icmp eq i32 %t359, 1, !dbg !215
  br i1 %t360, label %L361, label %L362, !dbg !215
L361:
  store i32 1, ptr %sret, align 4, !dbg !215
  %t363 = getelementptr inbounds i8, ptr %t358, i64 8, !dbg !215
  %t364 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !215
  call void @llvm.memcpy.p0.p0.i64(ptr %t364, ptr %t363, i64 32, i1 false), !dbg !215
  ret void, !dbg !215
L362:
  %t365 = getelementptr inbounds i8, ptr %t358, i64 8, !dbg !215
  store ptr @.str.34, ptr %t367, align 8, !dbg !219
  %t368 = getelementptr inbounds i8, ptr %t367, i64 8, !dbg !219
  store i64 1, ptr %t368, align 8, !dbg !219
  %t369 = getelementptr inbounds i8, ptr %t367, i64 16, !dbg !219
  store i64 1, ptr %t369, align 8, !dbg !219
  call void @"copy.Text"(ptr %t370, ptr %t367), !dbg !219
  call void @llvm.memcpy.p0.p0.i64(ptr %t366, ptr %t370, i64 24, i1 false), !dbg !218
  store ptr @"fnval.tests.syntax.shout", ptr %t371, align 8, !dbg !220
  %t372 = getelementptr inbounds i8, ptr %t371, i64 8, !dbg !220
  store ptr null, ptr %t372, align 8, !dbg !220
  %t373 = getelementptr inbounds i8, ptr %t366, i64 24, !dbg !218
  call void @llvm.memcpy.p0.p0.i64(ptr %t373, ptr %t371, i64 16, i1 false), !dbg !218
  call void @llvm.memcpy.p0.p0.i64(ptr %t374, ptr %t366, i64 40, i1 false), !dbg !217
  %t375 = getelementptr inbounds i8, ptr %t374, i64 24, !dbg !222
  %t376 = load ptr, ptr %t375, align 8, !dbg !222
  %t377 = getelementptr inbounds i8, ptr %t375, i64 8, !dbg !222
  %t378 = load ptr, ptr %t377, align 8, !dbg !222
  call void %t376(ptr %t378, ptr %t379, ptr %t374), !dbg !222
  call void @"tests.syntax.show"(ptr %t380, ptr %t379), !dbg !221
  %t381 = load i32, ptr %t380, align 4, !dbg !221
  %t382 = icmp eq i32 %t381, 1, !dbg !221
  br i1 %t382, label %L383, label %L384, !dbg !221
L383:
  store i32 1, ptr %sret, align 4, !dbg !221
  %t385 = getelementptr inbounds i8, ptr %t380, i64 8, !dbg !221
  %t386 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !221
  call void @llvm.memcpy.p0.p0.i64(ptr %t386, ptr %t385, i64 32, i1 false), !dbg !221
  ret void, !dbg !221
L384:
  %t387 = getelementptr inbounds i8, ptr %t380, i64 8, !dbg !221
  %t389 = call ptr @ignite.alloc(i64 24, i64 8), !dbg !226
  store ptr @.str.35, ptr %t390, align 8, !dbg !227
  %t391 = getelementptr inbounds i8, ptr %t390, i64 8, !dbg !227
  store i64 1, ptr %t391, align 8, !dbg !227
  %t392 = getelementptr inbounds i8, ptr %t390, i64 16, !dbg !227
  store i64 1, ptr %t392, align 8, !dbg !227
  call void @"copy.Text"(ptr %t393, ptr %t390), !dbg !227
  call void @llvm.memcpy.p0.p0.i64(ptr %t389, ptr %t393, i64 24, i1 false), !dbg !226
  store ptr %t389, ptr %t394, align 8, !dbg !226
  %t395 = getelementptr inbounds i8, ptr %t394, i64 8, !dbg !226
  store i64 1, ptr %t395, align 8, !dbg !226
  %t396 = getelementptr inbounds i8, ptr %t394, i64 16, !dbg !226
  store i64 1, ptr %t396, align 8, !dbg !226
  call void @llvm.memcpy.p0.p0.i64(ptr %t388, ptr %t394, i64 24, i1 false), !dbg !225
  call void @"tests.syntax.Reply.action"(ptr %t397), !dbg !225
  %t398 = getelementptr inbounds i8, ptr %t388, i64 24, !dbg !225
  call void @llvm.memcpy.p0.p0.i64(ptr %t398, ptr %t397, i64 8, i1 false), !dbg !225
  %t399 = call i64 @"tests.syntax.Reply.code"(), !dbg !225
  %t400 = getelementptr inbounds i8, ptr %t388, i64 32, !dbg !225
  store i64 %t399, ptr %t400, align 8, !dbg !225
  call void @llvm.memcpy.p0.p0.i64(ptr %t401, ptr %t388, i64 40, i1 false), !dbg !224
  %t403 = getelementptr inbounds i8, ptr %t401, i64 32, !dbg !230
  %t404 = load i64, ptr %t403, align 8, !dbg !230
  call void @ignite_rt_int_to_text(ptr %t405, i64 %t404), !dbg !230
  %t406 = load ptr, ptr %t405, align 8, !dbg !229
  %t407 = getelementptr inbounds i8, ptr %t405, i64 8, !dbg !229
  %t408 = load i64, ptr %t407, align 8, !dbg !229
  %t409 = add i64 %t408, 5, !dbg !229
  %t410 = call ptr @ignite.alloc(i64 %t409, i64 1), !dbg !229
  store ptr %t410, ptr %t402, align 8, !dbg !229
  call void @llvm.memcpy.p0.p0.i64(ptr %t410, ptr @.str.36, i64 5, i1 false), !dbg !229
  %t411 = getelementptr inbounds i8, ptr %t410, i64 5, !dbg !229
  call void @llvm.memcpy.p0.p0.i64(ptr %t411, ptr %t406, i64 %t408, i1 false), !dbg !229
  %t412 = getelementptr inbounds i8, ptr %t402, i64 8, !dbg !229
  store i64 %t409, ptr %t412, align 8, !dbg !229
  %t413 = getelementptr inbounds i8, ptr %t402, i64 16, !dbg !229
  store i64 %t409, ptr %t413, align 8, !dbg !229
  call void @"tests.syntax.show"(ptr %t414, ptr %t402), !dbg !228
  %t415 = load i32, ptr %t414, align 4, !dbg !228
  %t416 = icmp eq i32 %t415, 1, !dbg !228
  br i1 %t416, label %L417, label %L418, !dbg !228
L417:
  store i32 1, ptr %sret, align 4, !dbg !228
  %t419 = getelementptr inbounds i8, ptr %t414, i64 8, !dbg !228
  %t420 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !228
  call void @llvm.memcpy.p0.p0.i64(ptr %t420, ptr %t419, i64 32, i1 false), !dbg !228
  ret void, !dbg !228
L418:
  %t421 = getelementptr inbounds i8, ptr %t414, i64 8, !dbg !228
  %t422 = getelementptr inbounds i8, ptr %t401, i64 24, !dbg !232
  %t425 = load i32, ptr %t422, align 4, !dbg !231
  %t426 = icmp eq i32 %t425, 0, !dbg !231
  br i1 %t426, label %L427, label %L424, !dbg !231
L427:
  store ptr @.str.37, ptr %t405, align 8, !dbg !234
  %t428 = getelementptr inbounds i8, ptr %t405, i64 8, !dbg !234
  store i64 8, ptr %t428, align 8, !dbg !234
  %t429 = getelementptr inbounds i8, ptr %t405, i64 16, !dbg !234
  store i64 8, ptr %t429, align 8, !dbg !234
  call void @"copy.Text"(ptr %t430, ptr %t405), !dbg !234
  call void @"tests.syntax.show"(ptr %t431, ptr %t430), !dbg !233
  %t432 = load i32, ptr %t431, align 4, !dbg !233
  %t433 = icmp eq i32 %t432, 1, !dbg !233
  br i1 %t433, label %L434, label %L435, !dbg !233
L434:
  store i32 1, ptr %sret, align 4, !dbg !233
  %t436 = getelementptr inbounds i8, ptr %t431, i64 8, !dbg !233
  %t437 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !233
  call void @llvm.memcpy.p0.p0.i64(ptr %t437, ptr %t436, i64 32, i1 false), !dbg !233
  ret void, !dbg !233
L435:
  %t438 = getelementptr inbounds i8, ptr %t431, i64 8, !dbg !233
  br label %L423, !dbg !231
L424:
  %t440 = load i32, ptr %t422, align 4, !dbg !231
  %t441 = icmp eq i32 %t440, 1, !dbg !231
  br i1 %t441, label %L442, label %L439, !dbg !231
L442:
  store ptr @.str.38, ptr %t443, align 8, !dbg !236
  %t444 = getelementptr inbounds i8, ptr %t443, i64 8, !dbg !236
  store i64 5, ptr %t444, align 8, !dbg !236
  %t445 = getelementptr inbounds i8, ptr %t443, i64 16, !dbg !236
  store i64 5, ptr %t445, align 8, !dbg !236
  call void @"copy.Text"(ptr %t446, ptr %t443), !dbg !236
  call void @"tests.syntax.show"(ptr %t447, ptr %t446), !dbg !235
  %t448 = load i32, ptr %t447, align 4, !dbg !235
  %t449 = icmp eq i32 %t448, 1, !dbg !235
  br i1 %t449, label %L450, label %L451, !dbg !235
L450:
  store i32 1, ptr %sret, align 4, !dbg !235
  %t452 = getelementptr inbounds i8, ptr %t447, i64 8, !dbg !235
  %t453 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !235
  call void @llvm.memcpy.p0.p0.i64(ptr %t453, ptr %t452, i64 32, i1 false), !dbg !235
  ret void, !dbg !235
L451:
  %t454 = getelementptr inbounds i8, ptr %t447, i64 8, !dbg !235
  br label %L423, !dbg !231
L439:
  store ptr @.str.0, ptr %t455, align 8, !dbg !231
  %t456 = getelementptr inbounds i8, ptr %t455, i64 8, !dbg !231
  store i64 26, ptr %t456, align 8, !dbg !231
  %t457 = getelementptr inbounds i8, ptr %t455, i64 16, !dbg !231
  store i64 26, ptr %t457, align 8, !dbg !231
  call void @ignite_rt_fail_match(ptr %t455, i64 140, i64 5), !dbg !231
  unreachable, !dbg !231
L423:
  %t459 = zext i1 true to i8, !dbg !237
  store i8 %t459, ptr %t458, align 1, !dbg !237
  %t460 = call i8 @"tests.syntax.SPACE"(), !dbg !240
  store i8 %t460, ptr %t461, align 1, !dbg !239
  %t463 = load i8, ptr %t458, align 1, !dbg !242
  %t464 = trunc i8 %t463 to i1, !dbg !242
  store ptr @.str.30, ptr %t465, align 8, !dbg !242
  %t466 = getelementptr inbounds i8, ptr %t465, i64 8, !dbg !242
  store i64 4, ptr %t466, align 8, !dbg !242
  %t467 = getelementptr inbounds i8, ptr %t465, i64 16, !dbg !242
  store i64 4, ptr %t467, align 8, !dbg !242
  store ptr @.str.31, ptr %t468, align 8, !dbg !242
  %t469 = getelementptr inbounds i8, ptr %t468, i64 8, !dbg !242
  store i64 5, ptr %t469, align 8, !dbg !242
  %t470 = getelementptr inbounds i8, ptr %t468, i64 16, !dbg !242
  store i64 5, ptr %t470, align 8, !dbg !242
  %t471 = select i1 %t464, ptr %t465, ptr %t468, !dbg !242
  call void @"copy.Text"(ptr %t472, ptr %t471), !dbg !242
  %t473 = load ptr, ptr %t472, align 8, !dbg !242
  %t474 = getelementptr inbounds i8, ptr %t472, i64 8, !dbg !242
  %t475 = load i64, ptr %t474, align 8, !dbg !242
  %t476 = load i8, ptr %t461, align 1, !dbg !243
  %t477 = zext i8 %t476 to i64, !dbg !243
  call void @ignite_rt_int_to_text(ptr %t472, i64 %t477), !dbg !243
  %t478 = load ptr, ptr %t472, align 8, !dbg !242
  %t479 = getelementptr inbounds i8, ptr %t472, i64 8, !dbg !242
  %t480 = load i64, ptr %t479, align 8, !dbg !242
  store ptr @.str.39, ptr %t472, align 8, !dbg !244
  %t481 = getelementptr inbounds i8, ptr %t472, i64 8, !dbg !244
  store i64 1, ptr %t481, align 8, !dbg !244
  %t482 = getelementptr inbounds i8, ptr %t472, i64 16, !dbg !244
  store i64 1, ptr %t482, align 8, !dbg !244
  call void @"copy.Text"(ptr %t468, ptr %t472), !dbg !244
  %t483 = load ptr, ptr %t468, align 8, !dbg !242
  %t484 = getelementptr inbounds i8, ptr %t468, i64 8, !dbg !242
  %t485 = load i64, ptr %t484, align 8, !dbg !242
  %t486 = add i64 %t475, %t480, !dbg !242
  %t487 = add i64 %t486, %t485, !dbg !242
  %t488 = add i64 %t487, 2, !dbg !242
  %t489 = call ptr @ignite.alloc(i64 %t488, i64 1), !dbg !242
  store ptr %t489, ptr %t462, align 8, !dbg !242
  call void @llvm.memcpy.p0.p0.i64(ptr %t489, ptr %t473, i64 %t475, i1 false), !dbg !242
  %t490 = getelementptr inbounds i8, ptr %t489, i64 %t475, !dbg !242
  call void @llvm.memcpy.p0.p0.i64(ptr %t490, ptr @.str.16, i64 1, i1 false), !dbg !242
  %t491 = getelementptr inbounds i8, ptr %t490, i64 1, !dbg !242
  call void @llvm.memcpy.p0.p0.i64(ptr %t491, ptr %t478, i64 %t480, i1 false), !dbg !242
  %t492 = getelementptr inbounds i8, ptr %t491, i64 %t480, !dbg !242
  call void @llvm.memcpy.p0.p0.i64(ptr %t492, ptr @.str.16, i64 1, i1 false), !dbg !242
  %t493 = getelementptr inbounds i8, ptr %t492, i64 1, !dbg !242
  call void @llvm.memcpy.p0.p0.i64(ptr %t493, ptr %t483, i64 %t485, i1 false), !dbg !242
  %t494 = getelementptr inbounds i8, ptr %t462, i64 8, !dbg !242
  store i64 %t488, ptr %t494, align 8, !dbg !242
  %t495 = getelementptr inbounds i8, ptr %t462, i64 16, !dbg !242
  store i64 %t488, ptr %t495, align 8, !dbg !242
  call void @"tests.syntax.show"(ptr %t496, ptr %t462), !dbg !241
  %t497 = load i32, ptr %t496, align 4, !dbg !241
  %t498 = icmp eq i32 %t497, 1, !dbg !241
  br i1 %t498, label %L499, label %L500, !dbg !241
L499:
  store i32 1, ptr %sret, align 4, !dbg !241
  %t501 = getelementptr inbounds i8, ptr %t496, i64 8, !dbg !241
  %t502 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !241
  call void @llvm.memcpy.p0.p0.i64(ptr %t502, ptr %t501, i64 32, i1 false), !dbg !241
  ret void, !dbg !241
L500:
  %t503 = getelementptr inbounds i8, ptr %t496, i64 8, !dbg !241
  store i64 0, ptr %t504, align 8, !dbg !246
  store ptr @ignite.empty, ptr %t468, align 8, !dbg !248
  %t505 = getelementptr inbounds i8, ptr %t468, i64 8, !dbg !248
  store i64 0, ptr %t505, align 8, !dbg !248
  %t506 = getelementptr inbounds i8, ptr %t468, i64 16, !dbg !248
  store i64 0, ptr %t506, align 8, !dbg !248
  call void @"copy.Text"(ptr %t472, ptr %t468), !dbg !248
  %t507 = getelementptr inbounds i8, ptr %t504, i64 8, !dbg !246
  call void @llvm.memcpy.p0.p0.i64(ptr %t507, ptr %t472, i64 24, i1 false), !dbg !246
  call void @llvm.memcpy.p0.p0.i64(ptr %t508, ptr %t504, i64 32, i1 false), !dbg !245
  store i64 0, ptr %t509, align 8, !dbg !249
  %t511 = call i64 @ignite.arena_mark(), !dbg !251
  store i64 0, ptr %t512, align 8, !dbg !251
  br label %L513
L513:
  %t517 = load i64, ptr %t509, align 8, !dbg !252
  %t518 = icmp slt i64 %t517, 3000, !dbg !252
  br i1 %t518, label %L514, label %L516, !dbg !251
L514:
  call void @"copy.(Int,Text)"(ptr %t519, ptr %t508), !dbg !254
  %t522 = load i64, ptr %t519, align 8, !dbg !254
  store i64 %t522, ptr %t523, align 8, !dbg !254
  %t524 = getelementptr inbounds i8, ptr %t519, i64 8, !dbg !254
  br label %L520, !dbg !254
L521:
  store ptr @.str.0, ptr %t465, align 8, !dbg !254
  %t525 = getelementptr inbounds i8, ptr %t465, i64 8, !dbg !254
  store i64 26, ptr %t525, align 8, !dbg !254
  %t526 = getelementptr inbounds i8, ptr %t465, i64 16, !dbg !254
  store i64 26, ptr %t526, align 8, !dbg !254
  call void @ignite_rt_fail_match(ptr %t465, i64 150, i64 11), !dbg !254
  unreachable, !dbg !254
L520:
  %t528 = load i64, ptr %t523, align 8, !dbg !258
  %t529 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t528, i64 1), !dbg !258
  %t530 = extractvalue { i64, i1 } %t529, 1, !dbg !258
  br i1 %t530, label %L531, label %L532, !dbg !258
L531:
  store ptr @.str.0, ptr %t533, align 8, !dbg !258
  %t534 = getelementptr inbounds i8, ptr %t533, i64 8, !dbg !258
  store i64 26, ptr %t534, align 8, !dbg !258
  %t535 = getelementptr inbounds i8, ptr %t533, i64 16, !dbg !258
  store i64 26, ptr %t535, align 8, !dbg !258
  store ptr @.str.1, ptr %t536, align 8, !dbg !258
  %t537 = getelementptr inbounds i8, ptr %t536, i64 8, !dbg !258
  store i64 1, ptr %t537, align 8, !dbg !258
  %t538 = getelementptr inbounds i8, ptr %t536, i64 16, !dbg !258
  store i64 1, ptr %t538, align 8, !dbg !258
  call void @ignite_rt_fail_arith(ptr %t536, ptr %t533, i64 151, i64 15, i64 %t528, i64 1), !dbg !258
  unreachable, !dbg !258
L532:
  %t539 = extractvalue { i64, i1 } %t529, 0, !dbg !258
  store i64 %t539, ptr %t527, align 8, !dbg !257
  %t540 = load ptr, ptr %t524, align 8, !dbg !260
  %t541 = getelementptr inbounds i8, ptr %t524, i64 8, !dbg !260
  %t542 = load i64, ptr %t541, align 8, !dbg !260
  %t543 = add i64 %t542, 1, !dbg !260
  %t544 = call ptr @ignite.alloc(i64 %t543, i64 1), !dbg !260
  store ptr %t544, ptr %t536, align 8, !dbg !260
  call void @llvm.memcpy.p0.p0.i64(ptr %t544, ptr %t540, i64 %t542, i1 false), !dbg !260
  %t545 = getelementptr inbounds i8, ptr %t544, i64 %t542, !dbg !260
  call void @llvm.memcpy.p0.p0.i64(ptr %t545, ptr @.str.26, i64 1, i1 false), !dbg !260
  %t546 = getelementptr inbounds i8, ptr %t536, i64 8, !dbg !260
  store i64 %t543, ptr %t546, align 8, !dbg !260
  %t547 = getelementptr inbounds i8, ptr %t536, i64 16, !dbg !260
  store i64 %t543, ptr %t547, align 8, !dbg !260
  %t548 = getelementptr inbounds i8, ptr %t527, i64 8, !dbg !257
  call void @llvm.memcpy.p0.p0.i64(ptr %t548, ptr %t536, i64 24, i1 false), !dbg !257
  call void @llvm.memcpy.p0.p0.i64(ptr %t508, ptr %t527, i64 32, i1 false), !dbg !256
  %t549 = load i64, ptr %t509, align 8, !dbg !262
  %t550 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t549, i64 1), !dbg !262
  %t551 = extractvalue { i64, i1 } %t550, 1, !dbg !262
  br i1 %t551, label %L552, label %L553, !dbg !262
L552:
  store ptr @.str.0, ptr %t533, align 8, !dbg !262
  %t554 = getelementptr inbounds i8, ptr %t533, i64 8, !dbg !262
  store i64 26, ptr %t554, align 8, !dbg !262
  %t555 = getelementptr inbounds i8, ptr %t533, i64 16, !dbg !262
  store i64 26, ptr %t555, align 8, !dbg !262
  store ptr @.str.1, ptr %t556, align 8, !dbg !262
  %t557 = getelementptr inbounds i8, ptr %t556, i64 8, !dbg !262
  store i64 1, ptr %t557, align 8, !dbg !262
  %t558 = getelementptr inbounds i8, ptr %t556, i64 16, !dbg !262
  store i64 1, ptr %t558, align 8, !dbg !262
  call void @ignite_rt_fail_arith(ptr %t556, ptr %t533, i64 152, i64 11, i64 %t549, i64 1), !dbg !262
  unreachable, !dbg !262
L553:
  %t559 = extractvalue { i64, i1 } %t550, 0, !dbg !262
  store i64 %t559, ptr %t509, align 8, !dbg !261
  br label %L515
L515:
  %t560 = load i64, ptr %t512, align 8, !dbg !251
  %t561 = call i1 @ignite.arena_due(i64 %t511, i64 %t560), !dbg !251
  br i1 %t561, label %L562, label %L513, !dbg !251
L562:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !251
  call void @"copy.(Int,Text)"(ptr %t510, ptr %t508), !dbg !251
  call void @ignite_rt_arena_scratch(i8 0), !dbg !251
  call void @ignite_rt_arena_release(i64 %t511), !dbg !251
  call void @"copy.(Int,Text)"(ptr %t508, ptr %t510), !dbg !251
  call void @ignite_rt_arena_scratch_clear(), !dbg !251
  %t563 = call i64 @ignite.arena_mark(), !dbg !251
  %t564 = sub i64 %t563, %t511, !dbg !251
  store i64 %t564, ptr %t512, align 8, !dbg !251
  br label %L513, !dbg !251
L516:
  call void @"copy.(Int,Text)"(ptr %t565, ptr %t508), !dbg !264
  %t568 = load i64, ptr %t565, align 8, !dbg !264
  store i64 %t568, ptr %t569, align 8, !dbg !264
  %t570 = getelementptr inbounds i8, ptr %t565, i64 8, !dbg !264
  br label %L566, !dbg !264
L567:
  store ptr @.str.0, ptr %t556, align 8, !dbg !264
  %t571 = getelementptr inbounds i8, ptr %t556, i64 8, !dbg !264
  store i64 26, ptr %t571, align 8, !dbg !264
  %t572 = getelementptr inbounds i8, ptr %t556, i64 16, !dbg !264
  store i64 26, ptr %t572, align 8, !dbg !264
  call void @ignite_rt_fail_match(ptr %t556, i64 154, i64 9), !dbg !264
  unreachable, !dbg !264
L566:
  %t573 = getelementptr inbounds i8, ptr %t570, i64 8, !dbg !267
  %t574 = load i64, ptr %t573, align 8, !dbg !267
  store i64 %t574, ptr %t575, align 8, !dbg !266
  %t576 = load i64, ptr %t569, align 8, !dbg !270
  call void @ignite_rt_int_to_text(ptr %t577, i64 %t576), !dbg !270
  %t578 = load ptr, ptr %t577, align 8, !dbg !269
  %t579 = getelementptr inbounds i8, ptr %t577, i64 8, !dbg !269
  %t580 = load i64, ptr %t579, align 8, !dbg !269
  %t581 = load i64, ptr %t575, align 8, !dbg !271
  call void @ignite_rt_int_to_text(ptr %t577, i64 %t581), !dbg !271
  %t582 = load ptr, ptr %t577, align 8, !dbg !269
  %t583 = getelementptr inbounds i8, ptr %t577, i64 8, !dbg !269
  %t584 = load i64, ptr %t583, align 8, !dbg !269
  %t585 = add i64 %t580, %t584, !dbg !269
  %t586 = add i64 %t585, 1, !dbg !269
  %t587 = call ptr @ignite.alloc(i64 %t586, i64 1), !dbg !269
  store ptr %t587, ptr %t533, align 8, !dbg !269
  call void @llvm.memcpy.p0.p0.i64(ptr %t587, ptr %t578, i64 %t580, i1 false), !dbg !269
  %t588 = getelementptr inbounds i8, ptr %t587, i64 %t580, !dbg !269
  call void @llvm.memcpy.p0.p0.i64(ptr %t588, ptr @.str.16, i64 1, i1 false), !dbg !269
  %t589 = getelementptr inbounds i8, ptr %t588, i64 1, !dbg !269
  call void @llvm.memcpy.p0.p0.i64(ptr %t589, ptr %t582, i64 %t584, i1 false), !dbg !269
  %t590 = getelementptr inbounds i8, ptr %t533, i64 8, !dbg !269
  store i64 %t586, ptr %t590, align 8, !dbg !269
  %t591 = getelementptr inbounds i8, ptr %t533, i64 16, !dbg !269
  store i64 %t586, ptr %t591, align 8, !dbg !269
  call void @"tests.syntax.show"(ptr %t592, ptr %t533), !dbg !268
  %t593 = load i32, ptr %t592, align 4, !dbg !268
  %t594 = icmp eq i32 %t593, 1, !dbg !268
  br i1 %t594, label %L595, label %L596, !dbg !268
L595:
  store i32 1, ptr %sret, align 4, !dbg !268
  %t597 = getelementptr inbounds i8, ptr %t592, i64 8, !dbg !268
  %t598 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !268
  call void @llvm.memcpy.p0.p0.i64(ptr %t598, ptr %t597, i64 32, i1 false), !dbg !268
  ret void, !dbg !268
L596:
  %t599 = getelementptr inbounds i8, ptr %t592, i64 8, !dbg !268
  store i64 1, ptr %t600, align 8, !dbg !273
  store ptr @.str.18, ptr %t577, align 8, !dbg !275
  %t601 = getelementptr inbounds i8, ptr %t577, i64 8, !dbg !275
  store i64 1, ptr %t601, align 8, !dbg !275
  %t602 = getelementptr inbounds i8, ptr %t577, i64 16, !dbg !275
  store i64 1, ptr %t602, align 8, !dbg !275
  call void @"copy.Text"(ptr %t603, ptr %t577), !dbg !275
  %t604 = getelementptr inbounds i8, ptr %t600, i64 8, !dbg !273
  call void @llvm.memcpy.p0.p0.i64(ptr %t604, ptr %t603, i64 24, i1 false), !dbg !273
  call void @llvm.memcpy.p0.p0.i64(ptr %t605, ptr %t600, i64 32, i1 false), !dbg !272
  call void @"copy.(Int,Text)"(ptr %t606, ptr %t605), !dbg !276
  %t609 = load i64, ptr %t606, align 8, !dbg !276
  store i64 %t609, ptr %t610, align 8, !dbg !276
  %t611 = getelementptr inbounds i8, ptr %t606, i64 8, !dbg !276
  br label %L607, !dbg !276
L608:
  store ptr @.str.0, ptr %t612, align 8, !dbg !276
  %t613 = getelementptr inbounds i8, ptr %t612, i64 8, !dbg !276
  store i64 26, ptr %t613, align 8, !dbg !276
  %t614 = getelementptr inbounds i8, ptr %t612, i64 16, !dbg !276
  store i64 26, ptr %t614, align 8, !dbg !276
  call void @ignite_rt_fail_match(ptr %t612, i64 158, i64 9), !dbg !276
  unreachable, !dbg !276
L607:
  %t615 = load i64, ptr %t610, align 8, !dbg !279
  %t616 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t615, i64 1), !dbg !279
  %t617 = extractvalue { i64, i1 } %t616, 1, !dbg !279
  br i1 %t617, label %L618, label %L619, !dbg !279
L618:
  store ptr @.str.0, ptr %t620, align 8, !dbg !279
  %t621 = getelementptr inbounds i8, ptr %t620, i64 8, !dbg !279
  store i64 26, ptr %t621, align 8, !dbg !279
  %t622 = getelementptr inbounds i8, ptr %t620, i64 16, !dbg !279
  store i64 26, ptr %t622, align 8, !dbg !279
  store ptr @.str.1, ptr %t623, align 8, !dbg !279
  %t624 = getelementptr inbounds i8, ptr %t623, i64 8, !dbg !279
  store i64 1, ptr %t624, align 8, !dbg !279
  %t625 = getelementptr inbounds i8, ptr %t623, i64 16, !dbg !279
  store i64 1, ptr %t625, align 8, !dbg !279
  call void @ignite_rt_fail_arith(ptr %t623, ptr %t620, i64 159, i64 9, i64 %t615, i64 1), !dbg !279
  unreachable, !dbg !279
L619:
  %t626 = extractvalue { i64, i1 } %t616, 0, !dbg !279
  store i64 %t626, ptr %t610, align 8, !dbg !278
  %t627 = load ptr, ptr %t611, align 8, !dbg !282
  %t628 = getelementptr inbounds i8, ptr %t611, i64 8, !dbg !282
  %t629 = load i64, ptr %t628, align 8, !dbg !282
  %t630 = add i64 %t629, 1, !dbg !282
  %t631 = call ptr @ignite.alloc(i64 %t630, i64 1), !dbg !282
  store ptr %t631, ptr %t623, align 8, !dbg !282
  call void @llvm.memcpy.p0.p0.i64(ptr %t631, ptr %t627, i64 %t629, i1 false), !dbg !282
  %t632 = getelementptr inbounds i8, ptr %t631, i64 %t629, !dbg !282
  call void @llvm.memcpy.p0.p0.i64(ptr %t632, ptr @.str.28, i64 1, i1 false), !dbg !282
  %t633 = getelementptr inbounds i8, ptr %t623, i64 8, !dbg !282
  store i64 %t630, ptr %t633, align 8, !dbg !282
  %t634 = getelementptr inbounds i8, ptr %t623, i64 16, !dbg !282
  store i64 %t630, ptr %t634, align 8, !dbg !282
  call void @llvm.memcpy.p0.p0.i64(ptr %t611, ptr %t623, i64 24, i1 false), !dbg !281
  %t635 = load i64, ptr %t610, align 8, !dbg !285
  call void @ignite_rt_int_to_text(ptr %t636, i64 %t635), !dbg !285
  %t637 = load ptr, ptr %t636, align 8, !dbg !284
  %t638 = getelementptr inbounds i8, ptr %t636, i64 8, !dbg !284
  %t639 = load i64, ptr %t638, align 8, !dbg !284
  call void @"copy.Text"(ptr %t636, ptr %t611), !dbg !286
  %t640 = load ptr, ptr %t636, align 8, !dbg !284
  %t641 = getelementptr inbounds i8, ptr %t636, i64 8, !dbg !284
  %t642 = load i64, ptr %t641, align 8, !dbg !284
  %t643 = add i64 %t639, %t642, !dbg !284
  %t644 = call ptr @ignite.alloc(i64 %t643, i64 1), !dbg !284
  store ptr %t644, ptr %t620, align 8, !dbg !284
  call void @llvm.memcpy.p0.p0.i64(ptr %t644, ptr %t637, i64 %t639, i1 false), !dbg !284
  %t645 = getelementptr inbounds i8, ptr %t644, i64 %t639, !dbg !284
  call void @llvm.memcpy.p0.p0.i64(ptr %t645, ptr %t640, i64 %t642, i1 false), !dbg !284
  %t646 = getelementptr inbounds i8, ptr %t620, i64 8, !dbg !284
  store i64 %t643, ptr %t646, align 8, !dbg !284
  %t647 = getelementptr inbounds i8, ptr %t620, i64 16, !dbg !284
  store i64 %t643, ptr %t647, align 8, !dbg !284
  call void @"tests.syntax.show"(ptr %t648, ptr %t620), !dbg !283
  %t649 = load i32, ptr %t648, align 4, !dbg !283
  %t650 = icmp eq i32 %t649, 1, !dbg !283
  br i1 %t650, label %L651, label %L652, !dbg !283
L651:
  store i32 1, ptr %sret, align 4, !dbg !283
  %t653 = getelementptr inbounds i8, ptr %t648, i64 8, !dbg !283
  %t654 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !283
  call void @llvm.memcpy.p0.p0.i64(ptr %t654, ptr %t653, i64 32, i1 false), !dbg !283
  ret void, !dbg !283
L652:
  %t655 = getelementptr inbounds i8, ptr %t648, i64 8, !dbg !283
  %t656 = call i64 @"tests.syntax.twice"(i64 1), !dbg !288
  br label %L657, !dbg !287
L658:
  store ptr @.str.0, ptr %t636, align 8, !dbg !287
  %t659 = getelementptr inbounds i8, ptr %t636, i64 8, !dbg !287
  store i64 26, ptr %t659, align 8, !dbg !287
  %t660 = getelementptr inbounds i8, ptr %t636, i64 16, !dbg !287
  store i64 26, ptr %t660, align 8, !dbg !287
  call void @ignite_rt_fail_match(ptr %t636, i64 162, i64 9), !dbg !287
  unreachable, !dbg !287
L657:
  %t661 = call i64 @"tests.syntax.twice"(i64 2), !dbg !291
  br label %L662, !dbg !290
L663:
  store ptr @.str.0, ptr %t664, align 8, !dbg !290
  %t665 = getelementptr inbounds i8, ptr %t664, i64 8, !dbg !290
  store i64 26, ptr %t665, align 8, !dbg !290
  %t666 = getelementptr inbounds i8, ptr %t664, i64 16, !dbg !290
  store i64 26, ptr %t666, align 8, !dbg !290
  call void @ignite_rt_fail_match(ptr %t664, i64 163, i64 5), !dbg !290
  unreachable, !dbg !290
L662:
  store i32 0, ptr %t667, align 4, !dbg !294
  call void @llvm.memcpy.p0.p0.i64(ptr %t668, ptr %t667, i64 8, i1 false), !dbg !293
  %t671 = load i32, ptr %t668, align 4, !dbg !295
  %t672 = icmp eq i32 %t671, 0, !dbg !295
  br i1 %t672, label %L673, label %L670, !dbg !295
L673:
  store ptr @.str.40, ptr %t674, align 8, !dbg !298
  %t675 = getelementptr inbounds i8, ptr %t674, i64 8, !dbg !298
  store i64 9, ptr %t675, align 8, !dbg !298
  %t676 = getelementptr inbounds i8, ptr %t674, i64 16, !dbg !298
  store i64 9, ptr %t676, align 8, !dbg !298
  call void @"copy.Text"(ptr %t677, ptr %t674), !dbg !298
  call void @"tests.syntax.show"(ptr %t678, ptr %t677), !dbg !297
  %t679 = load i32, ptr %t678, align 4, !dbg !297
  %t680 = icmp eq i32 %t679, 1, !dbg !297
  br i1 %t680, label %L681, label %L682, !dbg !297
L681:
  store i32 1, ptr %sret, align 4, !dbg !297
  %t683 = getelementptr inbounds i8, ptr %t678, i64 8, !dbg !297
  %t684 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !297
  call void @llvm.memcpy.p0.p0.i64(ptr %t684, ptr %t683, i64 32, i1 false), !dbg !297
  ret void, !dbg !297
L682:
  %t685 = getelementptr inbounds i8, ptr %t678, i64 8, !dbg !297
  br label %L669, !dbg !295
L670:
  %t687 = load i32, ptr %t668, align 4, !dbg !295
  %t688 = icmp eq i32 %t687, 1, !dbg !295
  br i1 %t688, label %L689, label %L686, !dbg !295
L689:
  store ptr @.str.41, ptr %t690, align 8, !dbg !300
  %t691 = getelementptr inbounds i8, ptr %t690, i64 8, !dbg !300
  store i64 8, ptr %t691, align 8, !dbg !300
  %t692 = getelementptr inbounds i8, ptr %t690, i64 16, !dbg !300
  store i64 8, ptr %t692, align 8, !dbg !300
  call void @"copy.Text"(ptr %t693, ptr %t690), !dbg !300
  call void @"tests.syntax.show"(ptr %t694, ptr %t693), !dbg !299
  %t695 = load i32, ptr %t694, align 4, !dbg !299
  %t696 = icmp eq i32 %t695, 1, !dbg !299
  br i1 %t696, label %L697, label %L698, !dbg !299
L697:
  store i32 1, ptr %sret, align 4, !dbg !299
  %t699 = getelementptr inbounds i8, ptr %t694, i64 8, !dbg !299
  %t700 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !299
  call void @llvm.memcpy.p0.p0.i64(ptr %t700, ptr %t699, i64 32, i1 false), !dbg !299
  ret void, !dbg !299
L698:
  %t701 = getelementptr inbounds i8, ptr %t694, i64 8, !dbg !299
  br label %L669, !dbg !295
L686:
  store ptr @.str.0, ptr %t702, align 8, !dbg !295
  %t703 = getelementptr inbounds i8, ptr %t702, i64 8, !dbg !295
  store i64 26, ptr %t703, align 8, !dbg !295
  %t704 = getelementptr inbounds i8, ptr %t702, i64 16, !dbg !295
  store i64 26, ptr %t704, align 8, !dbg !295
  call void @ignite_rt_fail_match(ptr %t702, i64 165, i64 5), !dbg !295
  unreachable, !dbg !295
L669:
  call void @"tests.syntax.lower_case"(ptr %t706, i64 3), !dbg !302
  %t707 = load ptr, ptr %t706, align 8, !dbg !302
  %t708 = getelementptr inbounds i8, ptr %t706, i64 8, !dbg !302
  %t709 = load i64, ptr %t708, align 8, !dbg !302
  call void @"tests.syntax.lower_case"(ptr %t706, i64 4), !dbg !304
  %t710 = load ptr, ptr %t706, align 8, !dbg !302
  %t711 = getelementptr inbounds i8, ptr %t706, i64 8, !dbg !302
  %t712 = load i64, ptr %t711, align 8, !dbg !302
  %t713 = add i64 %t709, %t712, !dbg !302
  %t714 = add i64 %t713, 1, !dbg !302
  %t715 = call ptr @ignite.alloc(i64 %t714, i64 1), !dbg !302
  store ptr %t715, ptr %t705, align 8, !dbg !302
  call void @llvm.memcpy.p0.p0.i64(ptr %t715, ptr %t707, i64 %t709, i1 false), !dbg !302
  %t716 = getelementptr inbounds i8, ptr %t715, i64 %t709, !dbg !302
  call void @llvm.memcpy.p0.p0.i64(ptr %t716, ptr @.str.16, i64 1, i1 false), !dbg !302
  %t717 = getelementptr inbounds i8, ptr %t716, i64 1, !dbg !302
  call void @llvm.memcpy.p0.p0.i64(ptr %t717, ptr %t710, i64 %t712, i1 false), !dbg !302
  %t718 = getelementptr inbounds i8, ptr %t705, i64 8, !dbg !302
  store i64 %t714, ptr %t718, align 8, !dbg !302
  %t719 = getelementptr inbounds i8, ptr %t705, i64 16, !dbg !302
  store i64 %t714, ptr %t719, align 8, !dbg !302
  call void @"tests.syntax.show"(ptr %t720, ptr %t705), !dbg !301
  %t721 = load i32, ptr %t720, align 4, !dbg !301
  %t722 = icmp eq i32 %t721, 1, !dbg !301
  br i1 %t722, label %L723, label %L724, !dbg !301
L723:
  store i32 1, ptr %sret, align 4, !dbg !301
  %t725 = getelementptr inbounds i8, ptr %t720, i64 8, !dbg !301
  %t726 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !301
  call void @llvm.memcpy.p0.p0.i64(ptr %t726, ptr %t725, i64 32, i1 false), !dbg !301
  ret void, !dbg !301
L724:
  %t727 = getelementptr inbounds i8, ptr %t720, i64 8, !dbg !301
  store i32 0, ptr %t728, align 4, !dbg !306
  %t729 = getelementptr inbounds i8, ptr %t728, i64 8, !dbg !306
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t728, i64 40, i1 false), !dbg !117
  ret void, !dbg !117
}

define void @"tests.syntax.Runner.main"(ptr %sret, ptr %p0) !dbg !308 {
entry:
  %t3 = alloca [40 x i8], align 8
  %t4 = alloca [8 x i8], align 8
  %t5 = alloca [32 x i8], align 8
  %t13 = alloca [8 x i8], align 8
  %t21 = alloca [8 x i8], align 8
  %t28 = alloca [32 x i8], align 8
  %t31 = alloca [40 x i8], align 8
  %t34 = alloca [24 x i8], align 8
  %t38 = alloca [8 x i8], align 8
  %t40 = alloca [40 x i8], align 8
  %t41 = alloca [40 x i8], align 8
  %t48 = alloca [40 x i8], align 8
  %t55 = alloca [32 x i8], align 8
  %t58 = alloca [40 x i8], align 8
  %t60 = alloca [24 x i8], align 8
  %t1 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !314
  %t2 = load i64, ptr %t1, align 8, !dbg !314
  %t6 = call i32 @ignite_rt_tcp_connect(ptr %p0, i64 %t2, ptr %t4, ptr %t5), !dbg !312
  %t7 = icmp ne i32 %t6, 0, !dbg !312
  br i1 %t7, label %L8, label %L9, !dbg !312
L8:
  store i32 1, ptr %t3, align 4, !dbg !312
  %t11 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !312
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t5, i64 32, i1 false), !dbg !312
  br label %L10, !dbg !312
L9:
  store i32 0, ptr %t3, align 4, !dbg !312
  %t12 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !312
  call void @llvm.memcpy.p0.p0.i64(ptr %t12, ptr %t4, i64 8, i1 false), !dbg !312
  br label %L10, !dbg !312
L10:
  %t16 = load i32, ptr %t3, align 4, !dbg !311
  %t17 = icmp eq i32 %t16, 0, !dbg !311
  br i1 %t17, label %L18, label %L15, !dbg !311
L18:
  %t19 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !311
  %t20 = load i64, ptr %t19, align 8, !dbg !311
  store i64 %t20, ptr %t21, align 8, !dbg !311
  %t22 = load i64, ptr %t21, align 8, !dbg !315
  store i64 %t22, ptr %t13, align 8, !dbg !311
  br label %L14, !dbg !311
L15:
  %t24 = load i32, ptr %t3, align 4, !dbg !311
  %t25 = icmp eq i32 %t24, 1, !dbg !311
  br i1 %t25, label %L26, label %L23, !dbg !311
L26:
  %t27 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !311
  call void @"copy.IoError"(ptr %t28, ptr %t27), !dbg !318
  %t29 = trunc i64 3 to i32, !dbg !318
  %t30 = getelementptr inbounds i8, ptr %t28, i64 24, !dbg !318
  store i32 %t29, ptr %t30, align 4, !dbg !318
  store i32 1, ptr %t31, align 4, !dbg !317
  %t32 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !317
  call void @llvm.memcpy.p0.p0.i64(ptr %t32, ptr %t28, i64 32, i1 false), !dbg !317
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t31, i64 40, i1 false), !dbg !316
  ret void, !dbg !316
L33:
  br label %L14, !dbg !311
L23:
  store ptr @.str.0, ptr %t34, align 8, !dbg !311
  %t35 = getelementptr inbounds i8, ptr %t34, i64 8, !dbg !311
  store i64 26, ptr %t35, align 8, !dbg !311
  %t36 = getelementptr inbounds i8, ptr %t34, i64 16, !dbg !311
  store i64 26, ptr %t36, align 8, !dbg !311
  call void @ignite_rt_fail_match(ptr %t34, i64 180, i64 16), !dbg !311
  unreachable, !dbg !311
L14:
  %t37 = load i64, ptr %t13, align 8, !dbg !311
  store i64 %t37, ptr %t38, align 8, !dbg !310
  %t39 = load i64, ptr %t38, align 8, !dbg !322
  call void @"tests.syntax.Syntax.connect"(ptr %t40, i64 %t39), !dbg !321
  %t44 = load i32, ptr %t40, align 4, !dbg !320
  %t45 = icmp eq i32 %t44, 0, !dbg !320
  br i1 %t45, label %L46, label %L43, !dbg !320
L46:
  %t47 = getelementptr inbounds i8, ptr %t40, i64 8, !dbg !320
  store i32 0, ptr %t48, align 4, !dbg !323
  %t49 = getelementptr inbounds i8, ptr %t48, i64 8, !dbg !323
  call void @llvm.memcpy.p0.p0.i64(ptr %t41, ptr %t48, i64 40, i1 false), !dbg !320
  br label %L42, !dbg !320
L43:
  %t51 = load i32, ptr %t40, align 4, !dbg !320
  %t52 = icmp eq i32 %t51, 1, !dbg !320
  br i1 %t52, label %L53, label %L50, !dbg !320
L53:
  %t54 = getelementptr inbounds i8, ptr %t40, i64 8, !dbg !320
  call void @"copy.IoError"(ptr %t55, ptr %t54), !dbg !326
  %t56 = trunc i64 4 to i32, !dbg !326
  %t57 = getelementptr inbounds i8, ptr %t55, i64 24, !dbg !326
  store i32 %t56, ptr %t57, align 4, !dbg !326
  store i32 1, ptr %t58, align 4, !dbg !325
  %t59 = getelementptr inbounds i8, ptr %t58, i64 8, !dbg !325
  call void @llvm.memcpy.p0.p0.i64(ptr %t59, ptr %t55, i64 32, i1 false), !dbg !325
  call void @llvm.memcpy.p0.p0.i64(ptr %t41, ptr %t58, i64 40, i1 false), !dbg !320
  br label %L42, !dbg !320
L50:
  store ptr @.str.0, ptr %t60, align 8, !dbg !320
  %t61 = getelementptr inbounds i8, ptr %t60, i64 8, !dbg !320
  store i64 26, ptr %t61, align 8, !dbg !320
  %t62 = getelementptr inbounds i8, ptr %t60, i64 16, !dbg !320
  store i64 26, ptr %t62, align 8, !dbg !320
  call void @ignite_rt_fail_match(ptr %t60, i64 184, i64 3), !dbg !320
  unreachable, !dbg !320
L42:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t41, i64 40, i1 false), !dbg !309
  ret void, !dbg !309
}

define internal void @"copy.IoError"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 32, i1 false)
  %t1 = getelementptr inbounds i8, ptr %dst, i64 0
  %t2 = getelementptr inbounds i8, ptr %src, i64 0
  call void @"copy.Text"(ptr %t1, ptr %t2)
  ret void
}

define internal void @"copy.List<Byte>"(ptr %dst, ptr %src) {
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

define internal void @"copy.(Int,Text)"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 32, i1 false)
  %t1 = getelementptr inbounds i8, ptr %dst, i64 8
  %t2 = getelementptr inbounds i8, ptr %src, i64 8
  call void @"copy.Text"(ptr %t1, ptr %t2)
  ret void
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
!llvm.module.flags = !{!328, !329}
!0 = !DIFile(filename: "tests.syntax.ll", directory: ".")
!1 = distinct !DICompileUnit(language: DW_LANG_C, file: !0, producer: "ignite", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)
!2 = !DISubroutineType(types: !{})
!3 = !DIFile(filename: "syntax.ignite", directory: "./tests/golden")
!4 = distinct !DISubprogram(name: "tests.syntax.Reply.action", linkageName: "tests.syntax.Reply.action", scope: !3, file: !3, line: 11, type: !2, scopeLine: 11, spFlags: DISPFlagDefinition, unit: !1)
!5 = !DILocation(line: 11, column: 20, scope: !4)
!6 = distinct !DISubprogram(name: "tests.syntax.Reply.code", linkageName: "tests.syntax.Reply.code", scope: !3, file: !3, line: 12, type: !2, scopeLine: 12, spFlags: DISPFlagDefinition, unit: !1)
!7 = !DILocation(line: 12, column: 15, scope: !6)
!8 = !DILocation(line: 12, column: 22, scope: !6)
!9 = distinct !DISubprogram(name: "tests.syntax.BASE", linkageName: "tests.syntax.BASE", scope: !3, file: !3, line: 23, type: !2, scopeLine: 23, spFlags: DISPFlagDefinition, unit: !1)
!10 = !DILocation(line: 23, column: 19, scope: !9)
!11 = distinct !DISubprogram(name: "tests.syntax.QUIT", linkageName: "tests.syntax.QUIT", scope: !3, file: !3, line: 24, type: !2, scopeLine: 24, spFlags: DISPFlagDefinition, unit: !1)
!12 = !DILocation(line: 24, column: 20, scope: !11)
!13 = distinct !DISubprogram(name: "tests.syntax.SPACE", linkageName: "tests.syntax.SPACE", scope: !3, file: !3, line: 25, type: !2, scopeLine: 25, spFlags: DISPFlagDefinition, unit: !1)
!14 = !DILocation(line: 25, column: 21, scope: !13)
!15 = distinct !DISubprogram(name: "tests.syntax.lowest", linkageName: "tests.syntax.lowest", scope: !3, file: !3, line: 26, type: !2, scopeLine: 26, spFlags: DISPFlagDefinition, unit: !1)
!16 = !DILocation(line: 26, column: 21, scope: !15)
!17 = distinct !DISubprogram(name: "tests.syntax.shout", linkageName: "tests.syntax.shout", scope: !3, file: !3, line: 28, type: !2, scopeLine: 28, spFlags: DISPFlagDefinition, unit: !1)
!18 = !DILocation(line: 28, column: 29, scope: !17)
!19 = distinct !DISubprogram(name: "tests.syntax.twice", linkageName: "tests.syntax.twice", scope: !3, file: !3, line: 30, type: !2, scopeLine: 30, spFlags: DISPFlagDefinition, unit: !1)
!20 = !DILocation(line: 30, column: 27, scope: !19)
!21 = !DILocation(line: 30, column: 31, scope: !19)
!22 = distinct !DISubprogram(name: "tests.syntax.lower_case", linkageName: "tests.syntax.lower_case", scope: !3, file: !3, line: 32, type: !2, scopeLine: 32, spFlags: DISPFlagDefinition, unit: !1)
!23 = !DILocation(line: 32, column: 31, scope: !22)
!24 = !DILocation(line: 33, column: 3, scope: !22)
!25 = !DILocation(line: 33, column: 9, scope: !22)
!26 = !DILocation(line: 34, column: 15, scope: !22)
!27 = !DILocation(line: 35, column: 14, scope: !22)
!28 = !DILocation(line: 35, column: 22, scope: !22)
!29 = distinct !DISubprogram(name: "tests.syntax.swap", linkageName: "tests.syntax.swap", scope: !3, file: !3, line: 39, type: !2, scopeLine: 39, spFlags: DISPFlagDefinition, unit: !1)
!30 = !DILocation(line: 39, column: 40, scope: !29)
!31 = !DILocation(line: 40, column: 3, scope: !29)
!32 = !DILocation(line: 40, column: 16, scope: !29)
!33 = !DILocation(line: 41, column: 3, scope: !29)
!34 = !DILocation(line: 41, column: 4, scope: !29)
!35 = !DILocation(line: 41, column: 7, scope: !29)
!36 = distinct !DISubprogram(name: "tests.syntax.describe", linkageName: "tests.syntax.describe", scope: !3, file: !3, line: 44, type: !2, scopeLine: 44, spFlags: DISPFlagDefinition, unit: !1)
!37 = !DILocation(line: 44, column: 29, scope: !36)
!38 = !DILocation(line: 45, column: 3, scope: !36)
!39 = !DILocation(line: 45, column: 9, scope: !36)
!40 = !DILocation(line: 46, column: 14, scope: !36)
!41 = !DILocation(line: 47, column: 13, scope: !36)
!42 = !DILocation(line: 48, column: 10, scope: !36)
!43 = distinct !DISubprogram(name: "tests.syntax.classify", linkageName: "tests.syntax.classify", scope: !3, file: !3, line: 52, type: !2, scopeLine: 52, spFlags: DISPFlagDefinition, unit: !1)
!44 = !DILocation(line: 52, column: 40, scope: !43)
!45 = !DILocation(line: 53, column: 3, scope: !43)
!46 = !DILocation(line: 53, column: 9, scope: !43)
!47 = !DILocation(line: 54, column: 11, scope: !43)
!48 = !DILocation(line: 55, column: 12, scope: !43)
!49 = !DILocation(line: 55, column: 21, scope: !43)
!50 = !DILocation(line: 56, column: 18, scope: !43)
!51 = !DILocation(line: 57, column: 15, scope: !43)
!52 = !DILocation(line: 57, column: 24, scope: !43)
!53 = !DILocation(line: 57, column: 28, scope: !43)
!54 = !DILocation(line: 58, column: 10, scope: !43)
!55 = distinct !DISubprogram(name: "tests.syntax.nested", linkageName: "tests.syntax.nested", scope: !3, file: !3, line: 62, type: !2, scopeLine: 62, spFlags: DISPFlagDefinition, unit: !1)
!56 = !DILocation(line: 62, column: 49, scope: !55)
!57 = !DILocation(line: 63, column: 3, scope: !55)
!58 = !DILocation(line: 63, column: 9, scope: !55)
!59 = !DILocation(line: 64, column: 29, scope: !55)
!60 = !DILocation(line: 64, column: 36, scope: !55)
!61 = !DILocation(line: 64, column: 40, scope: !55)
!62 = !DILocation(line: 65, column: 32, scope: !55)
!63 = !DILocation(line: 66, column: 24, scope: !55)
!64 = distinct !DISubprogram(name: "tests.syntax.vowel", linkageName: "tests.syntax.vowel", scope: !3, file: !3, line: 70, type: !2, scopeLine: 70, spFlags: DISPFlagDefinition, unit: !1)
!65 = !DILocation(line: 70, column: 27, scope: !64)
!66 = !DILocation(line: 71, column: 3, scope: !64)
!67 = !DILocation(line: 71, column: 9, scope: !64)
!68 = !DILocation(line: 72, column: 36, scope: !64)
!69 = !DILocation(line: 73, column: 10, scope: !64)
!70 = distinct !DISubprogram(name: "tests.syntax.first_positive", linkageName: "tests.syntax.first_positive", scope: !3, file: !3, line: 77, type: !2, scopeLine: 77, spFlags: DISPFlagDefinition, unit: !1)
!71 = !DILocation(line: 77, column: 45, scope: !70)
!72 = !DILocation(line: 78, column: 3, scope: !70)
!73 = !DILocation(line: 78, column: 12, scope: !70)
!74 = !DILocation(line: 79, column: 5, scope: !70)
!75 = !DILocation(line: 79, column: 8, scope: !70)
!76 = !DILocation(line: 79, column: 12, scope: !70)
!77 = !DILocation(line: 80, column: 7, scope: !70)
!78 = !DILocation(line: 80, column: 14, scope: !70)
!79 = !DILocation(line: 83, column: 3, scope: !70)
!80 = distinct !DISubprogram(name: "tests.syntax.sign", linkageName: "tests.syntax.sign", scope: !3, file: !3, line: 86, type: !2, scopeLine: 86, spFlags: DISPFlagDefinition, unit: !1)
!81 = !DILocation(line: 86, column: 25, scope: !80)
!82 = !DILocation(line: 87, column: 3, scope: !80)
!83 = !DILocation(line: 87, column: 11, scope: !80)
!84 = !DILocation(line: 87, column: 17, scope: !80)
!85 = !DILocation(line: 88, column: 10, scope: !80)
!86 = !DILocation(line: 88, column: 17, scope: !80)
!87 = !DILocation(line: 89, column: 10, scope: !80)
!88 = !DILocation(line: 89, column: 13, scope: !80)
!89 = !DILocation(line: 89, column: 17, scope: !80)
!90 = !DILocation(line: 89, column: 21, scope: !80)
!91 = !DILocation(line: 89, column: 32, scope: !80)
!92 = !DILocation(line: 89, column: 34, scope: !80)
!93 = !DILocation(line: 91, column: 3, scope: !80)
!94 = distinct !DISubprogram(name: "tests.syntax.count_spaces", linkageName: "tests.syntax.count_spaces", scope: !3, file: !3, line: 94, type: !2, scopeLine: 94, spFlags: DISPFlagDefinition, unit: !1)
!95 = !DILocation(line: 94, column: 38, scope: !94)
!96 = !DILocation(line: 95, column: 3, scope: !94)
!97 = !DILocation(line: 95, column: 11, scope: !94)
!98 = !DILocation(line: 96, column: 3, scope: !94)
!99 = !DILocation(line: 96, column: 12, scope: !94)
!100 = !DILocation(line: 97, column: 5, scope: !94)
!101 = !DILocation(line: 97, column: 11, scope: !94)
!102 = !DILocation(line: 98, column: 16, scope: !94)
!103 = !DILocation(line: 99, column: 9, scope: !94)
!104 = !DILocation(line: 99, column: 13, scope: !94)
!105 = !DILocation(line: 99, column: 17, scope: !94)
!106 = !DILocation(line: 101, column: 12, scope: !94)
!107 = !DILocation(line: 104, column: 3, scope: !94)
!108 = distinct !DISubprogram(name: "tests.syntax.show", linkageName: "tests.syntax.show", scope: !3, file: !3, line: 107, type: !2, scopeLine: 107, spFlags: DISPFlagDefinition, unit: !1)
!109 = !DILocation(line: 107, column: 39, scope: !108)
!110 = !DILocation(line: 108, column: 3, scope: !108)
!111 = !DILocation(line: 108, column: 15, scope: !108)
!112 = !DILocation(line: 109, column: 3, scope: !108)
!113 = !DILocation(line: 109, column: 14, scope: !108)
!114 = !DILocation(line: 110, column: 3, scope: !108)
!115 = !DILocation(line: 110, column: 20, scope: !108)
!116 = distinct !DISubprogram(name: "tests.syntax.Syntax.connect", linkageName: "tests.syntax.Syntax.connect", scope: !3, file: !3, line: 114, type: !2, scopeLine: 114, spFlags: DISPFlagDefinition, unit: !1)
!117 = !DILocation(line: 114, column: 82, scope: !116)
!118 = !DILocation(line: 115, column: 5, scope: !116)
!119 = !DILocation(line: 115, column: 26, scope: !116)
!120 = !DILocation(line: 115, column: 31, scope: !116)
!121 = !DILocation(line: 115, column: 32, scope: !116)
!122 = !DILocation(line: 115, column: 35, scope: !116)
!123 = !DILocation(line: 116, column: 5, scope: !116)
!124 = !DILocation(line: 116, column: 10, scope: !116)
!125 = !DILocation(line: 116, column: 12, scope: !116)
!126 = !DILocation(line: 116, column: 19, scope: !116)
!127 = !DILocation(line: 117, column: 5, scope: !116)
!128 = !DILocation(line: 117, column: 10, scope: !116)
!129 = !DILocation(line: 117, column: 19, scope: !116)
!130 = !DILocation(line: 117, column: 30, scope: !116)
!131 = !DILocation(line: 117, column: 39, scope: !116)
!132 = !DILocation(line: 117, column: 51, scope: !116)
!133 = !DILocation(line: 117, column: 60, scope: !116)
!134 = !DILocation(line: 118, column: 5, scope: !116)
!135 = !DILocation(line: 118, column: 10, scope: !116)
!136 = !DILocation(line: 118, column: 19, scope: !116)
!137 = !DILocation(line: 119, column: 5, scope: !116)
!138 = !DILocation(line: 119, column: 10, scope: !116)
!139 = !DILocation(line: 119, column: 19, scope: !116)
!140 = !DILocation(line: 119, column: 20, scope: !116)
!141 = !DILocation(line: 120, column: 5, scope: !116)
!142 = !DILocation(line: 120, column: 10, scope: !116)
!143 = !DILocation(line: 120, column: 19, scope: !116)
!144 = !DILocation(line: 120, column: 20, scope: !116)
!145 = !DILocation(line: 120, column: 28, scope: !116)
!146 = !DILocation(line: 121, column: 5, scope: !116)
!147 = !DILocation(line: 121, column: 10, scope: !116)
!148 = !DILocation(line: 121, column: 19, scope: !116)
!149 = !DILocation(line: 121, column: 20, scope: !116)
!150 = !DILocation(line: 121, column: 25, scope: !116)
!151 = !DILocation(line: 122, column: 5, scope: !116)
!152 = !DILocation(line: 122, column: 10, scope: !116)
!153 = !DILocation(line: 122, column: 19, scope: !116)
!154 = !DILocation(line: 122, column: 20, scope: !116)
!155 = !DILocation(line: 122, column: 25, scope: !116)
!156 = !DILocation(line: 122, column: 30, scope: !116)
!157 = !DILocation(line: 123, column: 5, scope: !116)
!158 = !DILocation(line: 123, column: 10, scope: !116)
!159 = !DILocation(line: 123, column: 17, scope: !116)
!160 = !DILocation(line: 123, column: 22, scope: !116)
!161 = !DILocation(line: 123, column: 23, scope: !116)
!162 = !DILocation(line: 123, column: 29, scope: !116)
!163 = !DILocation(line: 123, column: 30, scope: !116)
!164 = !DILocation(line: 123, column: 33, scope: !116)
!165 = !DILocation(line: 124, column: 5, scope: !116)
!166 = !DILocation(line: 124, column: 10, scope: !116)
!167 = !DILocation(line: 124, column: 17, scope: !116)
!168 = !DILocation(line: 124, column: 22, scope: !116)
!169 = !DILocation(line: 124, column: 23, scope: !116)
!170 = !DILocation(line: 124, column: 30, scope: !116)
!171 = !DILocation(line: 124, column: 31, scope: !116)
!172 = !DILocation(line: 125, column: 5, scope: !116)
!173 = !DILocation(line: 125, column: 10, scope: !116)
!174 = !DILocation(line: 125, column: 17, scope: !116)
!175 = !DILocation(line: 126, column: 5, scope: !116)
!176 = !DILocation(line: 126, column: 10, scope: !116)
!177 = !DILocation(line: 126, column: 17, scope: !116)
!178 = !DILocation(line: 126, column: 22, scope: !116)
!179 = !DILocation(line: 126, column: 23, scope: !116)
!180 = !DILocation(line: 126, column: 29, scope: !116)
!181 = !DILocation(line: 126, column: 30, scope: !116)
!182 = !DILocation(line: 127, column: 5, scope: !116)
!183 = !DILocation(line: 127, column: 13, scope: !116)
!184 = !DILocation(line: 127, column: 19, scope: !116)
!185 = !DILocation(line: 128, column: 5, scope: !116)
!186 = !DILocation(line: 128, column: 13, scope: !116)
!187 = !DILocation(line: 128, column: 19, scope: !116)
!188 = !DILocation(line: 129, column: 5, scope: !116)
!189 = !DILocation(line: 129, column: 10, scope: !116)
!190 = !DILocation(line: 129, column: 12, scope: !116)
!191 = !DILocation(line: 129, column: 16, scope: !116)
!192 = !DILocation(line: 130, column: 5, scope: !116)
!193 = !DILocation(line: 130, column: 10, scope: !116)
!194 = !DILocation(line: 130, column: 25, scope: !116)
!195 = !DILocation(line: 130, column: 26, scope: !116)
!196 = !DILocation(line: 130, column: 27, scope: !116)
!197 = !DILocation(line: 130, column: 30, scope: !116)
!198 = !DILocation(line: 130, column: 33, scope: !116)
!199 = !DILocation(line: 130, column: 36, scope: !116)
!200 = !DILocation(line: 131, column: 5, scope: !116)
!201 = !DILocation(line: 131, column: 10, scope: !116)
!202 = !DILocation(line: 131, column: 15, scope: !116)
!203 = !DILocation(line: 131, column: 20, scope: !116)
!204 = !DILocation(line: 131, column: 25, scope: !116)
!205 = !DILocation(line: 131, column: 30, scope: !116)
!206 = !DILocation(line: 131, column: 35, scope: !116)
!207 = !DILocation(line: 131, column: 36, scope: !116)
!208 = !DILocation(line: 132, column: 5, scope: !116)
!209 = !DILocation(line: 132, column: 10, scope: !116)
!210 = !DILocation(line: 132, column: 23, scope: !116)
!211 = !DILocation(line: 133, column: 5, scope: !116)
!212 = !DILocation(line: 133, column: 13, scope: !116)
!213 = !DILocation(line: 134, column: 5, scope: !116)
!214 = !DILocation(line: 134, column: 10, scope: !116)
!215 = !DILocation(line: 135, column: 5, scope: !116)
!216 = !DILocation(line: 135, column: 10, scope: !116)
!217 = !DILocation(line: 136, column: 5, scope: !116)
!218 = !DILocation(line: 136, column: 13, scope: !116)
!219 = !DILocation(line: 136, column: 30, scope: !116)
!220 = !DILocation(line: 136, column: 45, scope: !116)
!221 = !DILocation(line: 137, column: 5, scope: !116)
!222 = !DILocation(line: 137, column: 10, scope: !116)
!223 = !DILocation(line: 137, column: 20, scope: !116)
!224 = !DILocation(line: 138, column: 5, scope: !116)
!225 = !DILocation(line: 138, column: 13, scope: !116)
!226 = !DILocation(line: 138, column: 29, scope: !116)
!227 = !DILocation(line: 138, column: 30, scope: !116)
!228 = !DILocation(line: 139, column: 5, scope: !116)
!229 = !DILocation(line: 139, column: 10, scope: !116)
!230 = !DILocation(line: 139, column: 17, scope: !116)
!231 = !DILocation(line: 140, column: 5, scope: !116)
!232 = !DILocation(line: 140, column: 11, scope: !116)
!233 = !DILocation(line: 141, column: 19, scope: !116)
!234 = !DILocation(line: 141, column: 24, scope: !116)
!235 = !DILocation(line: 142, column: 16, scope: !116)
!236 = !DILocation(line: 142, column: 21, scope: !116)
!237 = !DILocation(line: 144, column: 5, scope: !116)
!238 = !DILocation(line: 144, column: 16, scope: !116)
!239 = !DILocation(line: 145, column: 5, scope: !116)
!240 = !DILocation(line: 145, column: 17, scope: !116)
!241 = !DILocation(line: 146, column: 5, scope: !116)
!242 = !DILocation(line: 146, column: 10, scope: !116)
!243 = !DILocation(line: 146, column: 33, scope: !116)
!244 = !DILocation(line: 146, column: 57, scope: !116)
!245 = !DILocation(line: 147, column: 5, scope: !116)
!246 = !DILocation(line: 147, column: 16, scope: !116)
!247 = !DILocation(line: 147, column: 17, scope: !116)
!248 = !DILocation(line: 147, column: 20, scope: !116)
!249 = !DILocation(line: 148, column: 5, scope: !116)
!250 = !DILocation(line: 148, column: 13, scope: !116)
!251 = !DILocation(line: 149, column: 5, scope: !116)
!252 = !DILocation(line: 149, column: 11, scope: !116)
!253 = !DILocation(line: 149, column: 15, scope: !116)
!254 = !DILocation(line: 150, column: 7, scope: !116)
!255 = !DILocation(line: 150, column: 27, scope: !116)
!256 = !DILocation(line: 151, column: 7, scope: !116)
!257 = !DILocation(line: 151, column: 14, scope: !116)
!258 = !DILocation(line: 151, column: 15, scope: !116)
!259 = !DILocation(line: 151, column: 23, scope: !116)
!260 = !DILocation(line: 151, column: 26, scope: !116)
!261 = !DILocation(line: 152, column: 7, scope: !116)
!262 = !DILocation(line: 152, column: 11, scope: !116)
!263 = !DILocation(line: 152, column: 15, scope: !116)
!264 = !DILocation(line: 154, column: 5, scope: !116)
!265 = !DILocation(line: 154, column: 25, scope: !116)
!266 = !DILocation(line: 155, column: 5, scope: !116)
!267 = !DILocation(line: 155, column: 16, scope: !116)
!268 = !DILocation(line: 156, column: 5, scope: !116)
!269 = !DILocation(line: 156, column: 10, scope: !116)
!270 = !DILocation(line: 156, column: 12, scope: !116)
!271 = !DILocation(line: 156, column: 20, scope: !116)
!272 = !DILocation(line: 157, column: 5, scope: !116)
!273 = !DILocation(line: 157, column: 17, scope: !116)
!274 = !DILocation(line: 157, column: 18, scope: !116)
!275 = !DILocation(line: 157, column: 21, scope: !116)
!276 = !DILocation(line: 158, column: 5, scope: !116)
!277 = !DILocation(line: 158, column: 18, scope: !116)
!278 = !DILocation(line: 159, column: 5, scope: !116)
!279 = !DILocation(line: 159, column: 9, scope: !116)
!280 = !DILocation(line: 159, column: 13, scope: !116)
!281 = !DILocation(line: 160, column: 5, scope: !116)
!282 = !DILocation(line: 160, column: 9, scope: !116)
!283 = !DILocation(line: 161, column: 5, scope: !116)
!284 = !DILocation(line: 161, column: 10, scope: !116)
!285 = !DILocation(line: 161, column: 12, scope: !116)
!286 = !DILocation(line: 161, column: 15, scope: !116)
!287 = !DILocation(line: 162, column: 5, scope: !116)
!288 = !DILocation(line: 162, column: 13, scope: !116)
!289 = !DILocation(line: 162, column: 19, scope: !116)
!290 = !DILocation(line: 163, column: 5, scope: !116)
!291 = !DILocation(line: 163, column: 9, scope: !116)
!292 = !DILocation(line: 163, column: 15, scope: !116)
!293 = !DILocation(line: 164, column: 5, scope: !116)
!294 = !DILocation(line: 164, column: 24, scope: !116)
!295 = !DILocation(line: 165, column: 5, scope: !116)
!296 = !DILocation(line: 165, column: 11, scope: !116)
!297 = !DILocation(line: 166, column: 14, scope: !116)
!298 = !DILocation(line: 166, column: 19, scope: !116)
!299 = !DILocation(line: 167, column: 13, scope: !116)
!300 = !DILocation(line: 167, column: 18, scope: !116)
!301 = !DILocation(line: 169, column: 5, scope: !116)
!302 = !DILocation(line: 169, column: 10, scope: !116)
!303 = !DILocation(line: 169, column: 21, scope: !116)
!304 = !DILocation(line: 169, column: 32, scope: !116)
!305 = !DILocation(line: 169, column: 43, scope: !116)
!306 = !DILocation(line: 170, column: 5, scope: !116)
!307 = !DILocation(line: 170, column: 8, scope: !116)
!308 = distinct !DISubprogram(name: "tests.syntax.Runner.main", linkageName: "tests.syntax.Runner.main", scope: !3, file: !3, line: 179, type: !2, scopeLine: 179, spFlags: DISPFlagDefinition, unit: !1)
!309 = !DILocation(line: 179, column: 55, scope: !308)
!310 = !DILocation(line: 180, column: 3, scope: !308)
!311 = !DILocation(line: 180, column: 16, scope: !308)
!312 = !DILocation(line: 180, column: 22, scope: !308)
!313 = !DILocation(line: 180, column: 40, scope: !308)
!314 = !DILocation(line: 180, column: 54, scope: !308)
!315 = !DILocation(line: 181, column: 19, scope: !308)
!316 = !DILocation(line: 182, column: 19, scope: !308)
!317 = !DILocation(line: 182, column: 26, scope: !308)
!318 = !DILocation(line: 182, column: 30, scope: !308)
!319 = !DILocation(line: 182, column: 51, scope: !308)
!320 = !DILocation(line: 184, column: 3, scope: !308)
!321 = !DILocation(line: 184, column: 9, scope: !308)
!322 = !DILocation(line: 184, column: 24, scope: !308)
!323 = !DILocation(line: 185, column: 14, scope: !308)
!324 = !DILocation(line: 185, column: 17, scope: !308)
!325 = !DILocation(line: 186, column: 19, scope: !308)
!326 = !DILocation(line: 186, column: 23, scope: !308)
!327 = !DILocation(line: 186, column: 44, scope: !308)
!328 = !{i32 7, !"Dwarf Version", i32 5}
!329 = !{i32 2, !"Debug Info Version", i32 3}
