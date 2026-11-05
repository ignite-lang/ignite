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
  %t4 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  store ptr @.str.3, ptr %t1, align 8, !dbg !19
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !19
  store i64 1, ptr %t2, align 8, !dbg !19
  %t3 = getelementptr inbounds i8, ptr %t1, i64 16, !dbg !19
  store i64 1, ptr %t3, align 8, !dbg !19
  call void @"copy.Text"(ptr %t4, ptr %t1), !dbg !19
  call void @ignite_rt_text_concat(ptr %t5, ptr %p0, ptr %t4), !dbg !18
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t5, i64 24, i1 false), !dbg !18
  ret void, !dbg !18
}

define i64 @"tests.syntax.twice"(i64 %p0) !dbg !20 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !21
  %t2 = load i64, ptr %t1, align 8, !dbg !21
  %t3 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %t2, i64 2), !dbg !21
  %t4 = extractvalue { i64, i1 } %t3, 1, !dbg !21
  br i1 %t4, label %L5, label %L6, !dbg !21
L5:
  store ptr @.str.0, ptr %t7, align 8, !dbg !21
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !21
  store i64 26, ptr %t8, align 8, !dbg !21
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !21
  store i64 26, ptr %t9, align 8, !dbg !21
  store ptr @.str.4, ptr %t10, align 8, !dbg !21
  %t11 = getelementptr inbounds i8, ptr %t10, i64 8, !dbg !21
  store i64 1, ptr %t11, align 8, !dbg !21
  %t12 = getelementptr inbounds i8, ptr %t10, i64 16, !dbg !21
  store i64 1, ptr %t12, align 8, !dbg !21
  call void @ignite_rt_fail_arith(ptr %t10, ptr %t7, i64 30, i64 27, i64 %t2, i64 2), !dbg !21
  unreachable, !dbg !21
L6:
  %t13 = extractvalue { i64, i1 } %t3, 0, !dbg !21
  ret i64 %t13, !dbg !21
}

define void @"tests.syntax.lower_case"(ptr %sret, i64 %p0) !dbg !23 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t3 = alloca [24 x i8], align 8
  %t9 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t14 = alloca [8 x i8], align 8
  %t15 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t21 = alloca [24 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !24
  %t2 = load i64, ptr %t1, align 8, !dbg !26
  %t6 = call i64 @"tests.syntax.lowest"(), !dbg !25
  %t7 = icmp eq i64 %t2, %t6, !dbg !25
  br i1 %t7, label %L8, label %L5, !dbg !25
L8:
  store ptr @.str.5, ptr %t9, align 8, !dbg !27
  %t10 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !27
  store i64 6, ptr %t10, align 8, !dbg !27
  %t11 = getelementptr inbounds i8, ptr %t9, i64 16, !dbg !27
  store i64 6, ptr %t11, align 8, !dbg !27
  call void @"copy.Text"(ptr %t12, ptr %t9), !dbg !27
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t12, i64 24, i1 false), !dbg !25
  br label %L4, !dbg !25
L5:
  store i64 %t2, ptr %t14, align 8, !dbg !25
  store ptr @.str.6, ptr %t15, align 8, !dbg !28
  %t16 = getelementptr inbounds i8, ptr %t15, i64 8, !dbg !28
  store i64 6, ptr %t16, align 8, !dbg !28
  %t17 = getelementptr inbounds i8, ptr %t15, i64 16, !dbg !28
  store i64 6, ptr %t17, align 8, !dbg !28
  call void @"copy.Text"(ptr %t18, ptr %t15), !dbg !28
  %t19 = load i64, ptr %t14, align 8, !dbg !29
  call void @ignite_rt_int_to_text(ptr %t20, i64 %t19), !dbg !29
  call void @ignite_rt_text_concat(ptr %t21, ptr %t18, ptr %t20), !dbg !28
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t21, i64 24, i1 false), !dbg !25
  br label %L4, !dbg !25
L13:
  store ptr @.str.0, ptr %t20, align 8, !dbg !25
  %t22 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !25
  store i64 26, ptr %t22, align 8, !dbg !25
  %t23 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !25
  store i64 26, ptr %t23, align 8, !dbg !25
  call void @ignite_rt_fail_match(ptr %t20, i64 33, i64 3), !dbg !25
  unreachable, !dbg !25
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t3, i64 24, i1 false), !dbg !24
  ret void, !dbg !24
}

define void @"tests.syntax.swap"(ptr %sret, ptr %p0) !dbg !30 {
entry:
  %t1 = alloca [32 x i8], align 8
  %t5 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [32 x i8], align 8
  call void @"copy.(Int,Text)"(ptr %t1, ptr %p0), !dbg !32
  %t4 = load i64, ptr %t1, align 8, !dbg !32
  store i64 %t4, ptr %t5, align 8, !dbg !32
  %t6 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !32
  br label %L2, !dbg !32
L3:
  store ptr @.str.0, ptr %t7, align 8, !dbg !32
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !32
  store i64 26, ptr %t8, align 8, !dbg !32
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16, !dbg !32
  store i64 26, ptr %t9, align 8, !dbg !32
  call void @ignite_rt_fail_match(ptr %t7, i64 40, i64 7), !dbg !32
  unreachable, !dbg !32
L2:
  call void @"copy.Text"(ptr %t10, ptr %t6), !dbg !34
  %t11 = load i64, ptr %t5, align 8, !dbg !36
  %t12 = getelementptr inbounds i8, ptr %t10, i64 24, !dbg !34
  store i64 %t11, ptr %t12, align 8, !dbg !34
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t10, i64 32, i1 false), !dbg !31
  ret void, !dbg !31
}

define void @"tests.syntax.describe"(ptr %sret, i64 %p0) !dbg !37 {
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
  store i64 %p0, ptr %t1, align 8, !dbg !38
  %t2 = load i64, ptr %t1, align 8, !dbg !40
  %t8 = icmp eq i64 %t2, 0, !dbg !39
  br i1 %t8, label %L9, label %L7, !dbg !39
L9:
  br label %L6, !dbg !39
L7:
  %t10 = icmp eq i64 %t2, 1, !dbg !39
  br i1 %t10, label %L11, label %L5, !dbg !39
L11:
  br label %L6, !dbg !39
L6:
  store ptr @.str.7, ptr %t12, align 8, !dbg !41
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !41
  store i64 5, ptr %t13, align 8, !dbg !41
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !41
  store i64 5, ptr %t14, align 8, !dbg !41
  call void @"copy.Text"(ptr %t15, ptr %t12), !dbg !41
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t15, i64 24, i1 false), !dbg !39
  br label %L4, !dbg !39
L5:
  %t17 = call i64 @"tests.syntax.BASE"(), !dbg !39
  %t18 = icmp eq i64 %t2, %t17, !dbg !39
  br i1 %t18, label %L19, label %L16, !dbg !39
L19:
  store ptr @.str.8, ptr %t20, align 8, !dbg !42
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !42
  store i64 4, ptr %t21, align 8, !dbg !42
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !42
  store i64 4, ptr %t22, align 8, !dbg !42
  call void @"copy.Text"(ptr %t23, ptr %t20), !dbg !42
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t23, i64 24, i1 false), !dbg !39
  br label %L4, !dbg !39
L16:
  store ptr @.str.9, ptr %t25, align 8, !dbg !43
  %t26 = getelementptr inbounds i8, ptr %t25, i64 8, !dbg !43
  store i64 5, ptr %t26, align 8, !dbg !43
  %t27 = getelementptr inbounds i8, ptr %t25, i64 16, !dbg !43
  store i64 5, ptr %t27, align 8, !dbg !43
  call void @"copy.Text"(ptr %t28, ptr %t25), !dbg !43
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t28, i64 24, i1 false), !dbg !39
  br label %L4, !dbg !39
L24:
  store ptr @.str.0, ptr %t29, align 8, !dbg !39
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !39
  store i64 26, ptr %t30, align 8, !dbg !39
  %t31 = getelementptr inbounds i8, ptr %t29, i64 16, !dbg !39
  store i64 26, ptr %t31, align 8, !dbg !39
  call void @ignite_rt_fail_match(ptr %t29, i64 45, i64 3), !dbg !39
  unreachable, !dbg !39
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t3, i64 24, i1 false), !dbg !38
  ret void, !dbg !38
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

define internal void @ignite.append(ptr %list, ptr %bytes, i64 %len) noinline {
entry:
  call void @ignite.reserve(ptr %list, i64 %len, i64 1, i64 1)
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %old = load i64, ptr %lenp, align 8
  %data = load ptr, ptr %list, align 8
  %dst = getelementptr inbounds i8, ptr %data, i64 %old
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %bytes, i64 %len, i1 false)
  %new = add i64 %old, %len
  store i64 %new, ptr %lenp, align 8
  ret void
}

define internal void @ignite.append_text(ptr %list, ptr %text) noinline {
entry:
  %data = load ptr, ptr %text, align 8
  %lenp = getelementptr inbounds i8, ptr %text, i64 8
  %len = load i64, ptr %lenp, align 8
  call void @ignite.append(ptr %list, ptr %data, i64 %len)
  ret void
}

define void @"tests.syntax.classify"(ptr %sret, ptr %p0) !dbg !44 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t11 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t21 = alloca [24 x i8], align 8
  %t22 = alloca [24 x i8], align 8
  %t34 = alloca [24 x i8], align 8
  %t42 = alloca [24 x i8], align 8
  %t45 = alloca [24 x i8], align 8
  %t46 = alloca [24 x i8], align 8
  %t50 = alloca [24 x i8], align 8
  %t4 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !46
  %t5 = load i64, ptr %t4, align 8, !dbg !46
  %t6 = icmp eq i64 %t5, 0, !dbg !46
  br i1 %t6, label %L7, label %L3, !dbg !46
L7:
  store ptr @.str.10, ptr %t8, align 8, !dbg !48
  %t9 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !48
  store i64 4, ptr %t9, align 8, !dbg !48
  %t10 = getelementptr inbounds i8, ptr %t8, i64 16, !dbg !48
  store i64 4, ptr %t10, align 8, !dbg !48
  call void @"copy.Text"(ptr %t11, ptr %t8), !dbg !48
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t11, i64 24, i1 false), !dbg !46
  br label %L2, !dbg !46
L3:
  %t13 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !46
  %t14 = load i64, ptr %t13, align 8, !dbg !46
  %t15 = icmp eq i64 %t14, 1, !dbg !46
  br i1 %t15, label %L16, label %L12, !dbg !46
L16:
  %t17 = load ptr, ptr %p0, align 8, !dbg !46
  store ptr @.str.11, ptr %t18, align 8, !dbg !49
  %t19 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !49
  store i64 4, ptr %t19, align 8, !dbg !49
  %t20 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !49
  store i64 4, ptr %t20, align 8, !dbg !49
  call void @"copy.Text"(ptr %t21, ptr %t18), !dbg !49
  call void @ignite_rt_text_concat(ptr %t22, ptr %t21, ptr %t17), !dbg !49
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t22, i64 24, i1 false), !dbg !46
  br label %L2, !dbg !46
L12:
  %t24 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !46
  %t25 = load i64, ptr %t24, align 8, !dbg !46
  %t26 = icmp eq i64 %t25, 2, !dbg !46
  br i1 %t26, label %L27, label %L23, !dbg !46
L27:
  %t28 = load ptr, ptr %p0, align 8, !dbg !46
  call void @"tests.syntax.QUIT"(ptr %t21), !dbg !46
  %t29 = call i1 @ignite.text_eq(ptr %t28, ptr %t21), !dbg !46
  br i1 %t29, label %L30, label %L23, !dbg !46
L30:
  %t31 = getelementptr inbounds i8, ptr %t28, i64 24, !dbg !46
  store ptr @.str.12, ptr %t18, align 8, !dbg !51
  %t32 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !51
  store i64 18, ptr %t32, align 8, !dbg !51
  %t33 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !51
  store i64 18, ptr %t33, align 8, !dbg !51
  call void @"copy.Text"(ptr %t34, ptr %t18), !dbg !51
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t34, i64 24, i1 false), !dbg !46
  br label %L2, !dbg !46
L23:
  %t36 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !46
  %t37 = load i64, ptr %t36, align 8, !dbg !46
  %t38 = icmp eq i64 %t37, 2, !dbg !46
  br i1 %t38, label %L39, label %L35, !dbg !46
L39:
  %t40 = load ptr, ptr %p0, align 8, !dbg !46
  %t41 = getelementptr inbounds i8, ptr %t40, i64 24, !dbg !46
  store ptr @.str.13, ptr %t42, align 8, !dbg !52
  %t43 = getelementptr inbounds i8, ptr %t42, i64 8, !dbg !52
  store i64 4, ptr %t43, align 8, !dbg !52
  %t44 = getelementptr inbounds i8, ptr %t42, i64 16, !dbg !52
  store i64 4, ptr %t44, align 8, !dbg !52
  call void @"copy.Text"(ptr %t45, ptr %t42), !dbg !52
  call void @ignite_rt_text_concat(ptr %t46, ptr %t45, ptr %t40), !dbg !52
  call void @ignite.append_text(ptr %t46, ptr %t41), !dbg !52
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t46, i64 24, i1 false), !dbg !46
  br label %L2, !dbg !46
L35:
  store ptr @.str.14, ptr %t45, align 8, !dbg !55
  %t48 = getelementptr inbounds i8, ptr %t45, i64 8, !dbg !55
  store i64 4, ptr %t48, align 8, !dbg !55
  %t49 = getelementptr inbounds i8, ptr %t45, i64 16, !dbg !55
  store i64 4, ptr %t49, align 8, !dbg !55
  call void @"copy.Text"(ptr %t42, ptr %t45), !dbg !55
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t42, i64 24, i1 false), !dbg !46
  br label %L2, !dbg !46
L47:
  store ptr @.str.0, ptr %t50, align 8, !dbg !46
  %t51 = getelementptr inbounds i8, ptr %t50, i64 8, !dbg !46
  store i64 26, ptr %t51, align 8, !dbg !46
  %t52 = getelementptr inbounds i8, ptr %t50, i64 16, !dbg !46
  store i64 26, ptr %t52, align 8, !dbg !46
  call void @ignite_rt_fail_match(ptr %t50, i64 53, i64 3), !dbg !46
  unreachable, !dbg !46
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !45
  ret void, !dbg !45
}

define void @"tests.syntax.nested"(ptr %sret, ptr %p0) !dbg !56 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t19 = alloca [8 x i8], align 8
  %t22 = alloca [8 x i8], align 8
  %t23 = alloca [24 x i8], align 8
  %t26 = alloca [24 x i8], align 8
  %t28 = alloca [24 x i8], align 8
  %t29 = alloca [24 x i8], align 8
  %t60 = alloca [24 x i8], align 8
  %t61 = alloca [24 x i8], align 8
  %t4 = load i32, ptr %p0, align 4, !dbg !58
  %t5 = icmp eq i32 %t4, 0, !dbg !58
  br i1 %t5, label %L6, label %L3, !dbg !58
L6:
  %t7 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !58
  %t8 = load i8, ptr %t7, align 1, !dbg !58
  %t9 = trunc i8 %t8 to i1, !dbg !58
  %t10 = icmp eq i1 %t9, true, !dbg !58
  br i1 %t10, label %L11, label %L3, !dbg !58
L11:
  %t12 = getelementptr inbounds i8, ptr %t7, i64 8, !dbg !58
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !58
  %t14 = load i64, ptr %t13, align 8, !dbg !58
  %t15 = icmp eq i64 %t14, 2, !dbg !58
  br i1 %t15, label %L16, label %L3, !dbg !58
L16:
  %t17 = load ptr, ptr %t12, align 8, !dbg !58
  %t18 = load i64, ptr %t17, align 8, !dbg !58
  store i64 %t18, ptr %t19, align 8, !dbg !58
  %t20 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !58
  %t21 = load i64, ptr %t20, align 8, !dbg !58
  store i64 %t21, ptr %t22, align 8, !dbg !58
  store ptr @.str.15, ptr %t23, align 8, !dbg !60
  %t24 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !60
  store i64 5, ptr %t24, align 8, !dbg !60
  %t25 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !60
  store i64 5, ptr %t25, align 8, !dbg !60
  call void @"copy.Text"(ptr %t26, ptr %t23), !dbg !60
  %t27 = load i64, ptr %t19, align 8, !dbg !61
  call void @ignite_rt_int_to_text(ptr %t28, i64 %t27), !dbg !61
  call void @ignite_rt_text_concat(ptr %t29, ptr %t26, ptr %t28), !dbg !60
  call void @ignite.append(ptr %t29, ptr @.str.16, i64 1), !dbg !60
  %t30 = load i64, ptr %t22, align 8, !dbg !62
  call void @ignite_rt_int_to_text(ptr %t28, i64 %t30), !dbg !62
  call void @ignite.append_text(ptr %t29, ptr %t28), !dbg !60
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t29, i64 24, i1 false), !dbg !58
  br label %L2, !dbg !58
L3:
  %t34 = load i32, ptr %p0, align 4, !dbg !58
  %t35 = icmp eq i32 %t34, 0, !dbg !58
  br i1 %t35, label %L36, label %L33, !dbg !58
L36:
  %t37 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !58
  %t38 = load i8, ptr %t37, align 1, !dbg !58
  %t39 = trunc i8 %t38 to i1, !dbg !58
  %t40 = icmp eq i1 %t39, false, !dbg !58
  br i1 %t40, label %L41, label %L33, !dbg !58
L41:
  %t42 = getelementptr inbounds i8, ptr %t37, i64 8, !dbg !58
  br label %L32, !dbg !58
L33:
  %t43 = load i32, ptr %p0, align 4, !dbg !58
  %t44 = icmp eq i32 %t43, 1, !dbg !58
  br i1 %t44, label %L45, label %L31, !dbg !58
L45:
  br label %L32, !dbg !58
L32:
  store ptr @.str.17, ptr %t28, align 8, !dbg !63
  %t46 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !63
  store i64 11, ptr %t46, align 8, !dbg !63
  %t47 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !63
  store i64 11, ptr %t47, align 8, !dbg !63
  call void @"copy.Text"(ptr %t26, ptr %t28), !dbg !63
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t26, i64 24, i1 false), !dbg !58
  br label %L2, !dbg !58
L31:
  %t49 = load i32, ptr %p0, align 4, !dbg !58
  %t50 = icmp eq i32 %t49, 0, !dbg !58
  br i1 %t50, label %L51, label %L48, !dbg !58
L51:
  %t52 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !58
  %t53 = load i8, ptr %t52, align 1, !dbg !58
  %t54 = trunc i8 %t53 to i1, !dbg !58
  %t55 = icmp eq i1 %t54, true, !dbg !58
  br i1 %t55, label %L56, label %L48, !dbg !58
L56:
  %t57 = getelementptr inbounds i8, ptr %t52, i64 8, !dbg !58
  store ptr @.str.9, ptr %t23, align 8, !dbg !64
  %t58 = getelementptr inbounds i8, ptr %t23, i64 8, !dbg !64
  store i64 5, ptr %t58, align 8, !dbg !64
  %t59 = getelementptr inbounds i8, ptr %t23, i64 16, !dbg !64
  store i64 5, ptr %t59, align 8, !dbg !64
  call void @"copy.Text"(ptr %t60, ptr %t23), !dbg !64
  call void @llvm.memcpy.p0.p0.i64(ptr %t1, ptr %t60, i64 24, i1 false), !dbg !58
  br label %L2, !dbg !58
L48:
  store ptr @.str.0, ptr %t61, align 8, !dbg !58
  %t62 = getelementptr inbounds i8, ptr %t61, i64 8, !dbg !58
  store i64 26, ptr %t62, align 8, !dbg !58
  %t63 = getelementptr inbounds i8, ptr %t61, i64 16, !dbg !58
  store i64 26, ptr %t63, align 8, !dbg !58
  call void @ignite_rt_fail_match(ptr %t61, i64 63, i64 3), !dbg !58
  unreachable, !dbg !58
L2:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false), !dbg !57
  ret void, !dbg !57
}

define i1 @"tests.syntax.vowel"(ptr %p0) !dbg !65 {
entry:
  %t1 = alloca [1 x i8], align 1
  %t6 = alloca [24 x i8], align 8
  %t12 = alloca [24 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t24 = alloca [24 x i8], align 8
  %t29 = alloca [24 x i8], align 8
  %t37 = alloca [24 x i8], align 8
  store ptr @.str.18, ptr %t6, align 8, !dbg !67
  %t7 = getelementptr inbounds i8, ptr %t6, i64 8, !dbg !67
  store i64 1, ptr %t7, align 8, !dbg !67
  %t8 = getelementptr inbounds i8, ptr %t6, i64 16, !dbg !67
  store i64 1, ptr %t8, align 8, !dbg !67
  %t9 = call i1 @ignite.text_eq(ptr %p0, ptr %t6), !dbg !67
  br i1 %t9, label %L10, label %L5, !dbg !67
L10:
  br label %L4, !dbg !67
L5:
  store ptr @.str.19, ptr %t12, align 8, !dbg !67
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8, !dbg !67
  store i64 1, ptr %t13, align 8, !dbg !67
  %t14 = getelementptr inbounds i8, ptr %t12, i64 16, !dbg !67
  store i64 1, ptr %t14, align 8, !dbg !67
  %t15 = call i1 @ignite.text_eq(ptr %p0, ptr %t12), !dbg !67
  br i1 %t15, label %L16, label %L11, !dbg !67
L16:
  br label %L4, !dbg !67
L11:
  store ptr @.str.20, ptr %t18, align 8, !dbg !67
  %t19 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !67
  store i64 1, ptr %t19, align 8, !dbg !67
  %t20 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !67
  store i64 1, ptr %t20, align 8, !dbg !67
  %t21 = call i1 @ignite.text_eq(ptr %p0, ptr %t18), !dbg !67
  br i1 %t21, label %L22, label %L17, !dbg !67
L22:
  br label %L4, !dbg !67
L17:
  store ptr @.str.21, ptr %t24, align 8, !dbg !67
  %t25 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !67
  store i64 1, ptr %t25, align 8, !dbg !67
  %t26 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !67
  store i64 1, ptr %t26, align 8, !dbg !67
  %t27 = call i1 @ignite.text_eq(ptr %p0, ptr %t24), !dbg !67
  br i1 %t27, label %L28, label %L23, !dbg !67
L28:
  br label %L4, !dbg !67
L23:
  store ptr @.str.22, ptr %t29, align 8, !dbg !67
  %t30 = getelementptr inbounds i8, ptr %t29, i64 8, !dbg !67
  store i64 1, ptr %t30, align 8, !dbg !67
  %t31 = getelementptr inbounds i8, ptr %t29, i64 16, !dbg !67
  store i64 1, ptr %t31, align 8, !dbg !67
  %t32 = call i1 @ignite.text_eq(ptr %p0, ptr %t29), !dbg !67
  br i1 %t32, label %L33, label %L3, !dbg !67
L33:
  br label %L4, !dbg !67
L4:
  %t34 = zext i1 true to i8, !dbg !67
  store i8 %t34, ptr %t1, align 1, !dbg !67
  br label %L2, !dbg !67
L3:
  %t36 = zext i1 false to i8, !dbg !67
  store i8 %t36, ptr %t1, align 1, !dbg !67
  br label %L2, !dbg !67
L35:
  store ptr @.str.0, ptr %t37, align 8, !dbg !67
  %t38 = getelementptr inbounds i8, ptr %t37, i64 8, !dbg !67
  store i64 26, ptr %t38, align 8, !dbg !67
  %t39 = getelementptr inbounds i8, ptr %t37, i64 16, !dbg !67
  store i64 26, ptr %t39, align 8, !dbg !67
  call void @ignite_rt_fail_match(ptr %t37, i64 71, i64 3), !dbg !67
  unreachable, !dbg !67
L2:
  %t40 = load i8, ptr %t1, align 1, !dbg !67
  %t41 = trunc i8 %t40 to i1, !dbg !67
  ret i1 %t41, !dbg !66
}

define i64 @"tests.syntax.first_positive"(ptr %p0) !dbg !71 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t15 = alloca [8 x i8], align 8
  store i64 0, ptr %t1, align 8, !dbg !73
  %t2 = call i64 @ignite.arena_mark(), !dbg !73
  br label %L3
L3:
  %t7 = load i64, ptr %t1, align 8, !dbg !73
  %t8 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !73
  %t9 = load i64, ptr %t8, align 8, !dbg !73
  %t10 = icmp slt i64 %t7, %t9, !dbg !73
  br i1 %t10, label %L4, label %L6, !dbg !73
L4:
  %t11 = load ptr, ptr %p0, align 8, !dbg !73
  %t12 = mul i64 %t7, 8, !dbg !73
  %t13 = getelementptr inbounds i8, ptr %t11, i64 %t12, !dbg !73
  %t14 = load i64, ptr %t13, align 8, !dbg !73
  store i64 %t14, ptr %t15, align 8, !dbg !73
  %t16 = load i64, ptr %t15, align 8, !dbg !76
  %t17 = icmp sgt i64 %t16, 0, !dbg !76
  br i1 %t17, label %L18, label %L19, !dbg !75
L18:
  %t21 = load i64, ptr %t15, align 8, !dbg !79
  ret i64 %t21, !dbg !78
L22:
  br label %L20, !dbg !75
L19:
  br label %L20, !dbg !75
L20:
  br label %L5
L5:
  %t23 = load i64, ptr %t1, align 8, !dbg !73
  %t24 = add i64 %t23, 1, !dbg !73
  store i64 %t24, ptr %t1, align 8, !dbg !73
  call void @ignite.arena_release(i64 %t2), !dbg !73
  br label %L3, !dbg !73
L6:
  ret i64 0, !dbg !72
}

define void @"tests.syntax.sign"(ptr %sret, i64 %p0) !dbg !81 {
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
  store i64 %p0, ptr %t1, align 8, !dbg !82
  %t2 = load i64, ptr %t1, align 8, !dbg !85
  %t6 = icmp eq i64 %t2, 0, !dbg !84
  br i1 %t6, label %L7, label %L5, !dbg !84
L7:
  store ptr @.str.23, ptr %t8, align 8, !dbg !87
  %t9 = getelementptr inbounds i8, ptr %t8, i64 8, !dbg !87
  store i64 4, ptr %t9, align 8, !dbg !87
  %t10 = getelementptr inbounds i8, ptr %t8, i64 16, !dbg !87
  store i64 4, ptr %t10, align 8, !dbg !87
  call void @"copy.Text"(ptr %t11, ptr %t8), !dbg !87
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t11, i64 24, i1 false), !dbg !86
  ret void, !dbg !86
L12:
  br label %L4, !dbg !84
L5:
  %t14 = load i64, ptr %t1, align 8, !dbg !89
  %t15 = icmp sgt i64 %t14, 0, !dbg !89
  br i1 %t15, label %L17, label %L18, !dbg !88
L17:
  store ptr @.str.1, ptr %t20, align 8, !dbg !91
  %t21 = getelementptr inbounds i8, ptr %t20, i64 8, !dbg !91
  store i64 1, ptr %t21, align 8, !dbg !91
  %t22 = getelementptr inbounds i8, ptr %t20, i64 16, !dbg !91
  store i64 1, ptr %t22, align 8, !dbg !91
  call void @"copy.Text"(ptr %t23, ptr %t20), !dbg !91
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t23, i64 24, i1 false), !dbg !88
  br label %L19, !dbg !88
L18:
  store ptr @.str.24, ptr %t24, align 8, !dbg !93
  %t25 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !93
  store i64 1, ptr %t25, align 8, !dbg !93
  %t26 = getelementptr inbounds i8, ptr %t24, i64 16, !dbg !93
  store i64 1, ptr %t26, align 8, !dbg !93
  call void @"copy.Text"(ptr %t27, ptr %t24), !dbg !93
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t27, i64 24, i1 false), !dbg !88
  br label %L19, !dbg !88
L19:
  call void @llvm.memcpy.p0.p0.i64(ptr %t3, ptr %t16, i64 24, i1 false), !dbg !84
  br label %L4, !dbg !84
L13:
  store ptr @.str.0, ptr %t28, align 8, !dbg !84
  %t29 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !84
  store i64 26, ptr %t29, align 8, !dbg !84
  %t30 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !84
  store i64 26, ptr %t30, align 8, !dbg !84
  call void @ignite_rt_fail_match(ptr %t28, i64 87, i64 11), !dbg !84
  unreachable, !dbg !84
L4:
  call void @llvm.memcpy.p0.p0.i64(ptr %t31, ptr %t3, i64 24, i1 false), !dbg !83
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t31, i64 24, i1 false), !dbg !82
  ret void, !dbg !82
}

define i64 @"tests.syntax.count_spaces"(ptr %p0) !dbg !95 {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [8 x i8], align 8
  %t16 = alloca [1 x i8], align 1
  %t28 = alloca [24 x i8], align 8
  %t31 = alloca [24 x i8], align 8
  store i64 0, ptr %t1, align 8, !dbg !97
  store i64 0, ptr %t2, align 8, !dbg !99
  %t3 = call i64 @ignite.arena_mark(), !dbg !99
  br label %L4
L4:
  %t8 = load i64, ptr %t2, align 8, !dbg !99
  %t9 = getelementptr inbounds i8, ptr %p0, i64 8, !dbg !99
  %t10 = load i64, ptr %t9, align 8, !dbg !99
  %t11 = icmp slt i64 %t8, %t10, !dbg !99
  br i1 %t11, label %L5, label %L7, !dbg !99
L5:
  %t12 = load ptr, ptr %p0, align 8, !dbg !99
  %t13 = mul i64 %t8, 1, !dbg !99
  %t14 = getelementptr inbounds i8, ptr %t12, i64 %t13, !dbg !99
  %t15 = load i8, ptr %t14, align 1, !dbg !99
  store i8 %t15, ptr %t16, align 1, !dbg !99
  %t17 = load i8, ptr %t16, align 1, !dbg !102
  %t20 = call i8 @"tests.syntax.SPACE"(), !dbg !101
  %t21 = icmp eq i8 %t17, %t20, !dbg !101
  br i1 %t21, label %L22, label %L19, !dbg !101
L22:
  %t23 = load i64, ptr %t1, align 8, !dbg !105
  %t24 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t23, i64 1), !dbg !105
  %t25 = extractvalue { i64, i1 } %t24, 1, !dbg !105
  br i1 %t25, label %L26, label %L27, !dbg !105
L26:
  store ptr @.str.0, ptr %t28, align 8, !dbg !105
  %t29 = getelementptr inbounds i8, ptr %t28, i64 8, !dbg !105
  store i64 26, ptr %t29, align 8, !dbg !105
  %t30 = getelementptr inbounds i8, ptr %t28, i64 16, !dbg !105
  store i64 26, ptr %t30, align 8, !dbg !105
  store ptr @.str.1, ptr %t31, align 8, !dbg !105
  %t32 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !105
  store i64 1, ptr %t32, align 8, !dbg !105
  %t33 = getelementptr inbounds i8, ptr %t31, i64 16, !dbg !105
  store i64 1, ptr %t33, align 8, !dbg !105
  call void @ignite_rt_fail_arith(ptr %t31, ptr %t28, i64 99, i64 13, i64 %t23, i64 1), !dbg !105
  unreachable, !dbg !105
L27:
  %t34 = extractvalue { i64, i1 } %t24, 0, !dbg !105
  store i64 %t34, ptr %t1, align 8, !dbg !104
  br label %L18, !dbg !101
L19:
  br label %L18, !dbg !101
L35:
  store ptr @.str.0, ptr %t31, align 8, !dbg !101
  %t36 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !101
  store i64 26, ptr %t36, align 8, !dbg !101
  %t37 = getelementptr inbounds i8, ptr %t31, i64 16, !dbg !101
  store i64 26, ptr %t37, align 8, !dbg !101
  call void @ignite_rt_fail_match(ptr %t31, i64 97, i64 5), !dbg !101
  unreachable, !dbg !101
L18:
  br label %L6
L6:
  %t38 = load i64, ptr %t2, align 8, !dbg !99
  %t39 = add i64 %t38, 1, !dbg !99
  store i64 %t39, ptr %t2, align 8, !dbg !99
  call void @ignite.arena_release(i64 %t3), !dbg !99
  br label %L4, !dbg !99
L7:
  %t40 = load i64, ptr %t1, align 8, !dbg !108
  ret i64 %t40, !dbg !96
}

define void @"tests.syntax.show"(ptr %sret, ptr %p0) !dbg !109 {
entry:
  %t1 = alloca [24 x i8], align 8
  %t9 = alloca [40 x i8], align 8
  %t10 = alloca [32 x i8], align 8
  call void @"copy.List<Byte>"(ptr %t1, ptr %p0), !dbg !111
  call void @ignite.reserve(ptr %t1, i64 1, i64 1, i64 1), !dbg !113
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !113
  %t3 = load i64, ptr %t2, align 8, !dbg !113
  %t4 = load ptr, ptr %t1, align 8, !dbg !113
  %t5 = mul i64 %t3, 1, !dbg !113
  %t6 = getelementptr inbounds i8, ptr %t4, i64 %t5, !dbg !113
  store i8 10, ptr %t6, align 1, !dbg !113
  %t7 = add i64 %t3, 1, !dbg !113
  %t8 = getelementptr inbounds i8, ptr %t1, i64 8, !dbg !113
  store i64 %t7, ptr %t8, align 8, !dbg !113
  %t11 = call i32 @ignite_rt_stdout_write_all(ptr %t1, ptr %t10), !dbg !115
  %t12 = icmp ne i32 %t11, 0, !dbg !115
  br i1 %t12, label %L13, label %L14, !dbg !115
L13:
  store i32 1, ptr %t9, align 4, !dbg !115
  %t16 = getelementptr inbounds i8, ptr %t9, i64 8, !dbg !115
  call void @llvm.memcpy.p0.p0.i64(ptr %t16, ptr %t10, i64 32, i1 false), !dbg !115
  br label %L15, !dbg !115
L14:
  store i32 0, ptr %t9, align 4, !dbg !115
  br label %L15, !dbg !115
L15:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t9, i64 40, i1 false), !dbg !110
  ret void, !dbg !110
}

define internal void @"fnval.tests.syntax.shout"(ptr %env, ptr %a0, ptr %a1) {
entry:
  call void @"tests.syntax.shout"(ptr %a0, ptr %a1)
  ret void
}

define void @"tests.syntax.Syntax.connect"(ptr %sret, i64 %p0) !dbg !117 {
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
  %t21 = alloca [24 x i8], align 8
  %t22 = alloca [24 x i8], align 8
  %t24 = alloca [40 x i8], align 8
  %t34 = alloca [24 x i8], align 8
  %t35 = alloca [40 x i8], align 8
  %t45 = alloca [40 x i8], align 8
  %t56 = alloca [24 x i8], align 8
  %t57 = alloca [24 x i8], align 8
  %t60 = alloca [24 x i8], align 8
  %t61 = alloca [40 x i8], align 8
  %t70 = alloca [24 x i8], align 8
  %t73 = alloca [24 x i8], align 8
  %t74 = alloca [24 x i8], align 8
  %t77 = alloca [24 x i8], align 8
  %t79 = alloca [24 x i8], align 8
  %t82 = alloca [24 x i8], align 8
  %t83 = alloca [40 x i8], align 8
  %t92 = alloca [24 x i8], align 8
  %t95 = alloca [24 x i8], align 8
  %t96 = alloca [24 x i8], align 8
  %t99 = alloca [24 x i8], align 8
  %t101 = alloca [24 x i8], align 8
  %t104 = alloca [24 x i8], align 8
  %t105 = alloca [40 x i8], align 8
  %t114 = alloca [24 x i8], align 8
  %t117 = alloca [24 x i8], align 8
  %t118 = alloca [24 x i8], align 8
  %t121 = alloca [24 x i8], align 8
  %t123 = alloca [24 x i8], align 8
  %t126 = alloca [24 x i8], align 8
  %t128 = alloca [24 x i8], align 8
  %t131 = alloca [24 x i8], align 8
  %t132 = alloca [40 x i8], align 8
  %t140 = alloca [32 x i8], align 8
  %t144 = alloca [24 x i8], align 8
  %t148 = alloca [40 x i8], align 8
  %t150 = alloca [24 x i8], align 8
  %t151 = alloca [40 x i8], align 8
  %t159 = alloca [32 x i8], align 8
  %t162 = alloca [24 x i8], align 8
  %t166 = alloca [40 x i8], align 8
  %t168 = alloca [24 x i8], align 8
  %t169 = alloca [40 x i8], align 8
  %t177 = alloca [40 x i8], align 8
  %t178 = alloca [24 x i8], align 8
  %t179 = alloca [40 x i8], align 8
  %t187 = alloca [32 x i8], align 8
  %t190 = alloca [24 x i8], align 8
  %t194 = alloca [40 x i8], align 8
  %t196 = alloca [24 x i8], align 8
  %t197 = alloca [40 x i8], align 8
  %t205 = alloca [24 x i8], align 8
  %t208 = alloca [24 x i8], align 8
  %t210 = alloca [1 x i8], align 1
  %t212 = alloca [24 x i8], align 8
  %t215 = alloca [24 x i8], align 8
  %t217 = alloca [1 x i8], align 1
  %t221 = alloca [24 x i8], align 8
  %t224 = alloca [24 x i8], align 8
  %t228 = alloca [24 x i8], align 8
  %t229 = alloca [24 x i8], align 8
  %t232 = alloca [24 x i8], align 8
  %t233 = alloca [24 x i8], align 8
  %t241 = alloca [40 x i8], align 8
  %t257 = alloca [40 x i8], align 8
  %t266 = alloca [40 x i8], align 8
  %t277 = alloca [24 x i8], align 8
  %t278 = alloca [40 x i8], align 8
  %t286 = alloca [8 x i8], align 8
  %t289 = alloca [24 x i8], align 8
  %t290 = alloca [40 x i8], align 8
  %t298 = alloca [24 x i8], align 8
  %t301 = alloca [24 x i8], align 8
  %t302 = alloca [24 x i8], align 8
  %t303 = alloca [40 x i8], align 8
  %t311 = alloca [40 x i8], align 8
  %t312 = alloca [24 x i8], align 8
  %t315 = alloca [24 x i8], align 8
  %t316 = alloca [16 x i8], align 8
  %t319 = alloca [40 x i8], align 8
  %t324 = alloca [24 x i8], align 8
  %t325 = alloca [40 x i8], align 8
  %t333 = alloca [40 x i8], align 8
  %t335 = alloca [24 x i8], align 8
  %t338 = alloca [24 x i8], align 8
  %t339 = alloca [24 x i8], align 8
  %t342 = alloca [8 x i8], align 8
  %t346 = alloca [40 x i8], align 8
  %t347 = alloca [24 x i8], align 8
  %t350 = alloca [24 x i8], align 8
  %t353 = alloca [24 x i8], align 8
  %t354 = alloca [24 x i8], align 8
  %t355 = alloca [40 x i8], align 8
  %t371 = alloca [40 x i8], align 8
  %t385 = alloca [24 x i8], align 8
  %t386 = alloca [40 x i8], align 8
  %t394 = alloca [24 x i8], align 8
  %t397 = alloca [1 x i8], align 1
  %t400 = alloca [1 x i8], align 1
  %t403 = alloca [24 x i8], align 8
  %t406 = alloca [24 x i8], align 8
  %t410 = alloca [24 x i8], align 8
  %t411 = alloca [24 x i8], align 8
  %t414 = alloca [24 x i8], align 8
  %t415 = alloca [24 x i8], align 8
  %t420 = alloca [40 x i8], align 8
  %t428 = alloca [32 x i8], align 8
  %t432 = alloca [32 x i8], align 8
  %t433 = alloca [8 x i8], align 8
  %t434 = alloca [32 x i8], align 8
  %t436 = alloca [8 x i8], align 8
  %t443 = alloca [32 x i8], align 8
  %t447 = alloca [8 x i8], align 8
  %t451 = alloca [32 x i8], align 8
  %t464 = alloca [24 x i8], align 8
  %t481 = alloca [32 x i8], align 8
  %t485 = alloca [8 x i8], align 8
  %t491 = alloca [8 x i8], align 8
  %t493 = alloca [24 x i8], align 8
  %t496 = alloca [24 x i8], align 8
  %t497 = alloca [24 x i8], align 8
  %t499 = alloca [40 x i8], align 8
  %t507 = alloca [32 x i8], align 8
  %t511 = alloca [32 x i8], align 8
  %t512 = alloca [32 x i8], align 8
  %t516 = alloca [8 x i8], align 8
  %t525 = alloca [24 x i8], align 8
  %t528 = alloca [24 x i8], align 8
  %t534 = alloca [24 x i8], align 8
  %t536 = alloca [24 x i8], align 8
  %t537 = alloca [40 x i8], align 8
  %t555 = alloca [8 x i8], align 8
  %t556 = alloca [8 x i8], align 8
  %t562 = alloca [24 x i8], align 8
  %t565 = alloca [24 x i8], align 8
  %t566 = alloca [40 x i8], align 8
  %t578 = alloca [24 x i8], align 8
  %t581 = alloca [24 x i8], align 8
  %t582 = alloca [40 x i8], align 8
  %t590 = alloca [24 x i8], align 8
  %t593 = alloca [24 x i8], align 8
  %t594 = alloca [24 x i8], align 8
  %t597 = alloca [24 x i8], align 8
  %t598 = alloca [24 x i8], align 8
  %t599 = alloca [40 x i8], align 8
  %t607 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8, !dbg !118
  store i64 7, ptr %t2, align 8, !dbg !121
  store ptr @.str.25, ptr %t3, align 8, !dbg !123
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !123
  store i64 5, ptr %t4, align 8, !dbg !123
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16, !dbg !123
  store i64 5, ptr %t5, align 8, !dbg !123
  call void @"copy.Text"(ptr %t6, ptr %t3), !dbg !123
  %t7 = getelementptr inbounds i8, ptr %t2, i64 8, !dbg !121
  call void @llvm.memcpy.p0.p0.i64(ptr %t7, ptr %t6, i64 24, i1 false), !dbg !121
  call void @"tests.syntax.swap"(ptr %t8, ptr %t2), !dbg !120
  %t11 = getelementptr inbounds i8, ptr %t8, i64 24, !dbg !119
  %t12 = load i64, ptr %t11, align 8, !dbg !119
  store i64 %t12, ptr %t13, align 8, !dbg !119
  br label %L9, !dbg !119
L10:
  store ptr @.str.0, ptr %t14, align 8, !dbg !119
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8, !dbg !119
  store i64 26, ptr %t15, align 8, !dbg !119
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16, !dbg !119
  store i64 26, ptr %t16, align 8, !dbg !119
  call void @ignite_rt_fail_match(ptr %t14, i64 115, i64 9), !dbg !119
  unreachable, !dbg !119
L9:
  call void @"copy.Text"(ptr %t17, ptr %t8), !dbg !126
  store ptr @.str.16, ptr %t18, align 8, !dbg !125
  %t19 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !125
  store i64 1, ptr %t19, align 8, !dbg !125
  %t20 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !125
  store i64 1, ptr %t20, align 8, !dbg !125
  call void @"copy.Text"(ptr %t21, ptr %t18), !dbg !125
  call void @ignite_rt_text_concat(ptr %t22, ptr %t17, ptr %t21), !dbg !125
  %t23 = load i64, ptr %t13, align 8, !dbg !127
  call void @ignite_rt_int_to_text(ptr %t21, i64 %t23), !dbg !127
  call void @ignite.append_text(ptr %t22, ptr %t21), !dbg !125
  call void @"tests.syntax.show"(ptr %t24, ptr %t22), !dbg !124
  %t25 = load i32, ptr %t24, align 4, !dbg !124
  %t26 = icmp eq i32 %t25, 1, !dbg !124
  br i1 %t26, label %L27, label %L28, !dbg !124
L27:
  store i32 1, ptr %sret, align 4, !dbg !124
  %t29 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !124
  %t30 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !124
  call void @llvm.memcpy.p0.p0.i64(ptr %t30, ptr %t29, i64 32, i1 false), !dbg !124
  ret void, !dbg !124
L28:
  %t31 = getelementptr inbounds i8, ptr %t24, i64 8, !dbg !124
  call void @"tests.syntax.describe"(ptr %t21, i64 0), !dbg !129
  store ptr @.str.16, ptr %t18, align 8, !dbg !131
  %t32 = getelementptr inbounds i8, ptr %t18, i64 8, !dbg !131
  store i64 1, ptr %t32, align 8, !dbg !131
  %t33 = getelementptr inbounds i8, ptr %t18, i64 16, !dbg !131
  store i64 1, ptr %t33, align 8, !dbg !131
  call void @"copy.Text"(ptr %t17, ptr %t18), !dbg !131
  call void @ignite_rt_text_concat(ptr %t34, ptr %t21, ptr %t17), !dbg !129
  call void @"tests.syntax.describe"(ptr %t17, i64 41), !dbg !132
  call void @ignite.append_text(ptr %t34, ptr %t17), !dbg !129
  call void @ignite.append(ptr %t34, ptr @.str.16, i64 1), !dbg !129
  call void @"tests.syntax.describe"(ptr %t17, i64 9), !dbg !134
  call void @ignite.append_text(ptr %t34, ptr %t17), !dbg !129
  call void @"tests.syntax.show"(ptr %t35, ptr %t34), !dbg !128
  %t36 = load i32, ptr %t35, align 4, !dbg !128
  %t37 = icmp eq i32 %t36, 1, !dbg !128
  br i1 %t37, label %L38, label %L39, !dbg !128
L38:
  store i32 1, ptr %sret, align 4, !dbg !128
  %t40 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !128
  %t41 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !128
  call void @llvm.memcpy.p0.p0.i64(ptr %t41, ptr %t40, i64 32, i1 false), !dbg !128
  ret void, !dbg !128
L39:
  %t42 = getelementptr inbounds i8, ptr %t35, i64 8, !dbg !128
  store ptr @ignite.empty, ptr %t17, align 8, !dbg !138
  %t43 = getelementptr inbounds i8, ptr %t17, i64 8, !dbg !138
  store i64 0, ptr %t43, align 8, !dbg !138
  %t44 = getelementptr inbounds i8, ptr %t17, i64 16, !dbg !138
  store i64 0, ptr %t44, align 8, !dbg !138
  call void @"tests.syntax.classify"(ptr %t18, ptr %t17), !dbg !137
  call void @"tests.syntax.show"(ptr %t45, ptr %t18), !dbg !136
  %t46 = load i32, ptr %t45, align 4, !dbg !136
  %t47 = icmp eq i32 %t46, 1, !dbg !136
  br i1 %t47, label %L48, label %L49, !dbg !136
L48:
  store i32 1, ptr %sret, align 4, !dbg !136
  %t50 = getelementptr inbounds i8, ptr %t45, i64 8, !dbg !136
  %t51 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !136
  call void @llvm.memcpy.p0.p0.i64(ptr %t51, ptr %t50, i64 32, i1 false), !dbg !136
  ret void, !dbg !136
L49:
  %t52 = getelementptr inbounds i8, ptr %t45, i64 8, !dbg !136
  %t53 = call ptr @ignite.alloc(i64 24, i64 8), !dbg !141
  store ptr @.str.26, ptr %t21, align 8, !dbg !142
  %t54 = getelementptr inbounds i8, ptr %t21, i64 8, !dbg !142
  store i64 1, ptr %t54, align 8, !dbg !142
  %t55 = getelementptr inbounds i8, ptr %t21, i64 16, !dbg !142
  store i64 1, ptr %t55, align 8, !dbg !142
  call void @"copy.Text"(ptr %t56, ptr %t21), !dbg !142
  call void @llvm.memcpy.p0.p0.i64(ptr %t53, ptr %t56, i64 24, i1 false), !dbg !141
  store ptr %t53, ptr %t57, align 8, !dbg !141
  %t58 = getelementptr inbounds i8, ptr %t57, i64 8, !dbg !141
  store i64 1, ptr %t58, align 8, !dbg !141
  %t59 = getelementptr inbounds i8, ptr %t57, i64 16, !dbg !141
  store i64 1, ptr %t59, align 8, !dbg !141
  call void @"tests.syntax.classify"(ptr %t60, ptr %t57), !dbg !140
  call void @"tests.syntax.show"(ptr %t61, ptr %t60), !dbg !139
  %t62 = load i32, ptr %t61, align 4, !dbg !139
  %t63 = icmp eq i32 %t62, 1, !dbg !139
  br i1 %t63, label %L64, label %L65, !dbg !139
L64:
  store i32 1, ptr %sret, align 4, !dbg !139
  %t66 = getelementptr inbounds i8, ptr %t61, i64 8, !dbg !139
  %t67 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !139
  call void @llvm.memcpy.p0.p0.i64(ptr %t67, ptr %t66, i64 32, i1 false), !dbg !139
  ret void, !dbg !139
L65:
  %t68 = getelementptr inbounds i8, ptr %t61, i64 8, !dbg !139
  %t69 = call ptr @ignite.alloc(i64 48, i64 8), !dbg !145
  store ptr @.str.2, ptr %t70, align 8, !dbg !146
  %t71 = getelementptr inbounds i8, ptr %t70, i64 8, !dbg !146
  store i64 4, ptr %t71, align 8, !dbg !146
  %t72 = getelementptr inbounds i8, ptr %t70, i64 16, !dbg !146
  store i64 4, ptr %t72, align 8, !dbg !146
  call void @"copy.Text"(ptr %t73, ptr %t70), !dbg !146
  call void @llvm.memcpy.p0.p0.i64(ptr %t69, ptr %t73, i64 24, i1 false), !dbg !145
  store ptr @.str.27, ptr %t74, align 8, !dbg !147
  %t75 = getelementptr inbounds i8, ptr %t74, i64 8, !dbg !147
  store i64 3, ptr %t75, align 8, !dbg !147
  %t76 = getelementptr inbounds i8, ptr %t74, i64 16, !dbg !147
  store i64 3, ptr %t76, align 8, !dbg !147
  call void @"copy.Text"(ptr %t77, ptr %t74), !dbg !147
  %t78 = getelementptr inbounds i8, ptr %t69, i64 24, !dbg !145
  call void @llvm.memcpy.p0.p0.i64(ptr %t78, ptr %t77, i64 24, i1 false), !dbg !145
  store ptr %t69, ptr %t79, align 8, !dbg !145
  %t80 = getelementptr inbounds i8, ptr %t79, i64 8, !dbg !145
  store i64 2, ptr %t80, align 8, !dbg !145
  %t81 = getelementptr inbounds i8, ptr %t79, i64 16, !dbg !145
  store i64 2, ptr %t81, align 8, !dbg !145
  call void @"tests.syntax.classify"(ptr %t82, ptr %t79), !dbg !144
  call void @"tests.syntax.show"(ptr %t83, ptr %t82), !dbg !143
  %t84 = load i32, ptr %t83, align 4, !dbg !143
  %t85 = icmp eq i32 %t84, 1, !dbg !143
  br i1 %t85, label %L86, label %L87, !dbg !143
L86:
  store i32 1, ptr %sret, align 4, !dbg !143
  %t88 = getelementptr inbounds i8, ptr %t83, i64 8, !dbg !143
  %t89 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !143
  call void @llvm.memcpy.p0.p0.i64(ptr %t89, ptr %t88, i64 32, i1 false), !dbg !143
  ret void, !dbg !143
L87:
  %t90 = getelementptr inbounds i8, ptr %t83, i64 8, !dbg !143
  %t91 = call ptr @ignite.alloc(i64 48, i64 8), !dbg !150
  store ptr @.str.18, ptr %t92, align 8, !dbg !151
  %t93 = getelementptr inbounds i8, ptr %t92, i64 8, !dbg !151
  store i64 1, ptr %t93, align 8, !dbg !151
  %t94 = getelementptr inbounds i8, ptr %t92, i64 16, !dbg !151
  store i64 1, ptr %t94, align 8, !dbg !151
  call void @"copy.Text"(ptr %t95, ptr %t92), !dbg !151
  call void @llvm.memcpy.p0.p0.i64(ptr %t91, ptr %t95, i64 24, i1 false), !dbg !150
  store ptr @.str.28, ptr %t96, align 8, !dbg !152
  %t97 = getelementptr inbounds i8, ptr %t96, i64 8, !dbg !152
  store i64 1, ptr %t97, align 8, !dbg !152
  %t98 = getelementptr inbounds i8, ptr %t96, i64 16, !dbg !152
  store i64 1, ptr %t98, align 8, !dbg !152
  call void @"copy.Text"(ptr %t99, ptr %t96), !dbg !152
  %t100 = getelementptr inbounds i8, ptr %t91, i64 24, !dbg !150
  call void @llvm.memcpy.p0.p0.i64(ptr %t100, ptr %t99, i64 24, i1 false), !dbg !150
  store ptr %t91, ptr %t101, align 8, !dbg !150
  %t102 = getelementptr inbounds i8, ptr %t101, i64 8, !dbg !150
  store i64 2, ptr %t102, align 8, !dbg !150
  %t103 = getelementptr inbounds i8, ptr %t101, i64 16, !dbg !150
  store i64 2, ptr %t103, align 8, !dbg !150
  call void @"tests.syntax.classify"(ptr %t104, ptr %t101), !dbg !149
  call void @"tests.syntax.show"(ptr %t105, ptr %t104), !dbg !148
  %t106 = load i32, ptr %t105, align 4, !dbg !148
  %t107 = icmp eq i32 %t106, 1, !dbg !148
  br i1 %t107, label %L108, label %L109, !dbg !148
L108:
  store i32 1, ptr %sret, align 4, !dbg !148
  %t110 = getelementptr inbounds i8, ptr %t105, i64 8, !dbg !148
  %t111 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !148
  call void @llvm.memcpy.p0.p0.i64(ptr %t111, ptr %t110, i64 32, i1 false), !dbg !148
  ret void, !dbg !148
L109:
  %t112 = getelementptr inbounds i8, ptr %t105, i64 8, !dbg !148
  %t113 = call ptr @ignite.alloc(i64 72, i64 8), !dbg !155
  store ptr @.str.18, ptr %t114, align 8, !dbg !156
  %t115 = getelementptr inbounds i8, ptr %t114, i64 8, !dbg !156
  store i64 1, ptr %t115, align 8, !dbg !156
  %t116 = getelementptr inbounds i8, ptr %t114, i64 16, !dbg !156
  store i64 1, ptr %t116, align 8, !dbg !156
  call void @"copy.Text"(ptr %t117, ptr %t114), !dbg !156
  call void @llvm.memcpy.p0.p0.i64(ptr %t113, ptr %t117, i64 24, i1 false), !dbg !155
  store ptr @.str.28, ptr %t118, align 8, !dbg !157
  %t119 = getelementptr inbounds i8, ptr %t118, i64 8, !dbg !157
  store i64 1, ptr %t119, align 8, !dbg !157
  %t120 = getelementptr inbounds i8, ptr %t118, i64 16, !dbg !157
  store i64 1, ptr %t120, align 8, !dbg !157
  call void @"copy.Text"(ptr %t121, ptr %t118), !dbg !157
  %t122 = getelementptr inbounds i8, ptr %t113, i64 24, !dbg !155
  call void @llvm.memcpy.p0.p0.i64(ptr %t122, ptr %t121, i64 24, i1 false), !dbg !155
  store ptr @.str.29, ptr %t123, align 8, !dbg !158
  %t124 = getelementptr inbounds i8, ptr %t123, i64 8, !dbg !158
  store i64 1, ptr %t124, align 8, !dbg !158
  %t125 = getelementptr inbounds i8, ptr %t123, i64 16, !dbg !158
  store i64 1, ptr %t125, align 8, !dbg !158
  call void @"copy.Text"(ptr %t126, ptr %t123), !dbg !158
  %t127 = getelementptr inbounds i8, ptr %t113, i64 48, !dbg !155
  call void @llvm.memcpy.p0.p0.i64(ptr %t127, ptr %t126, i64 24, i1 false), !dbg !155
  store ptr %t113, ptr %t128, align 8, !dbg !155
  %t129 = getelementptr inbounds i8, ptr %t128, i64 8, !dbg !155
  store i64 3, ptr %t129, align 8, !dbg !155
  %t130 = getelementptr inbounds i8, ptr %t128, i64 16, !dbg !155
  store i64 3, ptr %t130, align 8, !dbg !155
  call void @"tests.syntax.classify"(ptr %t131, ptr %t128), !dbg !154
  call void @"tests.syntax.show"(ptr %t132, ptr %t131), !dbg !153
  %t133 = load i32, ptr %t132, align 4, !dbg !153
  %t134 = icmp eq i32 %t133, 1, !dbg !153
  br i1 %t134, label %L135, label %L136, !dbg !153
L135:
  store i32 1, ptr %sret, align 4, !dbg !153
  %t137 = getelementptr inbounds i8, ptr %t132, i64 8, !dbg !153
  %t138 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !153
  call void @llvm.memcpy.p0.p0.i64(ptr %t138, ptr %t137, i64 32, i1 false), !dbg !153
  ret void, !dbg !153
L136:
  %t139 = getelementptr inbounds i8, ptr %t132, i64 8, !dbg !153
  %t141 = zext i1 true to i8, !dbg !162
  store i8 %t141, ptr %t140, align 1, !dbg !162
  %t142 = call ptr @ignite.alloc(i64 16, i64 8), !dbg !164
  store i64 1, ptr %t142, align 8, !dbg !164
  %t143 = getelementptr inbounds i8, ptr %t142, i64 8, !dbg !164
  store i64 2, ptr %t143, align 8, !dbg !164
  store ptr %t142, ptr %t144, align 8, !dbg !164
  %t145 = getelementptr inbounds i8, ptr %t144, i64 8, !dbg !164
  store i64 2, ptr %t145, align 8, !dbg !164
  %t146 = getelementptr inbounds i8, ptr %t144, i64 16, !dbg !164
  store i64 2, ptr %t146, align 8, !dbg !164
  %t147 = getelementptr inbounds i8, ptr %t140, i64 8, !dbg !162
  call void @llvm.memcpy.p0.p0.i64(ptr %t147, ptr %t144, i64 24, i1 false), !dbg !162
  store i32 0, ptr %t148, align 4, !dbg !161
  %t149 = getelementptr inbounds i8, ptr %t148, i64 8, !dbg !161
  call void @llvm.memcpy.p0.p0.i64(ptr %t149, ptr %t140, i64 32, i1 false), !dbg !161
  call void @"tests.syntax.nested"(ptr %t150, ptr %t148), !dbg !160
  call void @"tests.syntax.show"(ptr %t151, ptr %t150), !dbg !159
  %t152 = load i32, ptr %t151, align 4, !dbg !159
  %t153 = icmp eq i32 %t152, 1, !dbg !159
  br i1 %t153, label %L154, label %L155, !dbg !159
L154:
  store i32 1, ptr %sret, align 4, !dbg !159
  %t156 = getelementptr inbounds i8, ptr %t151, i64 8, !dbg !159
  %t157 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !159
  call void @llvm.memcpy.p0.p0.i64(ptr %t157, ptr %t156, i64 32, i1 false), !dbg !159
  ret void, !dbg !159
L155:
  %t158 = getelementptr inbounds i8, ptr %t151, i64 8, !dbg !159
  %t160 = zext i1 false to i8, !dbg !170
  store i8 %t160, ptr %t159, align 1, !dbg !170
  %t161 = call ptr @ignite.alloc(i64 8, i64 8), !dbg !172
  store i64 1, ptr %t161, align 8, !dbg !172
  store ptr %t161, ptr %t162, align 8, !dbg !172
  %t163 = getelementptr inbounds i8, ptr %t162, i64 8, !dbg !172
  store i64 1, ptr %t163, align 8, !dbg !172
  %t164 = getelementptr inbounds i8, ptr %t162, i64 16, !dbg !172
  store i64 1, ptr %t164, align 8, !dbg !172
  %t165 = getelementptr inbounds i8, ptr %t159, i64 8, !dbg !170
  call void @llvm.memcpy.p0.p0.i64(ptr %t165, ptr %t162, i64 24, i1 false), !dbg !170
  store i32 0, ptr %t166, align 4, !dbg !169
  %t167 = getelementptr inbounds i8, ptr %t166, i64 8, !dbg !169
  call void @llvm.memcpy.p0.p0.i64(ptr %t167, ptr %t159, i64 32, i1 false), !dbg !169
  call void @"tests.syntax.nested"(ptr %t168, ptr %t166), !dbg !168
  call void @"tests.syntax.show"(ptr %t169, ptr %t168), !dbg !167
  %t170 = load i32, ptr %t169, align 4, !dbg !167
  %t171 = icmp eq i32 %t170, 1, !dbg !167
  br i1 %t171, label %L172, label %L173, !dbg !167
L172:
  store i32 1, ptr %sret, align 4, !dbg !167
  %t174 = getelementptr inbounds i8, ptr %t169, i64 8, !dbg !167
  %t175 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !167
  call void @llvm.memcpy.p0.p0.i64(ptr %t175, ptr %t174, i64 32, i1 false), !dbg !167
  ret void, !dbg !167
L173:
  %t176 = getelementptr inbounds i8, ptr %t169, i64 8, !dbg !167
  store i32 1, ptr %t177, align 4, !dbg !176
  call void @"tests.syntax.nested"(ptr %t178, ptr %t177), !dbg !175
  call void @"tests.syntax.show"(ptr %t179, ptr %t178), !dbg !174
  %t180 = load i32, ptr %t179, align 4, !dbg !174
  %t181 = icmp eq i32 %t180, 1, !dbg !174
  br i1 %t181, label %L182, label %L183, !dbg !174
L182:
  store i32 1, ptr %sret, align 4, !dbg !174
  %t184 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !174
  %t185 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !174
  call void @llvm.memcpy.p0.p0.i64(ptr %t185, ptr %t184, i64 32, i1 false), !dbg !174
  ret void, !dbg !174
L183:
  %t186 = getelementptr inbounds i8, ptr %t179, i64 8, !dbg !174
  %t188 = zext i1 true to i8, !dbg !180
  store i8 %t188, ptr %t187, align 1, !dbg !180
  %t189 = call ptr @ignite.alloc(i64 8, i64 8), !dbg !182
  store i64 3, ptr %t189, align 8, !dbg !182
  store ptr %t189, ptr %t190, align 8, !dbg !182
  %t191 = getelementptr inbounds i8, ptr %t190, i64 8, !dbg !182
  store i64 1, ptr %t191, align 8, !dbg !182
  %t192 = getelementptr inbounds i8, ptr %t190, i64 16, !dbg !182
  store i64 1, ptr %t192, align 8, !dbg !182
  %t193 = getelementptr inbounds i8, ptr %t187, i64 8, !dbg !180
  call void @llvm.memcpy.p0.p0.i64(ptr %t193, ptr %t190, i64 24, i1 false), !dbg !180
  store i32 0, ptr %t194, align 4, !dbg !179
  %t195 = getelementptr inbounds i8, ptr %t194, i64 8, !dbg !179
  call void @llvm.memcpy.p0.p0.i64(ptr %t195, ptr %t187, i64 32, i1 false), !dbg !179
  call void @"tests.syntax.nested"(ptr %t196, ptr %t194), !dbg !178
  call void @"tests.syntax.show"(ptr %t197, ptr %t196), !dbg !177
  %t198 = load i32, ptr %t197, align 4, !dbg !177
  %t199 = icmp eq i32 %t198, 1, !dbg !177
  br i1 %t199, label %L200, label %L201, !dbg !177
L200:
  store i32 1, ptr %sret, align 4, !dbg !177
  %t202 = getelementptr inbounds i8, ptr %t197, i64 8, !dbg !177
  %t203 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !177
  call void @llvm.memcpy.p0.p0.i64(ptr %t203, ptr %t202, i64 32, i1 false), !dbg !177
  ret void, !dbg !177
L201:
  %t204 = getelementptr inbounds i8, ptr %t197, i64 8, !dbg !177
  store ptr @.str.19, ptr %t205, align 8, !dbg !186
  %t206 = getelementptr inbounds i8, ptr %t205, i64 8, !dbg !186
  store i64 1, ptr %t206, align 8, !dbg !186
  %t207 = getelementptr inbounds i8, ptr %t205, i64 16, !dbg !186
  store i64 1, ptr %t207, align 8, !dbg !186
  call void @"copy.Text"(ptr %t208, ptr %t205), !dbg !186
  %t209 = call i1 @"tests.syntax.vowel"(ptr %t208), !dbg !185
  %t211 = zext i1 %t209 to i8, !dbg !184
  store i8 %t211, ptr %t210, align 1, !dbg !184
  store ptr @.str.26, ptr %t212, align 8, !dbg !189
  %t213 = getelementptr inbounds i8, ptr %t212, i64 8, !dbg !189
  store i64 1, ptr %t213, align 8, !dbg !189
  %t214 = getelementptr inbounds i8, ptr %t212, i64 16, !dbg !189
  store i64 1, ptr %t214, align 8, !dbg !189
  call void @"copy.Text"(ptr %t215, ptr %t212), !dbg !189
  %t216 = call i1 @"tests.syntax.vowel"(ptr %t215), !dbg !188
  %t218 = zext i1 %t216 to i8, !dbg !187
  store i8 %t218, ptr %t217, align 1, !dbg !187
  %t219 = load i8, ptr %t210, align 1, !dbg !192
  %t220 = trunc i8 %t219 to i1, !dbg !192
  store ptr @.str.30, ptr %t221, align 8, !dbg !192
  %t222 = getelementptr inbounds i8, ptr %t221, i64 8, !dbg !192
  store i64 4, ptr %t222, align 8, !dbg !192
  %t223 = getelementptr inbounds i8, ptr %t221, i64 16, !dbg !192
  store i64 4, ptr %t223, align 8, !dbg !192
  store ptr @.str.31, ptr %t224, align 8, !dbg !192
  %t225 = getelementptr inbounds i8, ptr %t224, i64 8, !dbg !192
  store i64 5, ptr %t225, align 8, !dbg !192
  %t226 = getelementptr inbounds i8, ptr %t224, i64 16, !dbg !192
  store i64 5, ptr %t226, align 8, !dbg !192
  %t227 = select i1 %t220, ptr %t221, ptr %t224, !dbg !192
  call void @"copy.Text"(ptr %t228, ptr %t227), !dbg !192
  store ptr @.str.16, ptr %t229, align 8, !dbg !191
  %t230 = getelementptr inbounds i8, ptr %t229, i64 8, !dbg !191
  store i64 1, ptr %t230, align 8, !dbg !191
  %t231 = getelementptr inbounds i8, ptr %t229, i64 16, !dbg !191
  store i64 1, ptr %t231, align 8, !dbg !191
  call void @"copy.Text"(ptr %t232, ptr %t229), !dbg !191
  call void @ignite_rt_text_concat(ptr %t233, ptr %t228, ptr %t232), !dbg !191
  %t234 = load i8, ptr %t217, align 1, !dbg !193
  %t235 = trunc i8 %t234 to i1, !dbg !193
  store ptr @.str.30, ptr %t232, align 8, !dbg !193
  %t236 = getelementptr inbounds i8, ptr %t232, i64 8, !dbg !193
  store i64 4, ptr %t236, align 8, !dbg !193
  %t237 = getelementptr inbounds i8, ptr %t232, i64 16, !dbg !193
  store i64 4, ptr %t237, align 8, !dbg !193
  store ptr @.str.31, ptr %t229, align 8, !dbg !193
  %t238 = getelementptr inbounds i8, ptr %t229, i64 8, !dbg !193
  store i64 5, ptr %t238, align 8, !dbg !193
  %t239 = getelementptr inbounds i8, ptr %t229, i64 16, !dbg !193
  store i64 5, ptr %t239, align 8, !dbg !193
  %t240 = select i1 %t235, ptr %t232, ptr %t229, !dbg !193
  call void @"copy.Text"(ptr %t228, ptr %t240), !dbg !193
  call void @ignite.append_text(ptr %t233, ptr %t228), !dbg !191
  call void @"tests.syntax.show"(ptr %t241, ptr %t233), !dbg !190
  %t242 = load i32, ptr %t241, align 4, !dbg !190
  %t243 = icmp eq i32 %t242, 1, !dbg !190
  br i1 %t243, label %L244, label %L245, !dbg !190
L244:
  store i32 1, ptr %sret, align 4, !dbg !190
  %t246 = getelementptr inbounds i8, ptr %t241, i64 8, !dbg !190
  %t247 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !190
  call void @llvm.memcpy.p0.p0.i64(ptr %t247, ptr %t246, i64 32, i1 false), !dbg !190
  ret void, !dbg !190
L245:
  %t248 = getelementptr inbounds i8, ptr %t241, i64 8, !dbg !190
  %t249 = call ptr @ignite.alloc(i64 32, i64 8), !dbg !196
  %t250 = sub i64 0, 1, !dbg !197
  store i64 %t250, ptr %t249, align 8, !dbg !196
  %t251 = getelementptr inbounds i8, ptr %t249, i64 8, !dbg !196
  store i64 0, ptr %t251, align 8, !dbg !196
  %t252 = getelementptr inbounds i8, ptr %t249, i64 16, !dbg !196
  store i64 5, ptr %t252, align 8, !dbg !196
  %t253 = getelementptr inbounds i8, ptr %t249, i64 24, !dbg !196
  store i64 6, ptr %t253, align 8, !dbg !196
  store ptr %t249, ptr %t228, align 8, !dbg !196
  %t254 = getelementptr inbounds i8, ptr %t228, i64 8, !dbg !196
  store i64 4, ptr %t254, align 8, !dbg !196
  %t255 = getelementptr inbounds i8, ptr %t228, i64 16, !dbg !196
  store i64 4, ptr %t255, align 8, !dbg !196
  %t256 = call i64 @"tests.syntax.first_positive"(ptr %t228), !dbg !195
  call void @ignite_rt_int_to_text(ptr %t229, i64 %t256), !dbg !195
  call void @"tests.syntax.show"(ptr %t257, ptr %t229), !dbg !194
  %t258 = load i32, ptr %t257, align 4, !dbg !194
  %t259 = icmp eq i32 %t258, 1, !dbg !194
  br i1 %t259, label %L260, label %L261, !dbg !194
L260:
  store i32 1, ptr %sret, align 4, !dbg !194
  %t262 = getelementptr inbounds i8, ptr %t257, i64 8, !dbg !194
  %t263 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !194
  call void @llvm.memcpy.p0.p0.i64(ptr %t263, ptr %t262, i64 32, i1 false), !dbg !194
  ret void, !dbg !194
L261:
  %t264 = getelementptr inbounds i8, ptr %t257, i64 8, !dbg !194
  call void @"tests.syntax.sign"(ptr %t232, i64 0), !dbg !203
  call void @"tests.syntax.sign"(ptr %t224, i64 3), !dbg !205
  call void @ignite_rt_text_concat(ptr %t221, ptr %t232, ptr %t224), !dbg !203
  %t265 = sub i64 0, 3, !dbg !208
  call void @"tests.syntax.sign"(ptr %t224, i64 %t265), !dbg !207
  call void @ignite.append_text(ptr %t221, ptr %t224), !dbg !203
  call void @"tests.syntax.show"(ptr %t266, ptr %t221), !dbg !202
  %t267 = load i32, ptr %t266, align 4, !dbg !202
  %t268 = icmp eq i32 %t267, 1, !dbg !202
  br i1 %t268, label %L269, label %L270, !dbg !202
L269:
  store i32 1, ptr %sret, align 4, !dbg !202
  %t271 = getelementptr inbounds i8, ptr %t266, i64 8, !dbg !202
  %t272 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !202
  call void @llvm.memcpy.p0.p0.i64(ptr %t272, ptr %t271, i64 32, i1 false), !dbg !202
  ret void, !dbg !202
L270:
  %t273 = getelementptr inbounds i8, ptr %t266, i64 8, !dbg !202
  store ptr @.str.32, ptr %t224, align 8, !dbg !212
  %t274 = getelementptr inbounds i8, ptr %t224, i64 8, !dbg !212
  store i64 6, ptr %t274, align 8, !dbg !212
  %t275 = getelementptr inbounds i8, ptr %t224, i64 16, !dbg !212
  store i64 6, ptr %t275, align 8, !dbg !212
  call void @"copy.Text"(ptr %t232, ptr %t224), !dbg !212
  %t276 = call i64 @"tests.syntax.count_spaces"(ptr %t232), !dbg !211
  call void @ignite_rt_int_to_text(ptr %t277, i64 %t276), !dbg !211
  call void @"tests.syntax.show"(ptr %t278, ptr %t277), !dbg !210
  %t279 = load i32, ptr %t278, align 4, !dbg !210
  %t280 = icmp eq i32 %t279, 1, !dbg !210
  br i1 %t280, label %L281, label %L282, !dbg !210
L281:
  store i32 1, ptr %sret, align 4, !dbg !210
  %t283 = getelementptr inbounds i8, ptr %t278, i64 8, !dbg !210
  %t284 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !210
  call void @llvm.memcpy.p0.p0.i64(ptr %t284, ptr %t283, i64 32, i1 false), !dbg !210
  ret void, !dbg !210
L282:
  %t285 = getelementptr inbounds i8, ptr %t278, i64 8, !dbg !210
  store i64 21, ptr %t286, align 8, !dbg !213
  %t287 = load i64, ptr %t286, align 8, !dbg !216
  %t288 = call i64 @"tests.syntax.twice"(i64 %t287), !dbg !216
  call void @ignite_rt_int_to_text(ptr %t289, i64 %t288), !dbg !216
  call void @"tests.syntax.show"(ptr %t290, ptr %t289), !dbg !215
  %t291 = load i32, ptr %t290, align 4, !dbg !215
  %t292 = icmp eq i32 %t291, 1, !dbg !215
  br i1 %t292, label %L293, label %L294, !dbg !215
L293:
  store i32 1, ptr %sret, align 4, !dbg !215
  %t295 = getelementptr inbounds i8, ptr %t290, i64 8, !dbg !215
  %t296 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !215
  call void @llvm.memcpy.p0.p0.i64(ptr %t296, ptr %t295, i64 32, i1 false), !dbg !215
  ret void, !dbg !215
L294:
  %t297 = getelementptr inbounds i8, ptr %t290, i64 8, !dbg !215
  store ptr @.str.33, ptr %t298, align 8, !dbg !218
  %t299 = getelementptr inbounds i8, ptr %t298, i64 8, !dbg !218
  store i64 3, ptr %t299, align 8, !dbg !218
  %t300 = getelementptr inbounds i8, ptr %t298, i64 16, !dbg !218
  store i64 3, ptr %t300, align 8, !dbg !218
  call void @"copy.Text"(ptr %t301, ptr %t298), !dbg !218
  call void @"tests.syntax.shout"(ptr %t302, ptr %t301), !dbg !218
  call void @"tests.syntax.show"(ptr %t303, ptr %t302), !dbg !217
  %t304 = load i32, ptr %t303, align 4, !dbg !217
  %t305 = icmp eq i32 %t304, 1, !dbg !217
  br i1 %t305, label %L306, label %L307, !dbg !217
L306:
  store i32 1, ptr %sret, align 4, !dbg !217
  %t308 = getelementptr inbounds i8, ptr %t303, i64 8, !dbg !217
  %t309 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !217
  call void @llvm.memcpy.p0.p0.i64(ptr %t309, ptr %t308, i64 32, i1 false), !dbg !217
  ret void, !dbg !217
L307:
  %t310 = getelementptr inbounds i8, ptr %t303, i64 8, !dbg !217
  store ptr @.str.34, ptr %t312, align 8, !dbg !221
  %t313 = getelementptr inbounds i8, ptr %t312, i64 8, !dbg !221
  store i64 1, ptr %t313, align 8, !dbg !221
  %t314 = getelementptr inbounds i8, ptr %t312, i64 16, !dbg !221
  store i64 1, ptr %t314, align 8, !dbg !221
  call void @"copy.Text"(ptr %t315, ptr %t312), !dbg !221
  call void @llvm.memcpy.p0.p0.i64(ptr %t311, ptr %t315, i64 24, i1 false), !dbg !220
  store ptr @"fnval.tests.syntax.shout", ptr %t316, align 8, !dbg !222
  %t317 = getelementptr inbounds i8, ptr %t316, i64 8, !dbg !222
  store ptr null, ptr %t317, align 8, !dbg !222
  %t318 = getelementptr inbounds i8, ptr %t311, i64 24, !dbg !220
  call void @llvm.memcpy.p0.p0.i64(ptr %t318, ptr %t316, i64 16, i1 false), !dbg !220
  call void @llvm.memcpy.p0.p0.i64(ptr %t319, ptr %t311, i64 40, i1 false), !dbg !219
  %t320 = getelementptr inbounds i8, ptr %t319, i64 24, !dbg !224
  %t321 = load ptr, ptr %t320, align 8, !dbg !224
  %t322 = getelementptr inbounds i8, ptr %t320, i64 8, !dbg !224
  %t323 = load ptr, ptr %t322, align 8, !dbg !224
  call void %t321(ptr %t323, ptr %t324, ptr %t319), !dbg !224
  call void @"tests.syntax.show"(ptr %t325, ptr %t324), !dbg !223
  %t326 = load i32, ptr %t325, align 4, !dbg !223
  %t327 = icmp eq i32 %t326, 1, !dbg !223
  br i1 %t327, label %L328, label %L329, !dbg !223
L328:
  store i32 1, ptr %sret, align 4, !dbg !223
  %t330 = getelementptr inbounds i8, ptr %t325, i64 8, !dbg !223
  %t331 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !223
  call void @llvm.memcpy.p0.p0.i64(ptr %t331, ptr %t330, i64 32, i1 false), !dbg !223
  ret void, !dbg !223
L329:
  %t332 = getelementptr inbounds i8, ptr %t325, i64 8, !dbg !223
  %t334 = call ptr @ignite.alloc(i64 24, i64 8), !dbg !228
  store ptr @.str.35, ptr %t335, align 8, !dbg !229
  %t336 = getelementptr inbounds i8, ptr %t335, i64 8, !dbg !229
  store i64 1, ptr %t336, align 8, !dbg !229
  %t337 = getelementptr inbounds i8, ptr %t335, i64 16, !dbg !229
  store i64 1, ptr %t337, align 8, !dbg !229
  call void @"copy.Text"(ptr %t338, ptr %t335), !dbg !229
  call void @llvm.memcpy.p0.p0.i64(ptr %t334, ptr %t338, i64 24, i1 false), !dbg !228
  store ptr %t334, ptr %t339, align 8, !dbg !228
  %t340 = getelementptr inbounds i8, ptr %t339, i64 8, !dbg !228
  store i64 1, ptr %t340, align 8, !dbg !228
  %t341 = getelementptr inbounds i8, ptr %t339, i64 16, !dbg !228
  store i64 1, ptr %t341, align 8, !dbg !228
  call void @llvm.memcpy.p0.p0.i64(ptr %t333, ptr %t339, i64 24, i1 false), !dbg !227
  call void @"tests.syntax.Reply.action"(ptr %t342), !dbg !227
  %t343 = getelementptr inbounds i8, ptr %t333, i64 24, !dbg !227
  call void @llvm.memcpy.p0.p0.i64(ptr %t343, ptr %t342, i64 8, i1 false), !dbg !227
  %t344 = call i64 @"tests.syntax.Reply.code"(), !dbg !227
  %t345 = getelementptr inbounds i8, ptr %t333, i64 32, !dbg !227
  store i64 %t344, ptr %t345, align 8, !dbg !227
  call void @llvm.memcpy.p0.p0.i64(ptr %t346, ptr %t333, i64 40, i1 false), !dbg !226
  store ptr @.str.36, ptr %t347, align 8, !dbg !231
  %t348 = getelementptr inbounds i8, ptr %t347, i64 8, !dbg !231
  store i64 5, ptr %t348, align 8, !dbg !231
  %t349 = getelementptr inbounds i8, ptr %t347, i64 16, !dbg !231
  store i64 5, ptr %t349, align 8, !dbg !231
  call void @"copy.Text"(ptr %t350, ptr %t347), !dbg !231
  %t351 = getelementptr inbounds i8, ptr %t346, i64 32, !dbg !232
  %t352 = load i64, ptr %t351, align 8, !dbg !232
  call void @ignite_rt_int_to_text(ptr %t353, i64 %t352), !dbg !232
  call void @ignite_rt_text_concat(ptr %t354, ptr %t350, ptr %t353), !dbg !231
  call void @"tests.syntax.show"(ptr %t355, ptr %t354), !dbg !230
  %t356 = load i32, ptr %t355, align 4, !dbg !230
  %t357 = icmp eq i32 %t356, 1, !dbg !230
  br i1 %t357, label %L358, label %L359, !dbg !230
L358:
  store i32 1, ptr %sret, align 4, !dbg !230
  %t360 = getelementptr inbounds i8, ptr %t355, i64 8, !dbg !230
  %t361 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !230
  call void @llvm.memcpy.p0.p0.i64(ptr %t361, ptr %t360, i64 32, i1 false), !dbg !230
  ret void, !dbg !230
L359:
  %t362 = getelementptr inbounds i8, ptr %t355, i64 8, !dbg !230
  %t363 = getelementptr inbounds i8, ptr %t346, i64 24, !dbg !234
  %t366 = load i32, ptr %t363, align 4, !dbg !233
  %t367 = icmp eq i32 %t366, 0, !dbg !233
  br i1 %t367, label %L368, label %L365, !dbg !233
L368:
  store ptr @.str.37, ptr %t353, align 8, !dbg !236
  %t369 = getelementptr inbounds i8, ptr %t353, i64 8, !dbg !236
  store i64 8, ptr %t369, align 8, !dbg !236
  %t370 = getelementptr inbounds i8, ptr %t353, i64 16, !dbg !236
  store i64 8, ptr %t370, align 8, !dbg !236
  call void @"copy.Text"(ptr %t350, ptr %t353), !dbg !236
  call void @"tests.syntax.show"(ptr %t371, ptr %t350), !dbg !235
  %t372 = load i32, ptr %t371, align 4, !dbg !235
  %t373 = icmp eq i32 %t372, 1, !dbg !235
  br i1 %t373, label %L374, label %L375, !dbg !235
L374:
  store i32 1, ptr %sret, align 4, !dbg !235
  %t376 = getelementptr inbounds i8, ptr %t371, i64 8, !dbg !235
  %t377 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !235
  call void @llvm.memcpy.p0.p0.i64(ptr %t377, ptr %t376, i64 32, i1 false), !dbg !235
  ret void, !dbg !235
L375:
  %t378 = getelementptr inbounds i8, ptr %t371, i64 8, !dbg !235
  br label %L364, !dbg !233
L365:
  %t380 = load i32, ptr %t363, align 4, !dbg !233
  %t381 = icmp eq i32 %t380, 1, !dbg !233
  br i1 %t381, label %L382, label %L379, !dbg !233
L382:
  store ptr @.str.38, ptr %t347, align 8, !dbg !238
  %t383 = getelementptr inbounds i8, ptr %t347, i64 8, !dbg !238
  store i64 5, ptr %t383, align 8, !dbg !238
  %t384 = getelementptr inbounds i8, ptr %t347, i64 16, !dbg !238
  store i64 5, ptr %t384, align 8, !dbg !238
  call void @"copy.Text"(ptr %t385, ptr %t347), !dbg !238
  call void @"tests.syntax.show"(ptr %t386, ptr %t385), !dbg !237
  %t387 = load i32, ptr %t386, align 4, !dbg !237
  %t388 = icmp eq i32 %t387, 1, !dbg !237
  br i1 %t388, label %L389, label %L390, !dbg !237
L389:
  store i32 1, ptr %sret, align 4, !dbg !237
  %t391 = getelementptr inbounds i8, ptr %t386, i64 8, !dbg !237
  %t392 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !237
  call void @llvm.memcpy.p0.p0.i64(ptr %t392, ptr %t391, i64 32, i1 false), !dbg !237
  ret void, !dbg !237
L390:
  %t393 = getelementptr inbounds i8, ptr %t386, i64 8, !dbg !237
  br label %L364, !dbg !233
L379:
  store ptr @.str.0, ptr %t394, align 8, !dbg !233
  %t395 = getelementptr inbounds i8, ptr %t394, i64 8, !dbg !233
  store i64 26, ptr %t395, align 8, !dbg !233
  %t396 = getelementptr inbounds i8, ptr %t394, i64 16, !dbg !233
  store i64 26, ptr %t396, align 8, !dbg !233
  call void @ignite_rt_fail_match(ptr %t394, i64 140, i64 5), !dbg !233
  unreachable, !dbg !233
L364:
  %t398 = zext i1 true to i8, !dbg !239
  store i8 %t398, ptr %t397, align 1, !dbg !239
  %t399 = call i8 @"tests.syntax.SPACE"(), !dbg !242
  store i8 %t399, ptr %t400, align 1, !dbg !241
  %t401 = load i8, ptr %t397, align 1, !dbg !244
  %t402 = trunc i8 %t401 to i1, !dbg !244
  store ptr @.str.30, ptr %t403, align 8, !dbg !244
  %t404 = getelementptr inbounds i8, ptr %t403, i64 8, !dbg !244
  store i64 4, ptr %t404, align 8, !dbg !244
  %t405 = getelementptr inbounds i8, ptr %t403, i64 16, !dbg !244
  store i64 4, ptr %t405, align 8, !dbg !244
  store ptr @.str.31, ptr %t406, align 8, !dbg !244
  %t407 = getelementptr inbounds i8, ptr %t406, i64 8, !dbg !244
  store i64 5, ptr %t407, align 8, !dbg !244
  %t408 = getelementptr inbounds i8, ptr %t406, i64 16, !dbg !244
  store i64 5, ptr %t408, align 8, !dbg !244
  %t409 = select i1 %t402, ptr %t403, ptr %t406, !dbg !244
  call void @"copy.Text"(ptr %t410, ptr %t409), !dbg !244
  store ptr @.str.16, ptr %t411, align 8, !dbg !245
  %t412 = getelementptr inbounds i8, ptr %t411, i64 8, !dbg !245
  store i64 1, ptr %t412, align 8, !dbg !245
  %t413 = getelementptr inbounds i8, ptr %t411, i64 16, !dbg !245
  store i64 1, ptr %t413, align 8, !dbg !245
  call void @"copy.Text"(ptr %t414, ptr %t411), !dbg !245
  call void @ignite_rt_text_concat(ptr %t415, ptr %t410, ptr %t414), !dbg !244
  %t416 = load i8, ptr %t400, align 1, !dbg !246
  %t417 = zext i8 %t416 to i64, !dbg !246
  call void @ignite_rt_int_to_text(ptr %t414, i64 %t417), !dbg !246
  call void @ignite.append_text(ptr %t415, ptr %t414), !dbg !244
  call void @ignite.append(ptr %t415, ptr @.str.16, i64 1), !dbg !244
  store ptr @.str.39, ptr %t414, align 8, !dbg !247
  %t418 = getelementptr inbounds i8, ptr %t414, i64 8, !dbg !247
  store i64 1, ptr %t418, align 8, !dbg !247
  %t419 = getelementptr inbounds i8, ptr %t414, i64 16, !dbg !247
  store i64 1, ptr %t419, align 8, !dbg !247
  call void @"copy.Text"(ptr %t411, ptr %t414), !dbg !247
  call void @ignite.append_text(ptr %t415, ptr %t411), !dbg !244
  call void @"tests.syntax.show"(ptr %t420, ptr %t415), !dbg !243
  %t421 = load i32, ptr %t420, align 4, !dbg !243
  %t422 = icmp eq i32 %t421, 1, !dbg !243
  br i1 %t422, label %L423, label %L424, !dbg !243
L423:
  store i32 1, ptr %sret, align 4, !dbg !243
  %t425 = getelementptr inbounds i8, ptr %t420, i64 8, !dbg !243
  %t426 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !243
  call void @llvm.memcpy.p0.p0.i64(ptr %t426, ptr %t425, i64 32, i1 false), !dbg !243
  ret void, !dbg !243
L424:
  %t427 = getelementptr inbounds i8, ptr %t420, i64 8, !dbg !243
  store i64 0, ptr %t428, align 8, !dbg !249
  store ptr @ignite.empty, ptr %t411, align 8, !dbg !251
  %t429 = getelementptr inbounds i8, ptr %t411, i64 8, !dbg !251
  store i64 0, ptr %t429, align 8, !dbg !251
  %t430 = getelementptr inbounds i8, ptr %t411, i64 16, !dbg !251
  store i64 0, ptr %t430, align 8, !dbg !251
  call void @"copy.Text"(ptr %t414, ptr %t411), !dbg !251
  %t431 = getelementptr inbounds i8, ptr %t428, i64 8, !dbg !249
  call void @llvm.memcpy.p0.p0.i64(ptr %t431, ptr %t414, i64 24, i1 false), !dbg !249
  call void @llvm.memcpy.p0.p0.i64(ptr %t432, ptr %t428, i64 32, i1 false), !dbg !248
  store i64 0, ptr %t433, align 8, !dbg !252
  %t435 = call i64 @ignite.arena_mark(), !dbg !254
  store i64 0, ptr %t436, align 8, !dbg !254
  br label %L437
L437:
  %t441 = load i64, ptr %t433, align 8, !dbg !255
  %t442 = icmp slt i64 %t441, 3000, !dbg !255
  br i1 %t442, label %L438, label %L440, !dbg !254
L438:
  call void @"copy.(Int,Text)"(ptr %t443, ptr %t432), !dbg !257
  %t446 = load i64, ptr %t443, align 8, !dbg !257
  store i64 %t446, ptr %t447, align 8, !dbg !257
  %t448 = getelementptr inbounds i8, ptr %t443, i64 8, !dbg !257
  br label %L444, !dbg !257
L445:
  store ptr @.str.0, ptr %t410, align 8, !dbg !257
  %t449 = getelementptr inbounds i8, ptr %t410, i64 8, !dbg !257
  store i64 26, ptr %t449, align 8, !dbg !257
  %t450 = getelementptr inbounds i8, ptr %t410, i64 16, !dbg !257
  store i64 26, ptr %t450, align 8, !dbg !257
  call void @ignite_rt_fail_match(ptr %t410, i64 150, i64 11), !dbg !257
  unreachable, !dbg !257
L444:
  %t452 = load i64, ptr %t447, align 8, !dbg !261
  %t453 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t452, i64 1), !dbg !261
  %t454 = extractvalue { i64, i1 } %t453, 1, !dbg !261
  br i1 %t454, label %L455, label %L456, !dbg !261
L455:
  store ptr @.str.0, ptr %t406, align 8, !dbg !261
  %t457 = getelementptr inbounds i8, ptr %t406, i64 8, !dbg !261
  store i64 26, ptr %t457, align 8, !dbg !261
  %t458 = getelementptr inbounds i8, ptr %t406, i64 16, !dbg !261
  store i64 26, ptr %t458, align 8, !dbg !261
  store ptr @.str.1, ptr %t403, align 8, !dbg !261
  %t459 = getelementptr inbounds i8, ptr %t403, i64 8, !dbg !261
  store i64 1, ptr %t459, align 8, !dbg !261
  %t460 = getelementptr inbounds i8, ptr %t403, i64 16, !dbg !261
  store i64 1, ptr %t460, align 8, !dbg !261
  call void @ignite_rt_fail_arith(ptr %t403, ptr %t406, i64 151, i64 15, i64 %t452, i64 1), !dbg !261
  unreachable, !dbg !261
L456:
  %t461 = extractvalue { i64, i1 } %t453, 0, !dbg !261
  store i64 %t461, ptr %t451, align 8, !dbg !260
  store ptr @.str.26, ptr %t403, align 8, !dbg !264
  %t462 = getelementptr inbounds i8, ptr %t403, i64 8, !dbg !264
  store i64 1, ptr %t462, align 8, !dbg !264
  %t463 = getelementptr inbounds i8, ptr %t403, i64 16, !dbg !264
  store i64 1, ptr %t463, align 8, !dbg !264
  call void @"copy.Text"(ptr %t406, ptr %t403), !dbg !264
  call void @ignite_rt_text_concat(ptr %t464, ptr %t448, ptr %t406), !dbg !263
  %t465 = getelementptr inbounds i8, ptr %t451, i64 8, !dbg !260
  call void @llvm.memcpy.p0.p0.i64(ptr %t465, ptr %t464, i64 24, i1 false), !dbg !260
  call void @llvm.memcpy.p0.p0.i64(ptr %t432, ptr %t451, i64 32, i1 false), !dbg !259
  %t466 = load i64, ptr %t433, align 8, !dbg !266
  %t467 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t466, i64 1), !dbg !266
  %t468 = extractvalue { i64, i1 } %t467, 1, !dbg !266
  br i1 %t468, label %L469, label %L470, !dbg !266
L469:
  store ptr @.str.0, ptr %t406, align 8, !dbg !266
  %t471 = getelementptr inbounds i8, ptr %t406, i64 8, !dbg !266
  store i64 26, ptr %t471, align 8, !dbg !266
  %t472 = getelementptr inbounds i8, ptr %t406, i64 16, !dbg !266
  store i64 26, ptr %t472, align 8, !dbg !266
  store ptr @.str.1, ptr %t403, align 8, !dbg !266
  %t473 = getelementptr inbounds i8, ptr %t403, i64 8, !dbg !266
  store i64 1, ptr %t473, align 8, !dbg !266
  %t474 = getelementptr inbounds i8, ptr %t403, i64 16, !dbg !266
  store i64 1, ptr %t474, align 8, !dbg !266
  call void @ignite_rt_fail_arith(ptr %t403, ptr %t406, i64 152, i64 11, i64 %t466, i64 1), !dbg !266
  unreachable, !dbg !266
L470:
  %t475 = extractvalue { i64, i1 } %t467, 0, !dbg !266
  store i64 %t475, ptr %t433, align 8, !dbg !265
  br label %L439
L439:
  %t476 = load i64, ptr %t436, align 8, !dbg !254
  %t477 = call i1 @ignite.arena_due(i64 %t435, i64 %t476), !dbg !254
  br i1 %t477, label %L478, label %L437, !dbg !254
L478:
  call void @ignite_rt_arena_scratch(i8 1), !dbg !254
  call void @"copy.(Int,Text)"(ptr %t434, ptr %t432), !dbg !254
  call void @ignite_rt_arena_scratch(i8 0), !dbg !254
  call void @ignite_rt_arena_release(i64 %t435), !dbg !254
  call void @"copy.(Int,Text)"(ptr %t432, ptr %t434), !dbg !254
  call void @ignite_rt_arena_scratch_clear(), !dbg !254
  %t479 = call i64 @ignite.arena_mark(), !dbg !254
  %t480 = sub i64 %t479, %t435, !dbg !254
  store i64 %t480, ptr %t436, align 8, !dbg !254
  br label %L437, !dbg !254
L440:
  call void @"copy.(Int,Text)"(ptr %t481, ptr %t432), !dbg !268
  %t484 = load i64, ptr %t481, align 8, !dbg !268
  store i64 %t484, ptr %t485, align 8, !dbg !268
  %t486 = getelementptr inbounds i8, ptr %t481, i64 8, !dbg !268
  br label %L482, !dbg !268
L483:
  store ptr @.str.0, ptr %t403, align 8, !dbg !268
  %t487 = getelementptr inbounds i8, ptr %t403, i64 8, !dbg !268
  store i64 26, ptr %t487, align 8, !dbg !268
  %t488 = getelementptr inbounds i8, ptr %t403, i64 16, !dbg !268
  store i64 26, ptr %t488, align 8, !dbg !268
  call void @ignite_rt_fail_match(ptr %t403, i64 154, i64 9), !dbg !268
  unreachable, !dbg !268
L482:
  %t489 = getelementptr inbounds i8, ptr %t486, i64 8, !dbg !271
  %t490 = load i64, ptr %t489, align 8, !dbg !271
  store i64 %t490, ptr %t491, align 8, !dbg !270
  %t492 = load i64, ptr %t485, align 8, !dbg !274
  call void @ignite_rt_int_to_text(ptr %t406, i64 %t492), !dbg !274
  store ptr @.str.16, ptr %t493, align 8, !dbg !273
  %t494 = getelementptr inbounds i8, ptr %t493, i64 8, !dbg !273
  store i64 1, ptr %t494, align 8, !dbg !273
  %t495 = getelementptr inbounds i8, ptr %t493, i64 16, !dbg !273
  store i64 1, ptr %t495, align 8, !dbg !273
  call void @"copy.Text"(ptr %t496, ptr %t493), !dbg !273
  call void @ignite_rt_text_concat(ptr %t497, ptr %t406, ptr %t496), !dbg !273
  %t498 = load i64, ptr %t491, align 8, !dbg !275
  call void @ignite_rt_int_to_text(ptr %t496, i64 %t498), !dbg !275
  call void @ignite.append_text(ptr %t497, ptr %t496), !dbg !273
  call void @"tests.syntax.show"(ptr %t499, ptr %t497), !dbg !272
  %t500 = load i32, ptr %t499, align 4, !dbg !272
  %t501 = icmp eq i32 %t500, 1, !dbg !272
  br i1 %t501, label %L502, label %L503, !dbg !272
L502:
  store i32 1, ptr %sret, align 4, !dbg !272
  %t504 = getelementptr inbounds i8, ptr %t499, i64 8, !dbg !272
  %t505 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !272
  call void @llvm.memcpy.p0.p0.i64(ptr %t505, ptr %t504, i64 32, i1 false), !dbg !272
  ret void, !dbg !272
L503:
  %t506 = getelementptr inbounds i8, ptr %t499, i64 8, !dbg !272
  store i64 1, ptr %t507, align 8, !dbg !277
  store ptr @.str.18, ptr %t496, align 8, !dbg !279
  %t508 = getelementptr inbounds i8, ptr %t496, i64 8, !dbg !279
  store i64 1, ptr %t508, align 8, !dbg !279
  %t509 = getelementptr inbounds i8, ptr %t496, i64 16, !dbg !279
  store i64 1, ptr %t509, align 8, !dbg !279
  call void @"copy.Text"(ptr %t493, ptr %t496), !dbg !279
  %t510 = getelementptr inbounds i8, ptr %t507, i64 8, !dbg !277
  call void @llvm.memcpy.p0.p0.i64(ptr %t510, ptr %t493, i64 24, i1 false), !dbg !277
  call void @llvm.memcpy.p0.p0.i64(ptr %t511, ptr %t507, i64 32, i1 false), !dbg !276
  call void @"copy.(Int,Text)"(ptr %t512, ptr %t511), !dbg !280
  %t515 = load i64, ptr %t512, align 8, !dbg !280
  store i64 %t515, ptr %t516, align 8, !dbg !280
  %t517 = getelementptr inbounds i8, ptr %t512, i64 8, !dbg !280
  br label %L513, !dbg !280
L514:
  store ptr @.str.0, ptr %t406, align 8, !dbg !280
  %t518 = getelementptr inbounds i8, ptr %t406, i64 8, !dbg !280
  store i64 26, ptr %t518, align 8, !dbg !280
  %t519 = getelementptr inbounds i8, ptr %t406, i64 16, !dbg !280
  store i64 26, ptr %t519, align 8, !dbg !280
  call void @ignite_rt_fail_match(ptr %t406, i64 158, i64 9), !dbg !280
  unreachable, !dbg !280
L513:
  %t520 = load i64, ptr %t516, align 8, !dbg !283
  %t521 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %t520, i64 1), !dbg !283
  %t522 = extractvalue { i64, i1 } %t521, 1, !dbg !283
  br i1 %t522, label %L523, label %L524, !dbg !283
L523:
  store ptr @.str.0, ptr %t525, align 8, !dbg !283
  %t526 = getelementptr inbounds i8, ptr %t525, i64 8, !dbg !283
  store i64 26, ptr %t526, align 8, !dbg !283
  %t527 = getelementptr inbounds i8, ptr %t525, i64 16, !dbg !283
  store i64 26, ptr %t527, align 8, !dbg !283
  store ptr @.str.1, ptr %t528, align 8, !dbg !283
  %t529 = getelementptr inbounds i8, ptr %t528, i64 8, !dbg !283
  store i64 1, ptr %t529, align 8, !dbg !283
  %t530 = getelementptr inbounds i8, ptr %t528, i64 16, !dbg !283
  store i64 1, ptr %t530, align 8, !dbg !283
  call void @ignite_rt_fail_arith(ptr %t528, ptr %t525, i64 159, i64 9, i64 %t520, i64 1), !dbg !283
  unreachable, !dbg !283
L524:
  %t531 = extractvalue { i64, i1 } %t521, 0, !dbg !283
  store i64 %t531, ptr %t516, align 8, !dbg !282
  store ptr @.str.28, ptr %t528, align 8, !dbg !287
  %t532 = getelementptr inbounds i8, ptr %t528, i64 8, !dbg !287
  store i64 1, ptr %t532, align 8, !dbg !287
  %t533 = getelementptr inbounds i8, ptr %t528, i64 16, !dbg !287
  store i64 1, ptr %t533, align 8, !dbg !287
  call void @"copy.Text"(ptr %t525, ptr %t528), !dbg !287
  call void @ignite_rt_text_concat(ptr %t534, ptr %t517, ptr %t525), !dbg !286
  call void @llvm.memcpy.p0.p0.i64(ptr %t517, ptr %t534, i64 24, i1 false), !dbg !285
  %t535 = load i64, ptr %t516, align 8, !dbg !290
  call void @ignite_rt_int_to_text(ptr %t525, i64 %t535), !dbg !290
  call void @"copy.Text"(ptr %t528, ptr %t517), !dbg !291
  call void @ignite_rt_text_concat(ptr %t536, ptr %t525, ptr %t528), !dbg !289
  call void @"tests.syntax.show"(ptr %t537, ptr %t536), !dbg !288
  %t538 = load i32, ptr %t537, align 4, !dbg !288
  %t539 = icmp eq i32 %t538, 1, !dbg !288
  br i1 %t539, label %L540, label %L541, !dbg !288
L540:
  store i32 1, ptr %sret, align 4, !dbg !288
  %t542 = getelementptr inbounds i8, ptr %t537, i64 8, !dbg !288
  %t543 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !288
  call void @llvm.memcpy.p0.p0.i64(ptr %t543, ptr %t542, i64 32, i1 false), !dbg !288
  ret void, !dbg !288
L541:
  %t544 = getelementptr inbounds i8, ptr %t537, i64 8, !dbg !288
  %t545 = call i64 @"tests.syntax.twice"(i64 1), !dbg !293
  br label %L546, !dbg !292
L547:
  store ptr @.str.0, ptr %t528, align 8, !dbg !292
  %t548 = getelementptr inbounds i8, ptr %t528, i64 8, !dbg !292
  store i64 26, ptr %t548, align 8, !dbg !292
  %t549 = getelementptr inbounds i8, ptr %t528, i64 16, !dbg !292
  store i64 26, ptr %t549, align 8, !dbg !292
  call void @ignite_rt_fail_match(ptr %t528, i64 162, i64 9), !dbg !292
  unreachable, !dbg !292
L546:
  %t550 = call i64 @"tests.syntax.twice"(i64 2), !dbg !296
  br label %L551, !dbg !295
L552:
  store ptr @.str.0, ptr %t525, align 8, !dbg !295
  %t553 = getelementptr inbounds i8, ptr %t525, i64 8, !dbg !295
  store i64 26, ptr %t553, align 8, !dbg !295
  %t554 = getelementptr inbounds i8, ptr %t525, i64 16, !dbg !295
  store i64 26, ptr %t554, align 8, !dbg !295
  call void @ignite_rt_fail_match(ptr %t525, i64 163, i64 5), !dbg !295
  unreachable, !dbg !295
L551:
  store i32 0, ptr %t555, align 4, !dbg !299
  call void @llvm.memcpy.p0.p0.i64(ptr %t556, ptr %t555, i64 8, i1 false), !dbg !298
  %t559 = load i32, ptr %t556, align 4, !dbg !300
  %t560 = icmp eq i32 %t559, 0, !dbg !300
  br i1 %t560, label %L561, label %L558, !dbg !300
L561:
  store ptr @.str.40, ptr %t562, align 8, !dbg !303
  %t563 = getelementptr inbounds i8, ptr %t562, i64 8, !dbg !303
  store i64 9, ptr %t563, align 8, !dbg !303
  %t564 = getelementptr inbounds i8, ptr %t562, i64 16, !dbg !303
  store i64 9, ptr %t564, align 8, !dbg !303
  call void @"copy.Text"(ptr %t565, ptr %t562), !dbg !303
  call void @"tests.syntax.show"(ptr %t566, ptr %t565), !dbg !302
  %t567 = load i32, ptr %t566, align 4, !dbg !302
  %t568 = icmp eq i32 %t567, 1, !dbg !302
  br i1 %t568, label %L569, label %L570, !dbg !302
L569:
  store i32 1, ptr %sret, align 4, !dbg !302
  %t571 = getelementptr inbounds i8, ptr %t566, i64 8, !dbg !302
  %t572 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !302
  call void @llvm.memcpy.p0.p0.i64(ptr %t572, ptr %t571, i64 32, i1 false), !dbg !302
  ret void, !dbg !302
L570:
  %t573 = getelementptr inbounds i8, ptr %t566, i64 8, !dbg !302
  br label %L557, !dbg !300
L558:
  %t575 = load i32, ptr %t556, align 4, !dbg !300
  %t576 = icmp eq i32 %t575, 1, !dbg !300
  br i1 %t576, label %L577, label %L574, !dbg !300
L577:
  store ptr @.str.41, ptr %t578, align 8, !dbg !305
  %t579 = getelementptr inbounds i8, ptr %t578, i64 8, !dbg !305
  store i64 8, ptr %t579, align 8, !dbg !305
  %t580 = getelementptr inbounds i8, ptr %t578, i64 16, !dbg !305
  store i64 8, ptr %t580, align 8, !dbg !305
  call void @"copy.Text"(ptr %t581, ptr %t578), !dbg !305
  call void @"tests.syntax.show"(ptr %t582, ptr %t581), !dbg !304
  %t583 = load i32, ptr %t582, align 4, !dbg !304
  %t584 = icmp eq i32 %t583, 1, !dbg !304
  br i1 %t584, label %L585, label %L586, !dbg !304
L585:
  store i32 1, ptr %sret, align 4, !dbg !304
  %t587 = getelementptr inbounds i8, ptr %t582, i64 8, !dbg !304
  %t588 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !304
  call void @llvm.memcpy.p0.p0.i64(ptr %t588, ptr %t587, i64 32, i1 false), !dbg !304
  ret void, !dbg !304
L586:
  %t589 = getelementptr inbounds i8, ptr %t582, i64 8, !dbg !304
  br label %L557, !dbg !300
L574:
  store ptr @.str.0, ptr %t590, align 8, !dbg !300
  %t591 = getelementptr inbounds i8, ptr %t590, i64 8, !dbg !300
  store i64 26, ptr %t591, align 8, !dbg !300
  %t592 = getelementptr inbounds i8, ptr %t590, i64 16, !dbg !300
  store i64 26, ptr %t592, align 8, !dbg !300
  call void @ignite_rt_fail_match(ptr %t590, i64 165, i64 5), !dbg !300
  unreachable, !dbg !300
L557:
  call void @"tests.syntax.lower_case"(ptr %t593, i64 3), !dbg !307
  store ptr @.str.16, ptr %t594, align 8, !dbg !309
  %t595 = getelementptr inbounds i8, ptr %t594, i64 8, !dbg !309
  store i64 1, ptr %t595, align 8, !dbg !309
  %t596 = getelementptr inbounds i8, ptr %t594, i64 16, !dbg !309
  store i64 1, ptr %t596, align 8, !dbg !309
  call void @"copy.Text"(ptr %t597, ptr %t594), !dbg !309
  call void @ignite_rt_text_concat(ptr %t598, ptr %t593, ptr %t597), !dbg !307
  call void @"tests.syntax.lower_case"(ptr %t597, i64 4), !dbg !310
  call void @ignite.append_text(ptr %t598, ptr %t597), !dbg !307
  call void @"tests.syntax.show"(ptr %t599, ptr %t598), !dbg !306
  %t600 = load i32, ptr %t599, align 4, !dbg !306
  %t601 = icmp eq i32 %t600, 1, !dbg !306
  br i1 %t601, label %L602, label %L603, !dbg !306
L602:
  store i32 1, ptr %sret, align 4, !dbg !306
  %t604 = getelementptr inbounds i8, ptr %t599, i64 8, !dbg !306
  %t605 = getelementptr inbounds i8, ptr %sret, i64 8, !dbg !306
  call void @llvm.memcpy.p0.p0.i64(ptr %t605, ptr %t604, i64 32, i1 false), !dbg !306
  ret void, !dbg !306
L603:
  %t606 = getelementptr inbounds i8, ptr %t599, i64 8, !dbg !306
  store i32 0, ptr %t607, align 4, !dbg !312
  %t608 = getelementptr inbounds i8, ptr %t607, i64 8, !dbg !312
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t607, i64 40, i1 false), !dbg !118
  ret void, !dbg !118
}

define void @"tests.syntax.Runner.main"(ptr %sret, ptr %p0) !dbg !314 {
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
  %t1 = getelementptr inbounds i8, ptr %p0, i64 24, !dbg !320
  %t2 = load i64, ptr %t1, align 8, !dbg !320
  %t6 = call i32 @ignite_rt_tcp_connect(ptr %p0, i64 %t2, ptr %t4, ptr %t5), !dbg !318
  %t7 = icmp ne i32 %t6, 0, !dbg !318
  br i1 %t7, label %L8, label %L9, !dbg !318
L8:
  store i32 1, ptr %t3, align 4, !dbg !318
  %t11 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !318
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t5, i64 32, i1 false), !dbg !318
  br label %L10, !dbg !318
L9:
  store i32 0, ptr %t3, align 4, !dbg !318
  %t12 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !318
  call void @llvm.memcpy.p0.p0.i64(ptr %t12, ptr %t4, i64 8, i1 false), !dbg !318
  br label %L10, !dbg !318
L10:
  %t16 = load i32, ptr %t3, align 4, !dbg !317
  %t17 = icmp eq i32 %t16, 0, !dbg !317
  br i1 %t17, label %L18, label %L15, !dbg !317
L18:
  %t19 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !317
  %t20 = load i64, ptr %t19, align 8, !dbg !317
  store i64 %t20, ptr %t21, align 8, !dbg !317
  %t22 = load i64, ptr %t21, align 8, !dbg !321
  store i64 %t22, ptr %t13, align 8, !dbg !317
  br label %L14, !dbg !317
L15:
  %t24 = load i32, ptr %t3, align 4, !dbg !317
  %t25 = icmp eq i32 %t24, 1, !dbg !317
  br i1 %t25, label %L26, label %L23, !dbg !317
L26:
  %t27 = getelementptr inbounds i8, ptr %t3, i64 8, !dbg !317
  call void @"copy.IoError"(ptr %t28, ptr %t27), !dbg !324
  %t29 = trunc i64 3 to i32, !dbg !324
  %t30 = getelementptr inbounds i8, ptr %t28, i64 24, !dbg !324
  store i32 %t29, ptr %t30, align 4, !dbg !324
  store i32 1, ptr %t31, align 4, !dbg !323
  %t32 = getelementptr inbounds i8, ptr %t31, i64 8, !dbg !323
  call void @llvm.memcpy.p0.p0.i64(ptr %t32, ptr %t28, i64 32, i1 false), !dbg !323
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t31, i64 40, i1 false), !dbg !322
  ret void, !dbg !322
L33:
  br label %L14, !dbg !317
L23:
  store ptr @.str.0, ptr %t34, align 8, !dbg !317
  %t35 = getelementptr inbounds i8, ptr %t34, i64 8, !dbg !317
  store i64 26, ptr %t35, align 8, !dbg !317
  %t36 = getelementptr inbounds i8, ptr %t34, i64 16, !dbg !317
  store i64 26, ptr %t36, align 8, !dbg !317
  call void @ignite_rt_fail_match(ptr %t34, i64 180, i64 16), !dbg !317
  unreachable, !dbg !317
L14:
  %t37 = load i64, ptr %t13, align 8, !dbg !317
  store i64 %t37, ptr %t38, align 8, !dbg !316
  %t39 = load i64, ptr %t38, align 8, !dbg !328
  call void @"tests.syntax.Syntax.connect"(ptr %t40, i64 %t39), !dbg !327
  %t44 = load i32, ptr %t40, align 4, !dbg !326
  %t45 = icmp eq i32 %t44, 0, !dbg !326
  br i1 %t45, label %L46, label %L43, !dbg !326
L46:
  %t47 = getelementptr inbounds i8, ptr %t40, i64 8, !dbg !326
  store i32 0, ptr %t48, align 4, !dbg !329
  %t49 = getelementptr inbounds i8, ptr %t48, i64 8, !dbg !329
  call void @llvm.memcpy.p0.p0.i64(ptr %t41, ptr %t48, i64 40, i1 false), !dbg !326
  br label %L42, !dbg !326
L43:
  %t51 = load i32, ptr %t40, align 4, !dbg !326
  %t52 = icmp eq i32 %t51, 1, !dbg !326
  br i1 %t52, label %L53, label %L50, !dbg !326
L53:
  %t54 = getelementptr inbounds i8, ptr %t40, i64 8, !dbg !326
  call void @"copy.IoError"(ptr %t55, ptr %t54), !dbg !332
  %t56 = trunc i64 4 to i32, !dbg !332
  %t57 = getelementptr inbounds i8, ptr %t55, i64 24, !dbg !332
  store i32 %t56, ptr %t57, align 4, !dbg !332
  store i32 1, ptr %t58, align 4, !dbg !331
  %t59 = getelementptr inbounds i8, ptr %t58, i64 8, !dbg !331
  call void @llvm.memcpy.p0.p0.i64(ptr %t59, ptr %t55, i64 32, i1 false), !dbg !331
  call void @llvm.memcpy.p0.p0.i64(ptr %t41, ptr %t58, i64 40, i1 false), !dbg !326
  br label %L42, !dbg !326
L50:
  store ptr @.str.0, ptr %t60, align 8, !dbg !326
  %t61 = getelementptr inbounds i8, ptr %t60, i64 8, !dbg !326
  store i64 26, ptr %t61, align 8, !dbg !326
  %t62 = getelementptr inbounds i8, ptr %t60, i64 16, !dbg !326
  store i64 26, ptr %t62, align 8, !dbg !326
  call void @ignite_rt_fail_match(ptr %t60, i64 184, i64 3), !dbg !326
  unreachable, !dbg !326
L42:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t41, i64 40, i1 false), !dbg !315
  ret void, !dbg !315
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
!llvm.module.flags = !{!334, !335}
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
!19 = !DILocation(line: 28, column: 33, scope: !17)
!20 = distinct !DISubprogram(name: "tests.syntax.twice", linkageName: "tests.syntax.twice", scope: !3, file: !3, line: 30, type: !2, scopeLine: 30, spFlags: DISPFlagDefinition, unit: !1)
!21 = !DILocation(line: 30, column: 27, scope: !20)
!22 = !DILocation(line: 30, column: 31, scope: !20)
!23 = distinct !DISubprogram(name: "tests.syntax.lower_case", linkageName: "tests.syntax.lower_case", scope: !3, file: !3, line: 32, type: !2, scopeLine: 32, spFlags: DISPFlagDefinition, unit: !1)
!24 = !DILocation(line: 32, column: 31, scope: !23)
!25 = !DILocation(line: 33, column: 3, scope: !23)
!26 = !DILocation(line: 33, column: 9, scope: !23)
!27 = !DILocation(line: 34, column: 15, scope: !23)
!28 = !DILocation(line: 35, column: 14, scope: !23)
!29 = !DILocation(line: 35, column: 22, scope: !23)
!30 = distinct !DISubprogram(name: "tests.syntax.swap", linkageName: "tests.syntax.swap", scope: !3, file: !3, line: 39, type: !2, scopeLine: 39, spFlags: DISPFlagDefinition, unit: !1)
!31 = !DILocation(line: 39, column: 40, scope: !30)
!32 = !DILocation(line: 40, column: 3, scope: !30)
!33 = !DILocation(line: 40, column: 16, scope: !30)
!34 = !DILocation(line: 41, column: 3, scope: !30)
!35 = !DILocation(line: 41, column: 4, scope: !30)
!36 = !DILocation(line: 41, column: 7, scope: !30)
!37 = distinct !DISubprogram(name: "tests.syntax.describe", linkageName: "tests.syntax.describe", scope: !3, file: !3, line: 44, type: !2, scopeLine: 44, spFlags: DISPFlagDefinition, unit: !1)
!38 = !DILocation(line: 44, column: 29, scope: !37)
!39 = !DILocation(line: 45, column: 3, scope: !37)
!40 = !DILocation(line: 45, column: 9, scope: !37)
!41 = !DILocation(line: 46, column: 14, scope: !37)
!42 = !DILocation(line: 47, column: 13, scope: !37)
!43 = !DILocation(line: 48, column: 10, scope: !37)
!44 = distinct !DISubprogram(name: "tests.syntax.classify", linkageName: "tests.syntax.classify", scope: !3, file: !3, line: 52, type: !2, scopeLine: 52, spFlags: DISPFlagDefinition, unit: !1)
!45 = !DILocation(line: 52, column: 40, scope: !44)
!46 = !DILocation(line: 53, column: 3, scope: !44)
!47 = !DILocation(line: 53, column: 9, scope: !44)
!48 = !DILocation(line: 54, column: 11, scope: !44)
!49 = !DILocation(line: 55, column: 12, scope: !44)
!50 = !DILocation(line: 55, column: 21, scope: !44)
!51 = !DILocation(line: 56, column: 18, scope: !44)
!52 = !DILocation(line: 57, column: 15, scope: !44)
!53 = !DILocation(line: 57, column: 24, scope: !44)
!54 = !DILocation(line: 57, column: 28, scope: !44)
!55 = !DILocation(line: 58, column: 10, scope: !44)
!56 = distinct !DISubprogram(name: "tests.syntax.nested", linkageName: "tests.syntax.nested", scope: !3, file: !3, line: 62, type: !2, scopeLine: 62, spFlags: DISPFlagDefinition, unit: !1)
!57 = !DILocation(line: 62, column: 49, scope: !56)
!58 = !DILocation(line: 63, column: 3, scope: !56)
!59 = !DILocation(line: 63, column: 9, scope: !56)
!60 = !DILocation(line: 64, column: 29, scope: !56)
!61 = !DILocation(line: 64, column: 36, scope: !56)
!62 = !DILocation(line: 64, column: 40, scope: !56)
!63 = !DILocation(line: 65, column: 32, scope: !56)
!64 = !DILocation(line: 66, column: 24, scope: !56)
!65 = distinct !DISubprogram(name: "tests.syntax.vowel", linkageName: "tests.syntax.vowel", scope: !3, file: !3, line: 70, type: !2, scopeLine: 70, spFlags: DISPFlagDefinition, unit: !1)
!66 = !DILocation(line: 70, column: 27, scope: !65)
!67 = !DILocation(line: 71, column: 3, scope: !65)
!68 = !DILocation(line: 71, column: 9, scope: !65)
!69 = !DILocation(line: 72, column: 36, scope: !65)
!70 = !DILocation(line: 73, column: 10, scope: !65)
!71 = distinct !DISubprogram(name: "tests.syntax.first_positive", linkageName: "tests.syntax.first_positive", scope: !3, file: !3, line: 77, type: !2, scopeLine: 77, spFlags: DISPFlagDefinition, unit: !1)
!72 = !DILocation(line: 77, column: 45, scope: !71)
!73 = !DILocation(line: 78, column: 3, scope: !71)
!74 = !DILocation(line: 78, column: 12, scope: !71)
!75 = !DILocation(line: 79, column: 5, scope: !71)
!76 = !DILocation(line: 79, column: 8, scope: !71)
!77 = !DILocation(line: 79, column: 12, scope: !71)
!78 = !DILocation(line: 80, column: 7, scope: !71)
!79 = !DILocation(line: 80, column: 14, scope: !71)
!80 = !DILocation(line: 83, column: 3, scope: !71)
!81 = distinct !DISubprogram(name: "tests.syntax.sign", linkageName: "tests.syntax.sign", scope: !3, file: !3, line: 86, type: !2, scopeLine: 86, spFlags: DISPFlagDefinition, unit: !1)
!82 = !DILocation(line: 86, column: 25, scope: !81)
!83 = !DILocation(line: 87, column: 3, scope: !81)
!84 = !DILocation(line: 87, column: 11, scope: !81)
!85 = !DILocation(line: 87, column: 17, scope: !81)
!86 = !DILocation(line: 88, column: 10, scope: !81)
!87 = !DILocation(line: 88, column: 17, scope: !81)
!88 = !DILocation(line: 89, column: 10, scope: !81)
!89 = !DILocation(line: 89, column: 13, scope: !81)
!90 = !DILocation(line: 89, column: 17, scope: !81)
!91 = !DILocation(line: 89, column: 21, scope: !81)
!92 = !DILocation(line: 89, column: 32, scope: !81)
!93 = !DILocation(line: 89, column: 34, scope: !81)
!94 = !DILocation(line: 91, column: 3, scope: !81)
!95 = distinct !DISubprogram(name: "tests.syntax.count_spaces", linkageName: "tests.syntax.count_spaces", scope: !3, file: !3, line: 94, type: !2, scopeLine: 94, spFlags: DISPFlagDefinition, unit: !1)
!96 = !DILocation(line: 94, column: 38, scope: !95)
!97 = !DILocation(line: 95, column: 3, scope: !95)
!98 = !DILocation(line: 95, column: 11, scope: !95)
!99 = !DILocation(line: 96, column: 3, scope: !95)
!100 = !DILocation(line: 96, column: 12, scope: !95)
!101 = !DILocation(line: 97, column: 5, scope: !95)
!102 = !DILocation(line: 97, column: 11, scope: !95)
!103 = !DILocation(line: 98, column: 16, scope: !95)
!104 = !DILocation(line: 99, column: 9, scope: !95)
!105 = !DILocation(line: 99, column: 13, scope: !95)
!106 = !DILocation(line: 99, column: 17, scope: !95)
!107 = !DILocation(line: 101, column: 12, scope: !95)
!108 = !DILocation(line: 104, column: 3, scope: !95)
!109 = distinct !DISubprogram(name: "tests.syntax.show", linkageName: "tests.syntax.show", scope: !3, file: !3, line: 107, type: !2, scopeLine: 107, spFlags: DISPFlagDefinition, unit: !1)
!110 = !DILocation(line: 107, column: 39, scope: !109)
!111 = !DILocation(line: 108, column: 3, scope: !109)
!112 = !DILocation(line: 108, column: 15, scope: !109)
!113 = !DILocation(line: 109, column: 3, scope: !109)
!114 = !DILocation(line: 109, column: 14, scope: !109)
!115 = !DILocation(line: 110, column: 3, scope: !109)
!116 = !DILocation(line: 110, column: 20, scope: !109)
!117 = distinct !DISubprogram(name: "tests.syntax.Syntax.connect", linkageName: "tests.syntax.Syntax.connect", scope: !3, file: !3, line: 114, type: !2, scopeLine: 114, spFlags: DISPFlagDefinition, unit: !1)
!118 = !DILocation(line: 114, column: 82, scope: !117)
!119 = !DILocation(line: 115, column: 5, scope: !117)
!120 = !DILocation(line: 115, column: 26, scope: !117)
!121 = !DILocation(line: 115, column: 31, scope: !117)
!122 = !DILocation(line: 115, column: 32, scope: !117)
!123 = !DILocation(line: 115, column: 35, scope: !117)
!124 = !DILocation(line: 116, column: 5, scope: !117)
!125 = !DILocation(line: 116, column: 10, scope: !117)
!126 = !DILocation(line: 116, column: 12, scope: !117)
!127 = !DILocation(line: 116, column: 19, scope: !117)
!128 = !DILocation(line: 117, column: 5, scope: !117)
!129 = !DILocation(line: 117, column: 10, scope: !117)
!130 = !DILocation(line: 117, column: 19, scope: !117)
!131 = !DILocation(line: 117, column: 24, scope: !117)
!132 = !DILocation(line: 117, column: 30, scope: !117)
!133 = !DILocation(line: 117, column: 39, scope: !117)
!134 = !DILocation(line: 117, column: 51, scope: !117)
!135 = !DILocation(line: 117, column: 60, scope: !117)
!136 = !DILocation(line: 118, column: 5, scope: !117)
!137 = !DILocation(line: 118, column: 10, scope: !117)
!138 = !DILocation(line: 118, column: 19, scope: !117)
!139 = !DILocation(line: 119, column: 5, scope: !117)
!140 = !DILocation(line: 119, column: 10, scope: !117)
!141 = !DILocation(line: 119, column: 19, scope: !117)
!142 = !DILocation(line: 119, column: 20, scope: !117)
!143 = !DILocation(line: 120, column: 5, scope: !117)
!144 = !DILocation(line: 120, column: 10, scope: !117)
!145 = !DILocation(line: 120, column: 19, scope: !117)
!146 = !DILocation(line: 120, column: 20, scope: !117)
!147 = !DILocation(line: 120, column: 28, scope: !117)
!148 = !DILocation(line: 121, column: 5, scope: !117)
!149 = !DILocation(line: 121, column: 10, scope: !117)
!150 = !DILocation(line: 121, column: 19, scope: !117)
!151 = !DILocation(line: 121, column: 20, scope: !117)
!152 = !DILocation(line: 121, column: 25, scope: !117)
!153 = !DILocation(line: 122, column: 5, scope: !117)
!154 = !DILocation(line: 122, column: 10, scope: !117)
!155 = !DILocation(line: 122, column: 19, scope: !117)
!156 = !DILocation(line: 122, column: 20, scope: !117)
!157 = !DILocation(line: 122, column: 25, scope: !117)
!158 = !DILocation(line: 122, column: 30, scope: !117)
!159 = !DILocation(line: 123, column: 5, scope: !117)
!160 = !DILocation(line: 123, column: 10, scope: !117)
!161 = !DILocation(line: 123, column: 17, scope: !117)
!162 = !DILocation(line: 123, column: 22, scope: !117)
!163 = !DILocation(line: 123, column: 23, scope: !117)
!164 = !DILocation(line: 123, column: 29, scope: !117)
!165 = !DILocation(line: 123, column: 30, scope: !117)
!166 = !DILocation(line: 123, column: 33, scope: !117)
!167 = !DILocation(line: 124, column: 5, scope: !117)
!168 = !DILocation(line: 124, column: 10, scope: !117)
!169 = !DILocation(line: 124, column: 17, scope: !117)
!170 = !DILocation(line: 124, column: 22, scope: !117)
!171 = !DILocation(line: 124, column: 23, scope: !117)
!172 = !DILocation(line: 124, column: 30, scope: !117)
!173 = !DILocation(line: 124, column: 31, scope: !117)
!174 = !DILocation(line: 125, column: 5, scope: !117)
!175 = !DILocation(line: 125, column: 10, scope: !117)
!176 = !DILocation(line: 125, column: 17, scope: !117)
!177 = !DILocation(line: 126, column: 5, scope: !117)
!178 = !DILocation(line: 126, column: 10, scope: !117)
!179 = !DILocation(line: 126, column: 17, scope: !117)
!180 = !DILocation(line: 126, column: 22, scope: !117)
!181 = !DILocation(line: 126, column: 23, scope: !117)
!182 = !DILocation(line: 126, column: 29, scope: !117)
!183 = !DILocation(line: 126, column: 30, scope: !117)
!184 = !DILocation(line: 127, column: 5, scope: !117)
!185 = !DILocation(line: 127, column: 13, scope: !117)
!186 = !DILocation(line: 127, column: 19, scope: !117)
!187 = !DILocation(line: 128, column: 5, scope: !117)
!188 = !DILocation(line: 128, column: 13, scope: !117)
!189 = !DILocation(line: 128, column: 19, scope: !117)
!190 = !DILocation(line: 129, column: 5, scope: !117)
!191 = !DILocation(line: 129, column: 10, scope: !117)
!192 = !DILocation(line: 129, column: 12, scope: !117)
!193 = !DILocation(line: 129, column: 16, scope: !117)
!194 = !DILocation(line: 130, column: 5, scope: !117)
!195 = !DILocation(line: 130, column: 10, scope: !117)
!196 = !DILocation(line: 130, column: 25, scope: !117)
!197 = !DILocation(line: 130, column: 26, scope: !117)
!198 = !DILocation(line: 130, column: 27, scope: !117)
!199 = !DILocation(line: 130, column: 30, scope: !117)
!200 = !DILocation(line: 130, column: 33, scope: !117)
!201 = !DILocation(line: 130, column: 36, scope: !117)
!202 = !DILocation(line: 131, column: 5, scope: !117)
!203 = !DILocation(line: 131, column: 10, scope: !117)
!204 = !DILocation(line: 131, column: 15, scope: !117)
!205 = !DILocation(line: 131, column: 20, scope: !117)
!206 = !DILocation(line: 131, column: 25, scope: !117)
!207 = !DILocation(line: 131, column: 30, scope: !117)
!208 = !DILocation(line: 131, column: 35, scope: !117)
!209 = !DILocation(line: 131, column: 36, scope: !117)
!210 = !DILocation(line: 132, column: 5, scope: !117)
!211 = !DILocation(line: 132, column: 10, scope: !117)
!212 = !DILocation(line: 132, column: 23, scope: !117)
!213 = !DILocation(line: 133, column: 5, scope: !117)
!214 = !DILocation(line: 133, column: 13, scope: !117)
!215 = !DILocation(line: 134, column: 5, scope: !117)
!216 = !DILocation(line: 134, column: 10, scope: !117)
!217 = !DILocation(line: 135, column: 5, scope: !117)
!218 = !DILocation(line: 135, column: 10, scope: !117)
!219 = !DILocation(line: 136, column: 5, scope: !117)
!220 = !DILocation(line: 136, column: 13, scope: !117)
!221 = !DILocation(line: 136, column: 30, scope: !117)
!222 = !DILocation(line: 136, column: 45, scope: !117)
!223 = !DILocation(line: 137, column: 5, scope: !117)
!224 = !DILocation(line: 137, column: 10, scope: !117)
!225 = !DILocation(line: 137, column: 20, scope: !117)
!226 = !DILocation(line: 138, column: 5, scope: !117)
!227 = !DILocation(line: 138, column: 13, scope: !117)
!228 = !DILocation(line: 138, column: 29, scope: !117)
!229 = !DILocation(line: 138, column: 30, scope: !117)
!230 = !DILocation(line: 139, column: 5, scope: !117)
!231 = !DILocation(line: 139, column: 10, scope: !117)
!232 = !DILocation(line: 139, column: 17, scope: !117)
!233 = !DILocation(line: 140, column: 5, scope: !117)
!234 = !DILocation(line: 140, column: 11, scope: !117)
!235 = !DILocation(line: 141, column: 19, scope: !117)
!236 = !DILocation(line: 141, column: 24, scope: !117)
!237 = !DILocation(line: 142, column: 16, scope: !117)
!238 = !DILocation(line: 142, column: 21, scope: !117)
!239 = !DILocation(line: 144, column: 5, scope: !117)
!240 = !DILocation(line: 144, column: 16, scope: !117)
!241 = !DILocation(line: 145, column: 5, scope: !117)
!242 = !DILocation(line: 145, column: 17, scope: !117)
!243 = !DILocation(line: 146, column: 5, scope: !117)
!244 = !DILocation(line: 146, column: 10, scope: !117)
!245 = !DILocation(line: 146, column: 27, scope: !117)
!246 = !DILocation(line: 146, column: 33, scope: !117)
!247 = !DILocation(line: 146, column: 57, scope: !117)
!248 = !DILocation(line: 147, column: 5, scope: !117)
!249 = !DILocation(line: 147, column: 16, scope: !117)
!250 = !DILocation(line: 147, column: 17, scope: !117)
!251 = !DILocation(line: 147, column: 20, scope: !117)
!252 = !DILocation(line: 148, column: 5, scope: !117)
!253 = !DILocation(line: 148, column: 13, scope: !117)
!254 = !DILocation(line: 149, column: 5, scope: !117)
!255 = !DILocation(line: 149, column: 11, scope: !117)
!256 = !DILocation(line: 149, column: 15, scope: !117)
!257 = !DILocation(line: 150, column: 7, scope: !117)
!258 = !DILocation(line: 150, column: 27, scope: !117)
!259 = !DILocation(line: 151, column: 7, scope: !117)
!260 = !DILocation(line: 151, column: 14, scope: !117)
!261 = !DILocation(line: 151, column: 15, scope: !117)
!262 = !DILocation(line: 151, column: 23, scope: !117)
!263 = !DILocation(line: 151, column: 26, scope: !117)
!264 = !DILocation(line: 151, column: 33, scope: !117)
!265 = !DILocation(line: 152, column: 7, scope: !117)
!266 = !DILocation(line: 152, column: 11, scope: !117)
!267 = !DILocation(line: 152, column: 15, scope: !117)
!268 = !DILocation(line: 154, column: 5, scope: !117)
!269 = !DILocation(line: 154, column: 25, scope: !117)
!270 = !DILocation(line: 155, column: 5, scope: !117)
!271 = !DILocation(line: 155, column: 16, scope: !117)
!272 = !DILocation(line: 156, column: 5, scope: !117)
!273 = !DILocation(line: 156, column: 10, scope: !117)
!274 = !DILocation(line: 156, column: 12, scope: !117)
!275 = !DILocation(line: 156, column: 20, scope: !117)
!276 = !DILocation(line: 157, column: 5, scope: !117)
!277 = !DILocation(line: 157, column: 17, scope: !117)
!278 = !DILocation(line: 157, column: 18, scope: !117)
!279 = !DILocation(line: 157, column: 21, scope: !117)
!280 = !DILocation(line: 158, column: 5, scope: !117)
!281 = !DILocation(line: 158, column: 18, scope: !117)
!282 = !DILocation(line: 159, column: 5, scope: !117)
!283 = !DILocation(line: 159, column: 9, scope: !117)
!284 = !DILocation(line: 159, column: 13, scope: !117)
!285 = !DILocation(line: 160, column: 5, scope: !117)
!286 = !DILocation(line: 160, column: 9, scope: !117)
!287 = !DILocation(line: 160, column: 13, scope: !117)
!288 = !DILocation(line: 161, column: 5, scope: !117)
!289 = !DILocation(line: 161, column: 10, scope: !117)
!290 = !DILocation(line: 161, column: 12, scope: !117)
!291 = !DILocation(line: 161, column: 15, scope: !117)
!292 = !DILocation(line: 162, column: 5, scope: !117)
!293 = !DILocation(line: 162, column: 13, scope: !117)
!294 = !DILocation(line: 162, column: 19, scope: !117)
!295 = !DILocation(line: 163, column: 5, scope: !117)
!296 = !DILocation(line: 163, column: 9, scope: !117)
!297 = !DILocation(line: 163, column: 15, scope: !117)
!298 = !DILocation(line: 164, column: 5, scope: !117)
!299 = !DILocation(line: 164, column: 24, scope: !117)
!300 = !DILocation(line: 165, column: 5, scope: !117)
!301 = !DILocation(line: 165, column: 11, scope: !117)
!302 = !DILocation(line: 166, column: 14, scope: !117)
!303 = !DILocation(line: 166, column: 19, scope: !117)
!304 = !DILocation(line: 167, column: 13, scope: !117)
!305 = !DILocation(line: 167, column: 18, scope: !117)
!306 = !DILocation(line: 169, column: 5, scope: !117)
!307 = !DILocation(line: 169, column: 10, scope: !117)
!308 = !DILocation(line: 169, column: 21, scope: !117)
!309 = !DILocation(line: 169, column: 26, scope: !117)
!310 = !DILocation(line: 169, column: 32, scope: !117)
!311 = !DILocation(line: 169, column: 43, scope: !117)
!312 = !DILocation(line: 170, column: 5, scope: !117)
!313 = !DILocation(line: 170, column: 8, scope: !117)
!314 = distinct !DISubprogram(name: "tests.syntax.Runner.main", linkageName: "tests.syntax.Runner.main", scope: !3, file: !3, line: 179, type: !2, scopeLine: 179, spFlags: DISPFlagDefinition, unit: !1)
!315 = !DILocation(line: 179, column: 55, scope: !314)
!316 = !DILocation(line: 180, column: 3, scope: !314)
!317 = !DILocation(line: 180, column: 16, scope: !314)
!318 = !DILocation(line: 180, column: 22, scope: !314)
!319 = !DILocation(line: 180, column: 40, scope: !314)
!320 = !DILocation(line: 180, column: 54, scope: !314)
!321 = !DILocation(line: 181, column: 19, scope: !314)
!322 = !DILocation(line: 182, column: 19, scope: !314)
!323 = !DILocation(line: 182, column: 26, scope: !314)
!324 = !DILocation(line: 182, column: 30, scope: !314)
!325 = !DILocation(line: 182, column: 51, scope: !314)
!326 = !DILocation(line: 184, column: 3, scope: !314)
!327 = !DILocation(line: 184, column: 9, scope: !314)
!328 = !DILocation(line: 184, column: 24, scope: !314)
!329 = !DILocation(line: 185, column: 14, scope: !314)
!330 = !DILocation(line: 185, column: 17, scope: !314)
!331 = !DILocation(line: 186, column: 19, scope: !314)
!332 = !DILocation(line: 186, column: 23, scope: !314)
!333 = !DILocation(line: 186, column: 44, scope: !314)
!334 = !{i32 7, !"Dwarf Version", i32 5}
!335 = !{i32 2, !"Debug Info Version", i32 3}
